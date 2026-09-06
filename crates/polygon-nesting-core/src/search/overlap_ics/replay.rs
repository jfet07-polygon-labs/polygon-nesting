//! **The replay probes: re-run a traced separation from a bite-microscope
//! capsule, deterministically, with an optional probe.** `--cell=replay` on
//! the benchmark; diagnostic only; nothing here is reachable from a default.
//!
//! # Why this exists
//!
//! `docs/experiments/overlap-ics/sparrow-warm-start/README.md` lines the
//! first twenty bites of both engines up on the identical constructor layout:
//! bites 1-16 cost the same, and then the hard bite costs Sparrow 17 passes
//! and us 37-43 master iterations. The bite microscope
//! (`super::microscope`, commit `9c38526`) traced that bite on seed 20 and
//! found that 339 of 339 fine-CD exits are by `limits`, 282 of them with a
//! nonzero residual (median 7.8 um), and that 176 of 492 changed rows had
//! their other endpoint already `visited` in the sweep. Those are the two
//! candidate causes GPT-6 Astra review 4 Q3
//! (`docs/astra-review-4-same-start-two-separators.md`) ranks, and Q3/Q4
//! prescribe **two detached probes on the traced bite before any live
//! mechanism is built**:
//!
//! 1. **fine-CD continuation** (`--probe=cdfinish`): relocates whose fine
//!    coordinate descent exited on its limits with nonzero incident
//!    violation continue the same walk from the accepted pose and saved walk
//!    state, with translation limits `min(existing, band/4)` = 1 um, an
//!    angular limit giving at most 1 um at the farthest source vertex, at
//!    most 64 further `+/-` pairs, stopping on incident zero, the finer
//!    limits or the budget; weighted comparison and accept-equal preserved;
//!    every evaluation charged (`relocate.rs::coord_descent_continue`). Then
//!    propagate through the sweep and the following iterations: does the
//!    separation clear the persistent blocking rows and enter the 4 um band,
//!    at what evaluation cost, against the traced control?
//! 2. **deferred-endpoint revisit** (`--probe=revisit`): a bounded revisit
//!    queue in the Gauss-Seidel sweep - a committed relocate that leaves a
//!    positive row on an endpoint already `visited` or `absent` from the
//!    order queues that endpoint (once per sweep per piece, bounded at the
//!    order length), and the queued pieces are relocated at the end of the
//!    sweep with ordinary CD unchanged (`descent.rs::gauss_seidel_replay`).
//!    Does it remove the blockers transferred to unavailable endpoints at
//!    equal evaluation work?
//!
//! # What a replay is
//!
//! A [`super::microscope::Capsule`] holds the poses, the mirror bits, the
//! GLS pair and edge weights, the target depth and the master descent's
//! stream coordinates at a retained bite's entry. The replay rebuilds the
//! [`super::state::IcsState`] from the request's sources at those poses,
//! restores the weights bit for bit, puts the descent's `iteration` and
//! `proposals` back and sets its `(bite, worker)` from the capsule's bite and
//! the trace's first winner (the capsule doc: "a replay sets them from `bite`
//! and the selected sweep's `winner` rather than from here"), and runs the
//! **same** explore separation loop: the eight-worker tournament,
//! winner-take-all by minimum guided Φ stable by ordinal, one GLS pass, the
//! band test at the top of every turn, the strike meter's snapshot, rollback
//! and ladder. Two things differ from [`super::Engine::separate`], both
//! stated: the replay **stops at band entry** and never calls the exact
//! authorities (it publishes nothing), and its iteration cap is the caller's
//! `--maxiters` rather than the profile's wall cap.
//!
//! # Control identity
//!
//! With `--probe=none` the replay must reproduce the traced sweeps: the
//! per-iteration `rawAfter`, `maxAfterMm` and `winner` must equal the
//! trace's `separations[].sweeps[]` bit for bit. That comparison is
//! `identity[]` in the report and is the replay's own correctness gate; a
//! probe result on a replay whose control does not pass means nothing.
//! [`Descent::worker_sweep_replay`] with both probes off is
//! [`Descent::worker_sweep_traced`]'s pass to the bit, and the live
//! `relocate`, `gauss_seidel` and `coord_descent` are untouched.
//!
//! # Diagnostic only
//!
//! The forbidden-rescue table in `docs/grok-review-12-reading-sparrow.md`
//! §5.2 (row "fixture as a seed") forbids starting a scored cell from a
//! known-good layout, and a replay capsule is exactly such a layout. So this
//! cell is never a default, it never publishes, and its document carries
//! `replay.tripwire` so `score.py` can refuse it. A replay's band entry is
//! not a depth and not a treatment score.

use std::collections::BTreeMap;

use serde::Serialize;

use super::descent::Descent;
use super::energy::{self, Totals};
use super::microscope::{blocking_rows, BlockingRow, RowId};
use super::relocate::{CdContinuation, ContinuationOutcome, RelocateOutcome, RelocateProbe};
use super::state::IcsState;
use super::{restore_keeping_weights, Engine, Phase};
use crate::search::overlap_ics_meter::strike_meter::{StrikeConfig, StrikeMeter};

/// Which probe a replay runs. `None` is the control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReplayProbe {
    None,
    Cdfinish,
    Revisit,
}

impl ReplayProbe {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "none" => Ok(Self::None),
            "cdfinish" => Ok(Self::Cdfinish),
            "revisit" => Ok(Self::Revisit),
            other => Err(format!("--probe must be none|cdfinish|revisit, not `{other}`")),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Cdfinish => "cdfinish",
            Self::Revisit => "revisit",
        }
    }
}

/// What the replay sweep is asked to do beyond the traced pass.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayProbeConfig {
    pub continuation: Option<CdContinuation>,
    pub revisit: bool,
}

impl ReplayProbeConfig {
    pub fn of(probe: ReplayProbe, band_mm: f64) -> Self {
        match probe {
            ReplayProbe::None => Self::default(),
            ReplayProbe::Cdfinish => Self {
                continuation: Some(CdContinuation::astra(band_mm)),
                revisit: false,
            },
            ReplayProbe::Revisit => Self {
                continuation: None,
                revisit: true,
            },
        }
    }
}

/// What [`RevisitQueue::offer`] did with a piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevisitOffer {
    Queued,
    AlreadyQueued,
    Full,
}

/// **The bounded revisit queue** of Astra review 4 Q3 probe 2: the
/// endpoints a sweep hands a positive row to after their turn has passed
/// (`visited`) or that were never in its colliding set (`absent`), each at
/// most once per sweep, the queue bounded at the sweep's order length.
/// Drained at the end of the sweep by `descent.rs::gauss_seidel_replay`.
#[derive(Clone, Debug)]
pub struct RevisitQueue {
    queue: Vec<usize>,
    queued: Vec<bool>,
    bound: usize,
}

impl RevisitQueue {
    /// `count` pieces, at most `bound` queued.
    pub fn new(count: usize, bound: usize) -> Self {
        Self {
            queue: Vec::with_capacity(bound.min(count)),
            queued: vec![false; count],
            bound,
        }
    }

    pub fn offer(&mut self, piece: usize) -> RevisitOffer {
        if self.queued[piece] {
            return RevisitOffer::AlreadyQueued;
        }
        if self.queue.len() >= self.bound {
            return RevisitOffer::Full;
        }
        self.queued[piece] = true;
        self.queue.push(piece);
        RevisitOffer::Queued
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// The queued pieces in the order they were queued.
    pub fn pieces(&self) -> &[usize] {
        &self.queue
    }
}

/// One worker's probe accounting for one sweep, summed over the eight slots
/// at the merge. Every counter is separate from the engine's work vector so
/// the probe's cost can be read beside the ordinary cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySweepStats {
    /// Relocates that ran (had positive incident raw Φ).
    pub relocates: u64,
    /// Relocates whose fine walk exited by `limits` with incident raw > 0:
    /// the continuation's population, counted whether or not the probe is on.
    pub limit_exits_with_residual: u64,
    pub continuation_walks: u64,
    pub continuation_evaluations: u64,
    pub continuation_candidate_pairs: u64,
    pub continuation_cleared: u64,
    pub continuation_stalled: u64,
    pub continuation_exhausted: u64,
    pub continuation_moved: u64,
    /// Row changes that qualified for the queue (positive after, endpoint
    /// `visited` or `absent`), duplicates included.
    pub revisit_candidates: u64,
    pub revisit_queued_visited: u64,
    pub revisit_queued_absent: u64,
    pub revisit_queue_full: u64,
    pub queued_relocates: u64,
    pub queued_ran: u64,
    pub queued_cleared: u64,
    pub queued_evaluations: u64,
}

impl ReplaySweepStats {
    pub fn observe_relocate(
        &mut self,
        outcome: &RelocateOutcome,
        probe: &RelocateProbe,
        continued: &ContinuationOutcome,
    ) {
        if !outcome.ran {
            return;
        }
        self.relocates += 1;
        if continued.ran {
            self.continuation_walks += 1;
            self.limit_exits_with_residual += 1;
            self.continuation_evaluations += continued.evaluations;
            self.continuation_candidate_pairs += continued.candidate_pairs as u64;
            self.continuation_cleared += u64::from(continued.cleared);
            self.continuation_stalled += u64::from(continued.stalled);
            self.continuation_exhausted += u64::from(continued.exhausted);
            self.continuation_moved += u64::from(continued.moved);
        } else if probe.fine_exit.stalled && outcome.after.raw > 0.0 {
            self.limit_exits_with_residual += 1;
        }
    }

    pub fn add(&mut self, other: &Self) {
        self.relocates += other.relocates;
        self.limit_exits_with_residual += other.limit_exits_with_residual;
        self.continuation_walks += other.continuation_walks;
        self.continuation_evaluations += other.continuation_evaluations;
        self.continuation_candidate_pairs += other.continuation_candidate_pairs;
        self.continuation_cleared += other.continuation_cleared;
        self.continuation_stalled += other.continuation_stalled;
        self.continuation_exhausted += other.continuation_exhausted;
        self.continuation_moved += other.continuation_moved;
        self.revisit_candidates += other.revisit_candidates;
        self.revisit_queued_visited += other.revisit_queued_visited;
        self.revisit_queued_absent += other.revisit_queued_absent;
        self.revisit_queue_full += other.revisit_queue_full;
        self.queued_relocates += other.queued_relocates;
        self.queued_ran += other.queued_ran;
        self.queued_cleared += other.queued_cleared;
        self.queued_evaluations += other.queued_evaluations;
    }
}

/// One traced sweep, as read from the microscope document: the identity
/// reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TracedSweep {
    pub iteration: u64,
    pub raw_after: f64,
    pub max_after_mm: f64,
    pub winner: u32,
}

/// What a replay is asked to do.
#[derive(Clone, Debug)]
pub struct ReplayParams {
    pub workers: usize,
    pub bite: u64,
    pub max_iterations: u64,
    pub probe: ReplayProbe,
    pub strikes: StrikeConfig,
    /// The trace's sweeps for this separation, for the identity comparison
    /// (every probe: the control compares, a probe reports where it diverged).
    pub traced: Vec<TracedSweep>,
    /// Rows whose clearing the report tracks: the control's most persistent
    /// blocking rows.
    pub watch_rows: Vec<RowId>,
}

/// `[iteration, rawTrace, rawReplay, maxTrace, maxReplay, winnerTrace,
/// winnerReplay, equal]` for one iteration the trace also has.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityRow {
    pub iteration: u64,
    pub raw_trace: f64,
    pub raw_replay: f64,
    pub max_trace: f64,
    pub max_replay: f64,
    pub winner_trace: u32,
    pub winner_replay: u32,
    pub equal: bool,
}

/// One master iteration of the replay.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayIteration {
    pub iteration: u64,
    pub raw_after: f64,
    pub guided_after: f64,
    pub max_after_mm: f64,
    pub winner: u32,
    pub contested: bool,
    pub new_minimum: bool,
    pub blocking: Vec<BlockingRow>,
    pub evaluations_all_workers: u64,
    pub evaluations_winner: u64,
    /// The continuation's share of `evaluations_all_workers`.
    pub continuation_evaluations: u64,
    /// The installed (winner's) sweep's queued relocates.
    pub queued_relocates: u64,
    pub queued_relocates_all_workers: u64,
    pub stats_all_workers: ReplaySweepStats,
    pub stats_winner: ReplaySweepStats,
}

/// Where a watched row stands at the end of the replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatchedRowStatus {
    /// Blocking at some point (the entry state included) and not blocking at
    /// the end: `cleared_at` is the first iteration after which it never
    /// reappeared.
    Cleared,
    BlockingAtEnd,
    NeverBlocking,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedRow {
    pub row_id: RowId,
    pub status: WatchedRowStatus,
    pub cleared_at_iteration: Option<u64>,
    pub blocking_iterations: u64,
    pub entry_residual_mm: f64,
    pub end_residual_mm: f64,
}

/// The replay's result.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayReport {
    pub probe: ReplayProbe,
    pub workers: u32,
    pub bite: u64,
    pub max_iterations: u64,
    pub band_mm: f64,
    pub continuation: Option<ContinuationParams>,
    pub revisit: bool,
    pub entry_raw: f64,
    pub entry_guided: f64,
    pub entry_max_mm: f64,
    pub entry_blocking: Vec<BlockingRow>,
    /// `band-entry`, `iteration-cap` or `struck`.
    pub stop: &'static str,
    pub iterations: Vec<ReplayIteration>,
    pub band_entered_at_iteration: Option<u64>,
    /// All eight workers' sample evaluations up to the band entry,
    /// continuation included.
    pub evaluations_to_band: Option<u64>,
    pub evaluations_total: u64,
    pub continuation_evaluations_total: u64,
    pub queued_relocates_total: u64,
    pub queued_relocates_all_workers_total: u64,
    pub stats_total: ReplaySweepStats,
    pub rollbacks: Vec<u64>,
    pub identity: Vec<IdentityRow>,
    pub identity_pass: u64,
    pub identity_fail: u64,
    /// The first iteration whose reading differs from the trace, if any
    /// (a probe is expected to diverge; the control must not).
    pub diverges_from_trace_at_iteration: Option<u64>,
    pub watched_rows: Vec<WatchedRow>,
    /// `rowId -> iterationCleared` for the watched rows that cleared.
    pub persistent_rows_cleared: BTreeMap<String, u64>,
}

/// The continuation's parameters, as the report prints them.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationParams {
    pub translation_limit_mm: f64,
    pub vertex_displacement_limit_mm: f64,
    pub max_candidate_pairs: u32,
}

impl From<CdContinuation> for ContinuationParams {
    fn from(value: CdContinuation) -> Self {
        Self {
            translation_limit_mm: value.translation_limit_mm,
            vertex_displacement_limit_mm: value.vertex_displacement_limit_mm,
            max_candidate_pairs: value.max_candidate_pairs,
        }
    }
}

/// One competitive worker's private world for one replay iteration: the
/// live `Slot` without its feature-gated instruments, plus the probe stats.
struct ReplaySlot {
    state: IcsState,
    descent: Descent,
    work: super::diagnostics::WorkVector,
    stats: ReplaySweepStats,
}

impl<'a> Engine<'a> {
    /// **Replay only.** Puts a capsule's GLS weights back onto the rows just
    /// rebuilt from its poses, bit for bit. Refuses a capsule of the wrong
    /// shape.
    pub fn restore_capsule_weights(
        &mut self,
        pair_weights: &[f64],
        edge_weights: &[[f64; 4]],
    ) -> Result<(), String> {
        if pair_weights.len() != self.state.pair_rows.len() {
            return Err(format!(
                "the capsule carries {} pair weights; this request has {} pair rows",
                pair_weights.len(),
                self.state.pair_rows.len()
            ));
        }
        if edge_weights.len() != self.state.edge_rows.len() {
            return Err(format!(
                "the capsule carries {} edge weight rows; this request has {} pieces",
                edge_weights.len(),
                self.state.edge_rows.len()
            ));
        }
        for (row, weight) in self.state.pair_rows.iter_mut().zip(pair_weights) {
            row.weight = *weight;
        }
        for (rows, weights) in self.state.edge_rows.iter_mut().zip(edge_weights) {
            for (row, weight) in rows.iter_mut().zip(weights) {
                row.weight = *weight;
            }
        }
        Ok(())
    }

    /// **Replay only.** See [`Descent::restore_replay_stream`].
    pub fn restore_replay_stream(&mut self, bite: u64, worker: u64, iteration: u64, proposals: u64) {
        self.descent
            .restore_replay_stream(bite, worker, iteration, proposals);
    }

    /// **The replayed separation.** [`Engine::separate`]'s explore loop from
    /// the engine's current state, with the replay tournament in place of
    /// the live one and two stated differences: it stops at band entry
    /// without calling the exact authorities, and its cap is
    /// `params.max_iterations`. See the module doc.
    pub fn replay_separation(&mut self, params: &ReplayParams) -> ReplayReport {
        let band = self.config.limits.band_mm;
        let probe = ReplayProbeConfig::of(params.probe, band);
        let workers = params.workers.max(1);
        let entry = energy::fold(&self.state);
        let entry_blocking = blocking_rows(&self.state);
        let mut snapshot = self.state.clone();
        let mut meter = StrikeMeter::for_phase(params.strikes, Phase::Explore, entry.raw);
        let mut batch_sample_evaluations = 0u64;
        let mut iterations = 0u64;
        let mut rollbacks = Vec::new();
        let mut records: Vec<ReplayIteration> = Vec::new();
        let mut band_entered_at = None;
        let mut evaluations_cumulative = 0u64;
        let mut evaluations_to_band = None;
        let stop = loop {
            let totals = energy::fold(&self.state);
            let new_minimum = meter
                .observe(totals.raw, batch_sample_evaluations)
                .is_new_minimum();
            if new_minimum {
                snapshot.clone_from(&self.state);
            }
            if let Some(last) = records.last_mut() {
                last.new_minimum = new_minimum;
            }
            // The band test comes first, exactly as in the live loop. The
            // live loop would now call `attempt_publication`; the replay
            // stops here and reports.
            if totals.max_violation_mm <= band {
                band_entered_at = Some(iterations);
                evaluations_to_band = Some(evaluations_cumulative);
                break "band-entry";
            }
            if meter.patience_exhausted() {
                restore_keeping_weights(&mut self.state, &snapshot);
                rollbacks.push(iterations);
                let event = meter.strike();
                if event.struck_out {
                    break "struck";
                }
            }
            if iterations >= params.max_iterations {
                break "iteration-cap";
            }
            let samples_before = self.trace.work.sample_evaluations;
            let (totals, winner, contested, all_stats, winner_stats, all_evaluations, winner_evaluations) =
                self.replay_tournament(workers, params.bite, &probe);
            iterations += 1;
            batch_sample_evaluations = self.trace.work.sample_evaluations - samples_before;
            evaluations_cumulative += all_evaluations;
            records.push(ReplayIteration {
                iteration: iterations,
                raw_after: totals.raw,
                guided_after: totals.guided,
                max_after_mm: totals.max_violation_mm,
                winner: winner as u32,
                contested,
                new_minimum: false,
                blocking: blocking_rows(&self.state),
                evaluations_all_workers: all_evaluations,
                evaluations_winner: winner_evaluations,
                continuation_evaluations: all_stats.continuation_evaluations,
                queued_relocates: winner_stats.queued_relocates,
                queued_relocates_all_workers: all_stats.queued_relocates,
                stats_all_workers: all_stats,
                stats_winner: winner_stats,
            });
        };

        // The identity comparison against the trace.
        let mut identity = Vec::new();
        let mut identity_pass = 0u64;
        let mut identity_fail = 0u64;
        let mut diverges_at = None;
        for record in &records {
            let Some(traced) = params
                .traced
                .iter()
                .find(|sweep| sweep.iteration == record.iteration)
            else {
                continue;
            };
            let equal = traced.raw_after.to_bits() == record.raw_after.to_bits()
                && traced.max_after_mm.to_bits() == record.max_after_mm.to_bits()
                && traced.winner == record.winner;
            if equal {
                identity_pass += 1;
            } else {
                identity_fail += 1;
                if diverges_at.is_none() {
                    diverges_at = Some(record.iteration);
                }
            }
            identity.push(IdentityRow {
                iteration: record.iteration,
                raw_trace: traced.raw_after,
                raw_replay: record.raw_after,
                max_trace: traced.max_after_mm,
                max_replay: record.max_after_mm,
                winner_trace: traced.winner,
                winner_replay: record.winner,
                equal,
            });
        }

        // The watched rows: blocking at entry (iteration 0) and after every
        // iteration.
        let residual_in = |rows: &[BlockingRow], id: RowId| {
            rows.iter().find(|row| row.0 == id).map_or(0.0, |row| row.1)
        };
        let mut watched_rows = Vec::new();
        let mut persistent_rows_cleared = BTreeMap::new();
        for &id in &params.watch_rows {
            let entry_residual = residual_in(&entry_blocking, id);
            let mut last_blocking: Option<u64> = (entry_residual > 0.0).then_some(0);
            let mut blocking_iterations = u64::from(entry_residual > 0.0);
            for record in &records {
                if residual_in(&record.blocking, id) > 0.0 {
                    last_blocking = Some(record.iteration);
                    blocking_iterations += 1;
                }
            }
            let end_residual = records
                .last()
                .map_or(entry_residual, |record| residual_in(&record.blocking, id));
            let (status, cleared_at) = match last_blocking {
                None => (WatchedRowStatus::NeverBlocking, None),
                Some(_) if end_residual > 0.0 => (WatchedRowStatus::BlockingAtEnd, None),
                Some(last) => (WatchedRowStatus::Cleared, Some(last + 1)),
            };
            if let Some(at) = cleared_at {
                persistent_rows_cleared.insert(id.to_string(), at);
            }
            watched_rows.push(WatchedRow {
                row_id: id,
                status,
                cleared_at_iteration: cleared_at,
                blocking_iterations,
                entry_residual_mm: entry_residual,
                end_residual_mm: end_residual,
            });
        }

        let mut stats_total = ReplaySweepStats::default();
        let mut queued_winner_total = 0u64;
        for record in &records {
            stats_total.add(&record.stats_all_workers);
            queued_winner_total += record.queued_relocates;
        }
        ReplayReport {
            probe: params.probe,
            workers: workers as u32,
            bite: params.bite,
            max_iterations: params.max_iterations,
            band_mm: band,
            continuation: probe.continuation.map(ContinuationParams::from),
            revisit: probe.revisit,
            entry_raw: entry.raw,
            entry_guided: entry.guided,
            entry_max_mm: entry.max_violation_mm,
            entry_blocking,
            stop,
            band_entered_at_iteration: band_entered_at,
            evaluations_to_band,
            evaluations_total: evaluations_cumulative,
            continuation_evaluations_total: stats_total.continuation_evaluations,
            queued_relocates_total: queued_winner_total,
            queued_relocates_all_workers_total: stats_total.queued_relocates,
            stats_total,
            rollbacks,
            identity,
            identity_pass,
            identity_fail,
            diverges_from_trace_at_iteration: diverges_at,
            watched_rows,
            persistent_rows_cleared,
            iterations: records,
        }
    }

    /// [`Engine::tournament`]'s steps 1-7 with [`Descent::worker_sweep_replay`]
    /// in every slot: clone, set the stream ordinal, sweep in scoped threads,
    /// join in ordinal order, select the minimum guided Φ stable by ordinal,
    /// install, one GLS pass. Returns the post-GLS totals, the winner, the
    /// contested flag, the summed and the winner's probe stats, and the
    /// summed and the winner's sample evaluations.
    #[allow(clippy::type_complexity)]
    fn replay_tournament(
        &mut self,
        workers: usize,
        bite: u64,
        probe: &ReplayProbeConfig,
    ) -> (Totals, usize, bool, ReplaySweepStats, ReplaySweepStats, u64, u64) {
        let mut slots: Vec<ReplaySlot> = Vec::with_capacity(workers);
        for ordinal in 0..workers {
            let mut descent = self.descent.clone();
            descent.set_stream(bite, ordinal as u64);
            slots.push(ReplaySlot {
                state: self.state.clone(),
                descent,
                work: super::diagnostics::WorkVector::default(),
                stats: ReplaySweepStats::default(),
            });
        }
        let sources: &[super::state::PieceSource] = &self.sources;
        let contract: &super::state::Contract = &self.contract;
        let mut outcomes = Vec::with_capacity(workers);
        if workers == 1 {
            let slot = &mut slots[0];
            outcomes.push(slot.descent.worker_sweep_replay(
                &mut slot.state,
                sources,
                contract,
                &mut slot.work,
                probe,
                &mut slot.stats,
            ));
        } else {
            std::thread::scope(|scope| {
                let handles: Vec<_> = slots
                    .iter_mut()
                    .map(|slot| {
                        scope.spawn(move || {
                            slot.descent.worker_sweep_replay(
                                &mut slot.state,
                                sources,
                                contract,
                                &mut slot.work,
                                probe,
                                &mut slot.stats,
                            )
                        })
                    })
                    .collect();
                for handle in handles {
                    outcomes.push(handle.join().expect("a replay worker panicked"));
                }
            });
        }
        let mut all_stats = ReplaySweepStats::default();
        let mut all_evaluations = 0u64;
        for slot in &slots {
            self.trace.work.saturating_add(&slot.work);
            all_stats.add(&slot.stats);
            all_evaluations += slot.work.sample_evaluations;
        }
        let mut winner = 0usize;
        for ordinal in 1..workers {
            if outcomes[ordinal].totals.guided < outcomes[winner].totals.guided {
                winner = ordinal;
            }
        }
        let contested = outcomes
            .iter()
            .any(|other| other.totals.guided != outcomes[0].totals.guided);
        let slot = slots.swap_remove(winner);
        let winner_stats = slot.stats;
        let winner_evaluations = slot.work.sample_evaluations;
        self.state = slot.state;
        self.descent = slot.descent;
        self.trace.sweeps += 1;
        energy::gls_update(&mut self.state);
        self.trace.work.weight_updates += 1;
        let totals = energy::fold(&self.state);
        (
            totals,
            winner,
            contested,
            all_stats,
            winner_stats,
            all_evaluations,
            winner_evaluations,
        )
    }
}
