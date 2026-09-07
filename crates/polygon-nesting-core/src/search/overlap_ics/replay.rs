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
//! 3. **the exponent probe** (`--probe=exponent:<p>`), from the microscope
//!    README's addendum "how the column broke" and its Sparrow correction:
//!    neither detached probe broke the pinned column early, and the trace
//!    says why - the column's members leave only when their rows' GLS
//!    weights reach ~1e5 (36 updates at ~1.5x), because the guided objective
//!    is `w v^2` and a 5 um residual is `8e-6` of a 1.8 mm fresh overlap.
//!    Sparrow's loss at the pinned revision is ~`sqrt(penetration)`, so the
//!    same escape needs a weight of ~20 (3-5 updates); it breaks the same
//!    column on the same layout in 17 passes. The hypothesis is that the
//!    exponent on the violation in the **guided ranking** sets the escape
//!    time. The probe computes every guided quantity on the replay path as
//!    `sum w v^p` instead of `sum w v^2`: the candidate ranking inside the
//!    relocate (`relocate.rs::relocate_inner_with_exponent`, through
//!    `energy::incident_totals_with_exponent`) and the tournament's winner
//!    selection (`energy::fold_with_exponent`). Nothing else: raw Φ
//!    (`sum v^2`) stays for the band test, the strike meter's minimum and
//!    the `v / v_max` weight growth in `gls_update`, and the lexicographic
//!    "any clear pose beats every colliding pose" stays. `p = 2` must
//!    reproduce `--probe=none` bit for bit (the probe's own identity gate;
//!    `v * v` is special-cased for it). It is a landscape change, not a
//!    constant change: a live version would go to a prospective spec of
//!    its own.
//!
//! 4. **the committed geometry** (this commit; `--fork=<sweep>`,
//!    `--certify=1`, and fields on every replay document): GPT-6 Astra
//!    review 5b (`docs/astra-review-5b-the-exponent.md`, Q6-Q8) scored the
//!    exponent probe against registered deadlines that name a *column
//!    break* iteration, and asked that the break be read off **committed
//!    geometry and row identities**, not off a persistent-row table: the
//!    blocking graph, its bottom-to-top paths, the releases of the
//!    column's rows (temporary ones recorded separately), `column avoided`
//!    when the treatment never forms one ([`analyse_column`]). It asked
//!    for the replay's identity gate to compare **pose, weight and stream
//!    fingerprints, committed relocates and evaluation counts** and not
//!    only three scalars (Q7 item 4; [`IdentityRow`]), for one **fixed
//!    diagnostic fork** at the end of control sweep 24 where the four core
//!    members were unmoved, re-scoring identical candidate poses under all
//!    four exponents at the same weights (Q8; [`ForkRelocate`],
//!    `relocate.rs::relocate_inner_with_exponent`), for an explicit
//!    **resolved seed** (Q7 item 5; the benchmark's `resolvedSeed`), and
//!    for a **detached check of the unchanged publication path** before
//!    any band entry is called a certification (Q6's qualification;
//!    [`CertificationReport`], through [`Engine::attempt_publication`]
//!    unchanged). All of it is observation: the fork changes no
//!    trajectory, the column is computed after the run from the states it
//!    left behind, and the certification runs after the replay has already
//!    stopped.
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
//! stated: the replay **stops at band entry**, where the live loop would
//! call `attempt_publication` - it makes that call only under `--certify=1`,
//! once, after the stop (`certify_band_entry`, [`CertificationReport`]) -
//! and its iteration cap is the caller's `--maxiters` rather than the
//! profile's wall cap.
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
//! cell is never a default, it installs nothing, and its document carries
//! `replay.tripwire` so `score.py` can refuse it. A replay's band entry is
//! not a depth and not a treatment score. Without `--certify=1` no exact
//! authority is called and nothing is published. With it, the replay makes
//! the live loop's one publication call after it has stopped at band entry;
//! that call may publish, and the publication goes into
//! `replay.certification` of the refused document only: no state is
//! installed, the only incumbent that receives it is the replay engine's
//! placeholder (infinite depth, never read again, never written out), and
//! the document stays refused by the same tripwire.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::descent::Descent;
use super::energy::{self, incident_totals_with_exponent, Totals};
use super::microscope::{
    blocking_rows, boundary_row_id, decode_row_id, BlockingRow, RelocateRecord, RowId, RowKind,
    StreamKey, SweepRecord, SweepTrace,
};
use super::relocate::{CdContinuation, ContinuationOutcome, RelocateOutcome, RelocateProbe};
use super::state::{IcsState, Pose, EDGE_BOTTOM, EDGE_TOP};
use super::{restore_keeping_weights, Engine, Phase};
use crate::search::overlap_ics_meter::strike_meter::{StrikeConfig, StrikeMeter};

/// Which probe a replay runs. `None` is the control. Serialized as its
/// [`ReplayProbe::label`] (`none`, `cdfinish`, `revisit`, `exponent:<p>`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReplayProbe {
    None,
    Cdfinish,
    Revisit,
    /// `--probe=exponent:<p>`: the guided ranking and the tournament's
    /// winner on `sum w v^p` (module doc, probe 3). `p` is a finite
    /// positive `f64`; `p = 2` is the identity gate.
    Exponent(f64),
}

impl ReplayProbe {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "none" => Ok(Self::None),
            "cdfinish" => Ok(Self::Cdfinish),
            "revisit" => Ok(Self::Revisit),
            other => match other.strip_prefix("exponent:") {
                Some(exponent) => {
                    let exponent: f64 = exponent.trim().parse().map_err(|error| {
                        format!("--probe=exponent:<p>: `{exponent}` is not a number ({error})")
                    })?;
                    if !exponent.is_finite() || exponent <= 0.0 {
                        return Err(format!(
                            "--probe=exponent:<p>: p must be a finite positive number, not {exponent}"
                        ));
                    }
                    Ok(Self::Exponent(exponent))
                }
                None => Err(format!(
                    "--probe must be none|cdfinish|revisit|exponent:<p>, not `{other}`"
                )),
            },
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::None => "none".to_owned(),
            Self::Cdfinish => "cdfinish".to_owned(),
            Self::Revisit => "revisit".to_owned(),
            Self::Exponent(exponent) => format!("exponent:{exponent}"),
        }
    }
}

impl Serialize for ReplayProbe {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.label())
    }
}

/// What the replay sweep is asked to do beyond the traced pass. The three
/// probes are exclusive: exactly one of `continuation`, `revisit` and
/// `exponent` is set, or none for the control.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayProbeConfig {
    pub continuation: Option<CdContinuation>,
    pub revisit: bool,
    /// `Some(p)`: rank on `sum w v^p` (`energy::fold_with_exponent`,
    /// `energy::incident_totals_with_exponent`). `Some(2.0)` is the
    /// control's ranking to the bit, through the probe's own code path.
    pub exponent: Option<f64>,
}

impl ReplayProbeConfig {
    pub fn of(probe: ReplayProbe, band_mm: f64) -> Self {
        match probe {
            ReplayProbe::None => Self::default(),
            ReplayProbe::Cdfinish => Self {
                continuation: Some(CdContinuation::astra(band_mm)),
                revisit: false,
                exponent: None,
            },
            ReplayProbe::Revisit => Self {
                continuation: None,
                revisit: true,
                exponent: None,
            },
            ReplayProbe::Exponent(exponent) => Self {
                continuation: None,
                revisit: false,
                exponent: Some(exponent),
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

/// One committed relocate of a traced sweep, as the identity gate compares
/// it: the piece, the committed displacement bit for bit, and the changed
/// rows (`microscope::row_changes`, status dropped).
#[derive(Clone, Debug, PartialEq)]
pub struct TracedRelocate {
    pub piece: u32,
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub dtheta_deg: f64,
    /// `(rowId, violationBefore, violationAfter)`.
    pub rows: Vec<(RowId, f64, f64)>,
}

/// One traced sweep, as read from the microscope document: the identity
/// reference. The three scalars are the original gate; the relocates, the
/// evaluation counts and the stream key are the extended gate (Astra 5b Q7
/// item 4).
#[derive(Clone, Debug, PartialEq)]
pub struct TracedSweep {
    pub iteration: u64,
    pub raw_after: f64,
    pub max_after_mm: f64,
    pub winner: u32,
    pub evaluations_all_workers: u64,
    pub evaluations_winner: u64,
    /// The key the winner's sweep drew from (`sweeps[].stream`), if the
    /// trace has it.
    pub stream: Option<StreamKey>,
    pub relocates: Vec<TracedRelocate>,
}

impl From<&SweepRecord> for TracedSweep {
    fn from(sweep: &SweepRecord) -> Self {
        Self {
            iteration: sweep.iteration,
            raw_after: sweep.raw_after,
            max_after_mm: sweep.max_after_mm,
            winner: sweep.winner,
            evaluations_all_workers: sweep.evaluations_all_workers,
            evaluations_winner: sweep.evaluations_winner,
            stream: sweep.stream,
            relocates: sweep
                .relocates
                .iter()
                .map(|relocate| TracedRelocate {
                    piece: relocate.piece,
                    dx_mm: relocate.dx_mm,
                    dy_mm: relocate.dy_mm,
                    dtheta_deg: relocate.dtheta_deg,
                    rows: relocate
                        .rows
                        .iter()
                        .map(|change| (change.0, change.1, change.2))
                        .collect(),
                })
                .collect(),
        }
    }
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
    /// `--fork=<sweep>`: run the probe through sweep `<sweep>`, then in sweep
    /// `<sweep> + 1` re-score every relocate's candidates under the four
    /// exponents ([`FORK_EXPONENTS`]) without changing the trajectory, and
    /// stop (`stop = "fork"`). Only with `--probe=none|exponent:<p>`.
    pub fork: Option<u64>,
    /// `--certify=1`: at band entry, call the unchanged live publication
    /// path once on the band-entry state and record what it said.
    pub certify: bool,
}

/// The identity comparison of one replay iteration against the traced
/// sweep with the same iteration number. `equal` is the conjunction of the
/// original scalar test (`rawAfter`, `maxAfterMm`, `winner` bit for bit),
/// `relocatesEqual` and `evaluationsEqual`. The fingerprints are the
/// replay's own: the trace does not carry every pose, so the pose
/// comparison goes through `relocatesEqual` (same pieces in the same order,
/// same `dx/dy/dtheta` bits, same changed rows with the same before/after
/// bits); the stream fingerprint is compared with the one derived from the
/// traced sweep's `stream` key (`streamEqual`, reported beside `equal`).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityRow {
    pub iteration: u64,
    pub raw_trace: f64,
    pub raw_replay: f64,
    pub max_trace: f64,
    pub max_replay: f64,
    pub winner_trace: u32,
    pub winner_replay: u32,
    /// The original three-scalar test.
    pub scalars_equal: bool,
    /// FNV-1a 64 over every piece's `x, y, theta` bits after the iteration,
    /// in piece order ([`poses_fingerprint`]).
    pub poses_fingerprint: String,
    /// FNV-1a 64 over every pair row weight in pair-id order, then every
    /// edge weight in piece order, `L R B T` ([`weights_fingerprint`]).
    pub weights_fingerprint: String,
    /// FNV-1a 64 over the master descent's stream bookkeeping after the
    /// iteration: `(seed, bite, iteration, worker, proposals)`
    /// ([`stream_fingerprint`]).
    pub stream_fingerprint: String,
    /// The same fingerprint derived from the traced sweep's `stream` key
    /// (`iteration + 1`, its `worker`) and the proposal ordinal the sweep
    /// must have left (`entry + k * pieces`); `null` if the trace has no
    /// stream key.
    pub stream_fingerprint_trace: Option<String>,
    pub stream_equal: Option<bool>,
    pub evaluations_all_workers: u64,
    pub evaluations_winner: u64,
    pub evaluations_all_workers_trace: u64,
    pub evaluations_winner_trace: u64,
    pub relocates_replay: u64,
    pub relocates_trace: u64,
    pub relocates_equal: bool,
    /// Where the relocates first differ, for a reader; `null` when equal.
    pub relocates_first_difference: Option<String>,
    pub evaluations_equal: bool,
    pub equal: bool,
}

/// `[rowId, violationBeforeMm, violationAfterMm]` of one changed row of a
/// committed relocate: `microscope::RowChange` without the endpoint
/// status.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct RowDelta(pub RowId, pub f64, pub f64);

/// One committed relocate of the winner's sweep, as the replay document
/// carries it: the microscope's `RelocateRecord` (the same row-change
/// collection, `microscope::row_changes`) reduced to the committed
/// geometry.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayRelocate {
    pub piece: u32,
    pub origin: &'static str,
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub dtheta_deg: f64,
    pub moved: bool,
    pub raw_before: f64,
    pub raw_after: f64,
    pub guided_before: f64,
    pub guided_after: f64,
    pub max_before_mm: f64,
    pub max_after_mm: f64,
    pub rows: Vec<RowDelta>,
}

impl From<&RelocateRecord> for ReplayRelocate {
    fn from(record: &RelocateRecord) -> Self {
        Self {
            piece: record.piece,
            origin: record.origin,
            dx_mm: record.dx_mm,
            dy_mm: record.dy_mm,
            dtheta_deg: record.dtheta_deg,
            moved: record.moved,
            raw_before: record.raw_before,
            raw_after: record.raw_after,
            guided_before: record.guided_before,
            guided_after: record.guided_after,
            max_before_mm: record.max_before_mm,
            max_after_mm: record.max_after_mm,
            rows: record
                .rows
                .iter()
                .map(|change| RowDelta(change.0, change.1, change.2))
                .collect(),
        }
    }
}

/// `[rowId, violationMm, weight]` of one column row after an iteration.
/// The violation is the row's signed value from the state (a released
/// boundary row reads negative), the weight the row's actual GLS weight.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ColumnRowReading(pub RowId, pub f64, pub f64);

/// `[piece, xMm, yMm, thetaDeg]` of one core member after an iteration.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct CorePose(pub u32, pub f64, pub f64, pub f64);

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
    /// The winner's committed relocates in sweep order (the queued
    /// relocates of the revisit probe are not traced and not listed).
    pub relocates: Vec<ReplayRelocate>,
    /// The column's rows after this iteration ([`ColumnReport::rows`]);
    /// empty when no column formed.
    pub column_rows: Vec<ColumnRowReading>,
    /// The core members' poses after this iteration.
    pub core_poses: Vec<CorePose>,
    pub poses_fingerprint: String,
    pub weights_fingerprint: String,
    pub stream_fingerprint: String,
    /// The master descent's stream key after the iteration.
    pub stream: StreamKey,
    pub proposals: u64,
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

// ------------------------------------------------------------ fingerprints --

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64 over the little-endian bytes of a sequence of 64-bit words.
pub fn fnv1a_words(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut hash = FNV_OFFSET;
    for word in words {
        for byte in word.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

fn hex16(hash: u64) -> String {
    format!("{hash:016x}")
}

/// The pose fingerprint: every piece's `x, y, theta` bit patterns in piece
/// order.
pub fn poses_fingerprint(poses: &[Pose]) -> String {
    hex16(fnv1a_words(poses.iter().flat_map(|pose| {
        [
            pose.tx_mm.to_bits(),
            pose.ty_mm.to_bits(),
            pose.theta_deg.to_bits(),
        ]
    })))
}

/// The weight fingerprint: every pair row weight in pair-id order, then
/// every edge weight in piece order, `L R B T`.
pub fn weights_fingerprint(state: &IcsState) -> String {
    hex16(fnv1a_words(
        state
            .pair_rows
            .iter()
            .map(|row| row.weight.to_bits())
            .chain(
                state
                    .edge_rows
                    .iter()
                    .flat_map(|rows| rows.iter().map(|row| row.weight.to_bits())),
            ),
    ))
}

/// The stream fingerprint: the descent's stream bookkeeping,
/// `(seed, bite, iteration, worker, proposals)`.
pub fn stream_fingerprint(key: StreamKey, proposals: u64) -> String {
    hex16(fnv1a_words([
        key.seed,
        key.bite,
        key.iteration,
        key.worker,
        proposals,
    ]))
}

// ------------------------------------------------------------- the column --

/// **The column, from committed geometry and row identities** (Astra 5b
/// Q6: "a core-member rearrangement releases the original bottom-to-top
/// branches, which do not re-form before band entry. Record temporary
/// releases separately. If the treatment prevents the column from forming,
/// report column avoided").
///
/// Definitions, as [`analyse_column`] computes them:
///
/// * The **blocking graph** at an iteration has the pieces and the four
///   strip edges as vertices and the rows with violation `> 0` as edges: a
///   pair row joins two pieces, a boundary row joins a piece to its edge.
/// * The **column** is a simple path in the entry blocking graph
///   (iteration 0, the capsule's own blocking rows) from the bottom edge
///   (some piece's `B` row) to the top edge (some piece's `T` row). All such
///   paths are enumerated (`paths`, at most [`COLUMN_PATH_CAP`],
///   `pathsTruncated` if more); the column's **rows** are their union and
///   the **core members** the pieces on them. If no path exists at entry,
///   the first iteration with one gives `formed-at-iteration`; if none ever
///   forms before the stop, `status = "avoided"`.
/// * A column row is **released** at iteration `k` if its violation is
///   `> 0` at `k - 1` and `<= 0` at `k`. The release is **temporary** if
///   the row is `> 0` again at a later iteration before the stop,
///   **permanent** otherwise.
/// * The **break** is the first iteration `k` at which a permanent release
///   of a column row happens and the original column rows still positive
///   at `k` no longer connect bottom to top. `reformedAfterBreak` says
///   whether the original rows reconnect bottom to top at any later
///   iteration (then Astra's "do not re-form before band entry" fails and
///   the reader sees it); `firstDisconnectedAtIteration` is the first
///   iteration at which the original rows do not connect, whatever kind of
///   release caused it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnReport {
    /// `formed-at-entry`, `formed-at-iteration` or `avoided`.
    pub status: &'static str,
    pub formed_at_iteration: Option<u64>,
    pub paths: Vec<Vec<RowId>>,
    pub paths_truncated: bool,
    pub rows: Vec<RowId>,
    pub core_members: Vec<u32>,
    /// Consecutive iterations from the formation (inclusive) over which the
    /// original rows keep connecting bottom to top: the column's residence.
    pub residence_iterations: u64,
    pub break_iteration: Option<u64>,
    pub break_release: Option<BreakRelease>,
    pub first_disconnected_at_iteration: Option<u64>,
    pub reformed_after_first_disconnection: bool,
    pub temporary_releases: Vec<TemporaryRelease>,
    pub permanent_releases: Vec<PermanentRelease>,
    pub reformed_after_break: bool,
    pub band_entry_iteration: Option<u64>,
    /// The last iteration the analysis saw (the stop).
    pub last_iteration: u64,
    /// Per iteration from entry (index 0): do the original column rows
    /// still positive connect bottom to top?
    pub original_rows_connected: Vec<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakRelease {
    pub row_id: RowId,
    pub iteration: u64,
    /// The winner's relocate that committed the release (the last relocate
    /// of the sweep whose changed rows take this row to `<= 0`); `null` if
    /// the sweep's relocates were not available.
    pub relocate: Option<ReleaseRelocate>,
    /// `[rowId, weight]` for every column row after the break iteration.
    pub row_weights_at_break: Vec<(RowId, f64)>,
    pub max_column_row_weight_at_break: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseRelocate {
    pub piece: u32,
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub dtheta_deg: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporaryRelease {
    pub row_id: RowId,
    pub released_at: u64,
    pub reformed_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermanentRelease {
    pub row_id: RowId,
    pub iteration: u64,
}

/// The most bottom-to-top paths [`analyse_column`] enumerates.
pub const COLUMN_PATH_CAP: usize = 256;

/// What [`analyse_column`] reads: the positive rows per iteration from the
/// entry state (index 0), and optionally the row weights per iteration
/// (row-id indexed, index 0 = entry) and the winner's relocates per
/// iteration (index 0 = iteration 1). Empty weight or relocate slices are
/// allowed (a hand-built test state has neither).
pub struct ColumnInput<'a> {
    pub count: usize,
    pub blocking: &'a [Vec<BlockingRow>],
    pub weights: &'a [Vec<f64>],
    pub relocates: &'a [Vec<ReplayRelocate>],
    pub band_entry_iteration: Option<u64>,
}

/// The two vertices of a row in the blocking graph: pieces are `0..count`,
/// the four strip edges `count + side`.
fn row_vertices(count: usize, id: RowId) -> (usize, usize) {
    match decode_row_id(count, id) {
        RowKind::Pair { first, second } => (first, second),
        RowKind::Boundary { piece, side } => (piece, count + side),
    }
}

#[allow(clippy::too_many_arguments)]
fn walk_paths(
    at: usize,
    top: usize,
    adjacency: &[Vec<(usize, RowId)>],
    visited: &mut [bool],
    path: &mut Vec<RowId>,
    paths: &mut Vec<Vec<RowId>>,
    cap: usize,
    truncated: &mut bool,
) {
    if at == top {
        if paths.len() < cap {
            paths.push(path.clone());
        } else {
            *truncated = true;
        }
        return;
    }
    visited[at] = true;
    for &(next, id) in &adjacency[at] {
        if visited[next] || *truncated {
            continue;
        }
        path.push(id);
        walk_paths(next, top, adjacency, visited, path, paths, cap, truncated);
        path.pop();
    }
    visited[at] = false;
}

/// Every simple path from the bottom edge to the top edge through `rows`,
/// each as its row ids in path order, at most `cap` of them (the second
/// value says whether more exist).
pub fn bottom_to_top_paths(count: usize, rows: &[RowId], cap: usize) -> (Vec<Vec<RowId>>, bool) {
    let vertices = count + 4;
    let bottom = count + EDGE_BOTTOM;
    let top = count + EDGE_TOP;
    let mut adjacency: Vec<Vec<(usize, RowId)>> = vec![Vec::new(); vertices];
    for &id in rows {
        let (u, v) = row_vertices(count, id);
        adjacency[u].push((v, id));
        adjacency[v].push((u, id));
    }
    for list in &mut adjacency {
        list.sort_unstable_by_key(|entry| entry.1);
    }
    let mut paths = Vec::new();
    let mut truncated = false;
    let mut visited = vec![false; vertices];
    let mut path: Vec<RowId> = Vec::new();
    walk_paths(
        bottom,
        top,
        &adjacency,
        &mut visited,
        &mut path,
        &mut paths,
        cap,
        &mut truncated,
    );
    (paths, truncated)
}

/// Does the bottom edge reach the top edge through `rows`?
pub fn bottom_to_top_connected(count: usize, rows: &[RowId]) -> bool {
    let vertices = count + 4;
    let bottom = count + EDGE_BOTTOM;
    let top = count + EDGE_TOP;
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); vertices];
    for &id in rows {
        let (u, v) = row_vertices(count, id);
        adjacency[u].push(v);
        adjacency[v].push(u);
    }
    let mut seen = vec![false; vertices];
    let mut stack = vec![bottom];
    seen[bottom] = true;
    while let Some(at) = stack.pop() {
        if at == top {
            return true;
        }
        for &next in &adjacency[at] {
            if !seen[next] {
                seen[next] = true;
                stack.push(next);
            }
        }
    }
    false
}

fn positive_ids(rows: &[BlockingRow]) -> Vec<RowId> {
    rows.iter()
        .filter(|row| row.1 > 0.0)
        .map(|row| row.0)
        .collect()
}

fn column_avoided(input: &ColumnInput<'_>) -> ColumnReport {
    ColumnReport {
        status: "avoided",
        formed_at_iteration: None,
        paths: Vec::new(),
        paths_truncated: false,
        rows: Vec::new(),
        core_members: Vec::new(),
        residence_iterations: 0,
        break_iteration: None,
        break_release: None,
        first_disconnected_at_iteration: None,
        reformed_after_first_disconnection: false,
        temporary_releases: Vec::new(),
        permanent_releases: Vec::new(),
        reformed_after_break: false,
        band_entry_iteration: input.band_entry_iteration,
        last_iteration: input.blocking.len().saturating_sub(1) as u64,
        original_rows_connected: Vec::new(),
    }
}

/// **The column analysis** (see [`ColumnReport`] for the definitions). Pure:
/// reads the per-iteration positive rows the replay left behind and
/// nothing else, so a hand-built sequence of blocking sets is analysed the
/// same way as a replay's. The column is the entry blocking graph's
/// bottom-to-top path set, or the first iteration's that has one.
pub fn analyse_column(input: &ColumnInput<'_>) -> ColumnReport {
    (0..input.blocking.len())
        .find_map(|formed_at| analyse_column_from(input, formed_at))
        .unwrap_or_else(|| column_avoided(input))
}

/// [`analyse_column`] with the column taken from the iteration whose
/// bottom-to-top path set **lives longest**: for every iteration with a
/// path set, the residence is the number of consecutive iterations from
/// it over which that set's rows still connect bottom to top; the longest
/// residence wins, the earliest on a tie. On the bite-15 capsule the
/// entry graph's only bottom-to-top path is 2-8-45 (released at 29) while
/// the README's 0-6-43/44 column, the one Astra's "break 37" names, first
/// connects bottom to top at iteration 1 and stays for 36 iterations; this
/// reading is the one that finds it without naming its pieces.
pub fn analyse_longest_lived_column(input: &ColumnInput<'_>) -> ColumnReport {
    let mut best: Option<ColumnReport> = None;
    for formed_at in 0..input.blocking.len() {
        let Some(report) = analyse_column_from(input, formed_at) else {
            continue;
        };
        let longer = best
            .as_ref()
            .is_none_or(|incumbent| report.residence_iterations > incumbent.residence_iterations);
        if longer {
            best = Some(report);
        }
    }
    best.unwrap_or_else(|| column_avoided(input))
}

/// The analysis with the column defined by the bottom-to-top paths of
/// iteration `formed_at`; `None` if that iteration has none.
fn analyse_column_from(input: &ColumnInput<'_>, formed_at: usize) -> Option<ColumnReport> {
    let count = input.count;
    let frames: Vec<Vec<RowId>> = input.blocking.iter().map(|rows| positive_ids(rows)).collect();
    let last_iteration = frames.len().saturating_sub(1) as u64;
    let (paths, truncated) = bottom_to_top_paths(count, frames.get(formed_at)?, COLUMN_PATH_CAP);
    if paths.is_empty() {
        return None;
    }
    let formed_at = formed_at as u64;
    let status = if formed_at == 0 {
        "formed-at-entry"
    } else {
        "formed-at-iteration"
    };
    let rows: Vec<RowId> = paths
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<RowId>>()
        .into_iter()
        .collect();
    let core_members: Vec<u32> = rows
        .iter()
        .flat_map(|&id| match decode_row_id(count, id) {
            RowKind::Pair { first, second } => vec![first as u32, second as u32],
            RowKind::Boundary { piece, .. } => vec![piece as u32],
        })
        .collect::<BTreeSet<u32>>()
        .into_iter()
        .collect();
    // Per iteration: which column rows are positive, and whether the
    // original rows still connect bottom to top.
    let positive: Vec<Vec<bool>> = frames
        .iter()
        .map(|frame| rows.iter().map(|id| frame.contains(id)).collect())
        .collect();
    let original_rows_connected: Vec<bool> = positive
        .iter()
        .map(|flags| {
            let live: Vec<RowId> = rows
                .iter()
                .zip(flags)
                .filter(|(_, on)| **on)
                .map(|(id, _)| *id)
                .collect();
            bottom_to_top_connected(count, &live)
        })
        .collect();
    // Releases, from the formation onward.
    let mut temporary_releases = Vec::new();
    let mut permanent_releases = Vec::new();
    for (index, &id) in rows.iter().enumerate() {
        for k in (formed_at as usize + 1)..frames.len() {
            if positive[k - 1][index] && !positive[k][index] {
                let reformed = ((k + 1)..frames.len()).find(|&later| positive[later][index]);
                match reformed {
                    Some(later) => temporary_releases.push(TemporaryRelease {
                        row_id: id,
                        released_at: k as u64,
                        reformed_at: later as u64,
                    }),
                    None => permanent_releases.push(PermanentRelease {
                        row_id: id,
                        iteration: k as u64,
                    }),
                }
            }
        }
    }
    temporary_releases.sort_by_key(|release| (release.released_at, release.row_id));
    permanent_releases.sort_by_key(|release| (release.iteration, release.row_id));
    let first_disconnected = original_rows_connected
        .iter()
        .enumerate()
        .skip(formed_at as usize)
        .find(|(_, connected)| !**connected)
        .map(|(iteration, _)| iteration as u64);
    let reformed_after = |from: u64| {
        original_rows_connected
            .iter()
            .skip(from as usize + 1)
            .any(|connected| *connected)
    };
    let reformed_after_first_disconnection = first_disconnected.is_some_and(reformed_after);
    let residence_iterations = original_rows_connected
        .iter()
        .skip(formed_at as usize)
        .take_while(|connected| **connected)
        .count() as u64;
    let break_release = permanent_releases
        .iter()
        .find(|release| !original_rows_connected[release.iteration as usize])
        .copied();
    let break_iteration = break_release.map(|release| release.iteration);
    let reformed_after_break = break_iteration.is_some_and(reformed_after);
    let break_release = break_release.map(|release| {
        let k = release.iteration as usize;
        let weights = input.weights.get(k);
        let row_weights_at_break: Vec<(RowId, f64)> = rows
            .iter()
            .map(|&id| {
                (
                    id,
                    weights
                        .and_then(|weights| weights.get(id as usize))
                        .copied()
                        .unwrap_or(f64::NAN),
                )
            })
            .collect();
        let max_column_row_weight_at_break = row_weights_at_break
            .iter()
            .fold(0.0f64, |acc, (_, weight)| {
                if weight.is_nan() {
                    acc
                } else {
                    acc.max(*weight)
                }
            });
        let relocate = input
            .relocates
            .get(k - 1)
            .and_then(|sweep| {
                sweep.iter().rev().find(|relocate| {
                    relocate
                        .rows
                        .iter()
                        .any(|delta| delta.0 == release.row_id && delta.2 <= 0.0)
                })
            })
            .map(|relocate| ReleaseRelocate {
                piece: relocate.piece,
                dx_mm: relocate.dx_mm,
                dy_mm: relocate.dy_mm,
                dtheta_deg: relocate.dtheta_deg,
            });
        BreakRelease {
            row_id: release.row_id,
            iteration: release.iteration,
            relocate,
            row_weights_at_break,
            max_column_row_weight_at_break,
        }
    });
    Some(ColumnReport {
        status,
        formed_at_iteration: Some(formed_at),
        paths,
        paths_truncated: truncated,
        rows,
        core_members,
        residence_iterations,
        break_iteration,
        break_release,
        first_disconnected_at_iteration: first_disconnected,
        reformed_after_first_disconnection,
        temporary_releases,
        permanent_releases,
        reformed_after_break,
        band_entry_iteration: input.band_entry_iteration,
        last_iteration,
        original_rows_connected,
    })
}

impl ColumnReport {
    /// The one-line stderr summary; `label` is `column` or
    /// `column longest-lived`.
    pub fn summary(&self, label: &str) -> String {
        format!(
            "replay {} (status {}, formed at {:?}, paths {}{}, rows {}, core members {:?}, \
             residence {}, break at {:?}, max column weight at break {}, first disconnected at \
             {:?}, reformed after first disconnection {}, temporary releases {}, permanent \
             releases {}, reformed after break {}, band entry {:?})",
            label,
            self.status,
            self.formed_at_iteration,
            self.paths.len(),
            if self.paths_truncated { "+" } else { "" },
            self.rows.len(),
            self.core_members,
            self.residence_iterations,
            self.break_iteration,
            self.break_release
                .as_ref()
                .map_or("n/a".to_owned(), |release| format!(
                    "{:.3e} (row {} by piece {:?})",
                    release.max_column_row_weight_at_break,
                    release.row_id,
                    release.relocate.map(|relocate| relocate.piece)
                )),
            self.first_disconnected_at_iteration,
            self.reformed_after_first_disconnection,
            self.temporary_releases.len(),
            self.permanent_releases.len(),
            self.reformed_after_break,
            self.band_entry_iteration,
        )
    }
}

// --------------------------------------------------------------- the fork --

/// The four exponents the fork re-scores under, in the order of
/// [`ForkGuided`]'s fields.
pub const FORK_EXPONENTS: [f64; 4] = [2.0, 1.0, 0.75, 0.5];

/// One pose's guided incident total under the four exponents, at the same
/// installed pose and the same weights: `energy::incident_totals_with_exponent`
/// four times over the state's rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct ForkGuided {
    #[serde(rename = "2")]
    pub p2: f64,
    #[serde(rename = "1")]
    pub p1: f64,
    #[serde(rename = "0.75")]
    pub p075: f64,
    #[serde(rename = "0.5")]
    pub p05: f64,
}

impl ForkGuided {
    /// `(raw, guided under the four exponents)` at the state's current pose
    /// of `piece`. No counter moves.
    pub fn of(state: &IcsState, piece: usize) -> (f64, Self) {
        let (raw, p2) = incident_totals_with_exponent(state, piece, FORK_EXPONENTS[0]);
        let (_, p1) = incident_totals_with_exponent(state, piece, FORK_EXPONENTS[1]);
        let (_, p075) = incident_totals_with_exponent(state, piece, FORK_EXPONENTS[2]);
        let (_, p05) = incident_totals_with_exponent(state, piece, FORK_EXPONENTS[3]);
        (raw, Self { p2, p1, p075, p05 })
    }

    pub fn at(self, index: usize) -> f64 {
        [self.p2, self.p1, self.p075, self.p05][index]
    }
}

/// `[rowId, violationMm, weight]` of one incident row at a fork pose: the
/// piece's near rows in ascending other-piece order, then its four edge
/// rows `L R B T` (signed), i.e. exactly the rows and the order
/// `incident_totals_with_exponent` folds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ForkRow(pub RowId, pub f64, pub f64);

/// The incident rows of `piece` at its current pose, as [`ForkRow`]s.
pub fn fork_rows(state: &IcsState, piece: usize) -> Vec<ForkRow> {
    let count = state.poses.len();
    let mut rows = Vec::with_capacity(state.near[piece].len() + 4);
    for &other in &state.near[piece] {
        let id = super::microscope::pair_row_id(count, piece, other as usize);
        let row = &state.pair_rows[id as usize];
        rows.push(ForkRow(id, row.violation_mm, row.weight));
    }
    for (side, row) in state.edge_rows[piece].iter().enumerate() {
        rows.push(ForkRow(
            boundary_row_id(count, piece, side),
            row.violation_mm,
            row.weight,
        ));
    }
    rows
}

/// One candidate pose of a forked relocate: one of the 25 focused or 50
/// container samples, or a finalist's pose after its coarse walk.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkCandidate {
    /// `focused`, `container` or `finalist`.
    pub kind: &'static str,
    pub x_mm: f64,
    pub y_mm: f64,
    pub theta_deg: f64,
    pub raw: f64,
    pub guided: ForkGuided,
    /// The incident rows, finalists only (the samples would triple the
    /// document).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<ForkRow>,
}

/// The stay pose (the relocate's entry pose) scored the same way.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkStay {
    pub x_mm: f64,
    pub y_mm: f64,
    pub theta_deg: f64,
    pub raw: f64,
    pub guided: ForkGuided,
    pub rows: Vec<ForkRow>,
}

/// The pose the relocate actually committed (the probe's own exponent
/// decided it), scored under the four exponents.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkCommitted {
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub dtheta_deg: f64,
    pub moved: bool,
    pub origin: &'static str,
    pub raw: f64,
    pub guided: ForkGuided,
    pub rows: Vec<ForkRow>,
}

/// Per exponent, the index into `candidates` of the candidate that beats
/// the stay pose under the lexicographic rule "`raw == 0` beats any
/// positive; else lower guided", the best such candidate (first of a tie);
/// `null` when the stay pose wins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct ForkBestUnder {
    #[serde(rename = "2")]
    pub p2: Option<u32>,
    #[serde(rename = "1")]
    pub p1: Option<u32>,
    #[serde(rename = "0.75")]
    pub p075: Option<u32>,
    #[serde(rename = "0.5")]
    pub p05: Option<u32>,
}

impl ForkBestUnder {
    pub fn at(self, index: usize) -> Option<u32> {
        [self.p2, self.p1, self.p075, self.p05][index]
    }
}

/// One relocate of the fork sweep, every worker.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkRelocate {
    pub worker: u32,
    pub piece: u32,
    /// Position in the worker's sweep order.
    pub position: u32,
    pub stay: ForkStay,
    pub candidates: Vec<ForkCandidate>,
    pub best_under: ForkBestUnder,
    pub committed: ForkCommitted,
}

/// The lexicographic rule of `relocate::eval_cmp` on `(raw, guided)`:
/// `Less` if `left` is strictly better.
fn fork_cmp(left: (f64, f64), right: (f64, f64)) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (left.0 <= 0.0, right.0 <= 0.0) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left.1.partial_cmp(&right.1).unwrap_or(Ordering::Equal),
    }
}

impl ForkRelocate {
    /// Opens the record at the relocate's entry: the state is at the entry
    /// pose with its rows current.
    pub fn begin(worker: u32, piece: usize, entry: Pose, state: &IcsState) -> Self {
        let (raw, guided) = ForkGuided::of(state, piece);
        Self {
            worker,
            piece: piece as u32,
            position: 0,
            stay: ForkStay {
                x_mm: entry.tx_mm,
                y_mm: entry.ty_mm,
                theta_deg: entry.theta_deg,
                raw,
                guided,
                rows: fork_rows(state, piece),
            },
            candidates: Vec::new(),
            best_under: ForkBestUnder::default(),
            committed: ForkCommitted {
                dx_mm: 0.0,
                dy_mm: 0.0,
                dtheta_deg: 0.0,
                moved: false,
                origin: "stayPut",
                raw,
                guided,
                rows: Vec::new(),
            },
        }
    }

    /// Scores the pose currently installed in the state for `piece`.
    pub fn observe_candidate(&mut self, kind: &'static str, pose: Pose, state: &IcsState) {
        let piece = self.piece as usize;
        let (raw, guided) = ForkGuided::of(state, piece);
        self.candidates.push(ForkCandidate {
            kind,
            x_mm: pose.tx_mm,
            y_mm: pose.ty_mm,
            theta_deg: pose.theta_deg,
            raw,
            guided,
            rows: if kind == "finalist" {
                fork_rows(state, piece)
            } else {
                Vec::new()
            },
        });
    }

    /// Closes the record after the relocate's final install.
    pub fn finish(
        &mut self,
        dx_mm: f64,
        dy_mm: f64,
        dtheta_deg: f64,
        moved: bool,
        origin: &'static str,
        state: &IcsState,
    ) {
        let piece = self.piece as usize;
        let (raw, guided) = ForkGuided::of(state, piece);
        self.committed = ForkCommitted {
            dx_mm,
            dy_mm,
            dtheta_deg,
            moved,
            origin,
            raw,
            guided,
            rows: fork_rows(state, piece),
        };
        let mut best = [None; 4];
        for (index, slot) in best.iter_mut().enumerate() {
            let stay = (self.stay.raw, self.stay.guided.at(index));
            let mut winner: Option<(u32, (f64, f64))> = None;
            for (candidate_index, candidate) in self.candidates.iter().enumerate() {
                let score = (candidate.raw, candidate.guided.at(index));
                if !score.0.is_finite() || !score.1.is_finite() {
                    continue;
                }
                let better = match winner {
                    None => true,
                    Some((_, incumbent)) => {
                        fork_cmp(score, incumbent) == std::cmp::Ordering::Less
                    }
                };
                if better {
                    winner = Some((candidate_index as u32, score));
                }
            }
            *slot = winner
                .filter(|(_, score)| fork_cmp(*score, stay) == std::cmp::Ordering::Less)
                .map(|(candidate_index, _)| candidate_index);
        }
        self.best_under = ForkBestUnder {
            p2: best[0],
            p1: best[1],
            p075: best[2],
            p05: best[3],
        };
    }
}

/// One worker's fork records for the fork sweep.
#[derive(Clone, Debug, Default)]
pub struct ForkSink {
    pub worker: u32,
    pub relocates: Vec<ForkRelocate>,
}

/// The fork sweep's report.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkReport {
    /// The sweep the fork ran in (`--fork + 1`).
    pub sweep: u64,
    /// Always `true` here: the replay ran the fork sweep. The field exists
    /// so a reader of `replay.fork` never infers reachability from the
    /// presence of `relocates` ([`ForkNotReached`] carries `false`).
    pub reached: bool,
    pub exponents: [f64; 4],
    pub relocates: Vec<ForkRelocate>,
    /// Relocates with a candidate beating the stay pose, per exponent.
    pub would_move: ForkWouldMove,
    /// The probe's own exponent that decided the trajectory (2 for the
    /// control).
    pub deciding_exponent: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct ForkWouldMove {
    #[serde(rename = "2")]
    pub p2: u64,
    #[serde(rename = "1")]
    pub p1: u64,
    #[serde(rename = "0.75")]
    pub p075: u64,
    #[serde(rename = "0.5")]
    pub p05: u64,
}

impl ForkReport {
    pub fn summary(&self) -> String {
        format!(
            "replay fork (sweep {}: {} relocates; would move under p=2: {}, p=1: {}, p=0.75: {}, \
             p=0.5: {})",
            self.sweep,
            self.relocates.len(),
            self.would_move.p2,
            self.would_move.p1,
            self.would_move.p075,
            self.would_move.p05,
        )
    }
}

/// **The fork the replay never reached.** `--fork=<sweep>` names a sweep the
/// run may stop before - band entry, the iteration cap or a strike-out comes
/// first - and a document that then carried `fork: null` beside
/// `stop != "fork"` said so only by omission, which a reader scoring the
/// fork's readings could mistake for "no fork was asked for". This says it:
/// the sweep the fork would have run in, `reached: false`, the stop the
/// replay made instead and how many iterations it ran. No relocates: none
/// were forked.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkNotReached {
    /// The sweep the fork would have run in (`--fork + 1`).
    pub sweep: u64,
    /// Always `false` here.
    pub reached: bool,
    /// The replay's own stop: `band-entry`, `iteration-cap` or `struck`.
    pub stop: &'static str,
    /// Iterations the replay ran before that stop.
    pub iterations: u64,
}

/// `replay.fork` when `--fork=<sweep>` was named: the fork sweep's report,
/// or the statement that the replay never reached it. Untagged, so the
/// reached case's document shape is [`ForkReport`]'s (with `reached: true`)
/// and the other is `{sweep, reached: false, stop, iterations}`;
/// `docs/experiments/overlap-ics/bite-microscope/column-break.py` reads
/// both.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ForkOutcome {
    Reached(ForkReport),
    NotReached(ForkNotReached),
}

impl ForkOutcome {
    /// The fork sweep's report, when the replay ran it.
    pub fn reached(&self) -> Option<&ForkReport> {
        match self {
            Self::Reached(report) => Some(report),
            Self::NotReached(_) => None,
        }
    }

    /// The benchmark's stderr line for either case.
    pub fn summary(&self) -> String {
        match self {
            Self::Reached(report) => report.summary(),
            Self::NotReached(fork) => format!(
                "replay fork (sweep {} not reached: stopped {} after {} iterations)",
                fork.sweep, fork.stop, fork.iterations
            ),
        }
    }
}

// ------------------------------------------------------- the certification --

/// The exact checkpoint the live path recorded, copied field for field
/// from `diagnostics::ExactCheckpoint`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointFields {
    pub proposal_ordinal: u64,
    pub target_depth_mm: f64,
    pub max_violation_mm: f64,
    pub proxy_raw_depth_mm: f64,
    pub kernel_exclusive_valid: bool,
    pub contract_valid: bool,
    pub repair_rows: u64,
    pub repair_max_displacement_mm: f64,
    pub repair_depth_giveback_mm: f64,
    pub published_raw_depth_mm: Option<f64>,
    pub refusal: Option<String>,
    pub first_scan_failing_pairs: u32,
    pub first_scan_failing_boundaries: u32,
    pub blocked_on: Option<&'static str>,
    pub blocking_shortfall_um: Option<i64>,
    pub first_pair: Option<(u32, u32)>,
    pub first_pair_kernel_shortfall_um: Option<i64>,
    pub first_pair_proxy_violation_um: Option<f64>,
}

impl From<&super::diagnostics::ExactCheckpoint> for CheckpointFields {
    fn from(checkpoint: &super::diagnostics::ExactCheckpoint) -> Self {
        Self {
            proposal_ordinal: checkpoint.proposal_ordinal,
            target_depth_mm: checkpoint.target_depth_mm,
            max_violation_mm: checkpoint.max_violation_mm,
            proxy_raw_depth_mm: checkpoint.proxy_raw_depth_mm,
            kernel_exclusive_valid: checkpoint.kernel_exclusive_valid,
            contract_valid: checkpoint.contract_valid,
            repair_rows: checkpoint.repair_rows,
            repair_max_displacement_mm: checkpoint.repair_max_displacement_mm,
            repair_depth_giveback_mm: checkpoint.repair_depth_giveback_mm,
            published_raw_depth_mm: checkpoint.published_raw_depth_mm,
            refusal: checkpoint.refusal.clone(),
            first_scan_failing_pairs: checkpoint.first_scan_failing_pairs,
            first_scan_failing_boundaries: checkpoint.first_scan_failing_boundaries,
            blocked_on: checkpoint.blocked_on,
            blocking_shortfall_um: checkpoint.blocking_shortfall_um,
            first_pair: checkpoint.first_pair,
            first_pair_kernel_shortfall_um: checkpoint.first_pair_kernel_shortfall_um,
            first_pair_proxy_violation_um: checkpoint.first_pair_proxy_violation_um,
        }
    }
}

/// **The detached publication check** (`--certify=1`; Astra 5b Q6: "the
/// implemented replay stops at band entry and makes no exact calls. Its
/// numbers establish neither certification nor publication. Require a
/// detached check of the unchanged publication path before promoting any
/// band-entry result to the certification claim").
///
/// After the replay has stopped at band entry, [`Engine::attempt_publication`]
/// is called once on the band-entry state: the very method the live loop
/// calls when its band test passes (`mod.rs::separate`, "The band test
/// comes first"), which runs `publish::attempt` and, inside it, the
/// untouched `validate_placements_against_contract`. It reads the state
/// through `&self.state`, so the state is not cloned and not moved; the
/// engine's checkpoint trace and its incumbent receive the result as they
/// would live, and nothing is installed (`install_publication` is not
/// called; the continuous state is the band-entry state afterwards). The
/// incumbent of a replay engine is a placeholder at infinite depth that
/// nothing reads again and nothing writes out, so the improvement gate
/// (`proxy > incumbent - 1 um`) never refuses here and `improvedIncumbent`
/// says only that the exact authorities accepted; the proxy-above-target
/// refusal, the kernel, the repair and the contract validation are the
/// live ones. A publication therefore exists in `replay.certification` of
/// the refused document and nowhere else.
///
/// The `refusal` is read, never derived: it is the checkpoint's own string
/// when the live path pushed a checkpoint and no publication, `null` when
/// it published, and [`LIVE_PATH_RETURNED_NOTHING`] verbatim when the live
/// path returned neither - see that constant for why no gate is named.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificationReport {
    pub attempted: bool,
    /// Why not, when `attempted` is false.
    pub reason: Option<String>,
    pub published: bool,
    /// The checkpoint's own refusal string; `null` when published;
    /// [`LIVE_PATH_RETURNED_NOTHING`] when the live path pushed no
    /// checkpoint (then `exactCalls` is 0 beside it).
    pub refusal: Option<String>,
    /// The published raw source depth, when published.
    pub depth_mm: Option<f64>,
    pub proxy_depth_mm: f64,
    pub target_depth_mm: f64,
    pub incumbent_depth_mm: f64,
    pub improved_incumbent: bool,
    /// `work.exact_checkpoints` charged by the call.
    pub exact_calls: u64,
    pub max_violation_mm: f64,
    pub checkpoint: Option<CheckpointFields>,
    pub path: &'static str,
}

/// **What the live path returned when it returned neither a publication nor
/// a checkpoint**, recorded verbatim as `replay.certification.refusal`.
/// `Engine::attempt_publication` answers `CheckpointOutcome::none()` and
/// pushes no checkpoint when `publish::attempt` returns `None` at one of
/// its entry gates - the band test, the closed member's `proxy > T`, the
/// improvement gate - before `work.exact_checkpoints` is charged, and when
/// its own unchanged-pose digest skips the attempt. None of those gates
/// produces a string, so the report cannot read one, and it must not
/// re-derive one (a re-derivation would be this module's opinion of the
/// gates, not the live path's answer, and could disagree with it). What is
/// recorded is the return itself; the reader tells the case from
/// `exactCalls == 0`, `checkpoint: null`, `proxyDepthMm` and
/// `targetDepthMm` beside it.
pub const LIVE_PATH_RETURNED_NOTHING: &str = "Engine::attempt_publication returned \
    CheckpointOutcome { publication: None, improved: false } and pushed no checkpoint: \
    publish::attempt refused at an entry gate before any exact call (or the unchanged-pose \
    digest skipped the attempt); the live path names no reason there";

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
    /// The exponent probe's `p`; `null` for the other probes and the control.
    pub exponent: Option<f64>,
    pub entry_raw: f64,
    pub entry_guided: f64,
    pub entry_max_mm: f64,
    pub entry_blocking: Vec<BlockingRow>,
    pub entry_poses_fingerprint: String,
    pub entry_weights_fingerprint: String,
    pub entry_stream_fingerprint: String,
    /// `band-entry`, `iteration-cap`, `struck` or `fork`.
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
    /// The spec's column: the entry blocking graph's bottom-to-top paths
    /// ([`analyse_column`]).
    pub column: ColumnReport,
    /// The longest-lived column ([`analyse_longest_lived_column`]).
    pub column_longest_lived: ColumnReport,
    /// `null` unless `--fork=<sweep>`; then the fork sweep's report or
    /// [`ForkNotReached`] ([`ForkOutcome`]).
    pub fork: Option<ForkOutcome>,
    /// `null` unless `--certify=1`.
    pub certification: Option<CertificationReport>,
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
/// live `Slot` without its feature-gated instruments, plus the probe stats,
/// the microscope's sweep trace (the committed relocates) and, in the fork
/// sweep, the fork sink.
struct ReplaySlot {
    state: IcsState,
    descent: Descent,
    work: super::diagnostics::WorkVector,
    stats: ReplaySweepStats,
    trace: SweepTrace,
    fork: Option<ForkSink>,
}

/// What one replay tournament hands back.
struct ReplayTournament {
    totals: Totals,
    winner: usize,
    contested: bool,
    all_stats: ReplaySweepStats,
    winner_stats: ReplaySweepStats,
    all_evaluations: u64,
    winner_evaluations: u64,
    winner_trace: SweepTrace,
    forks: Vec<ForkSink>,
}

/// The state's rows after an iteration, row-id indexed: the signed
/// violations and the weights, and the poses. Kept in memory for the
/// column readings and emitted only for the column's rows and the core
/// members.
struct RowSnapshot {
    violations: Vec<f64>,
    weights: Vec<f64>,
    poses: Vec<Pose>,
}

impl RowSnapshot {
    fn of(state: &IcsState) -> Self {
        let rows = state.pair_rows.len() + 4 * state.edge_rows.len();
        let mut violations = Vec::with_capacity(rows);
        let mut weights = Vec::with_capacity(rows);
        for row in &state.pair_rows {
            violations.push(row.violation_mm);
            weights.push(row.weight);
        }
        for rows in &state.edge_rows {
            for row in rows {
                violations.push(row.violation_mm);
                weights.push(row.weight);
            }
        }
        Self {
            violations,
            weights,
            poses: state.poses.clone(),
        }
    }
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
    /// without calling the exact authorities (unless `--certify=1`, which
    /// calls them once *after* the stop), and its cap is
    /// `params.max_iterations`. See the module doc.
    pub fn replay_separation(&mut self, params: &ReplayParams) -> ReplayReport {
        let band = self.config.limits.band_mm;
        let probe = ReplayProbeConfig::of(params.probe, band);
        let workers = params.workers.max(1);
        let count = self.state.poses.len();
        debug_assert!(
            params.fork.is_none() || (probe.continuation.is_none() && !probe.revisit),
            "the fork runs only under --probe=none|exponent:<p>"
        );
        // The entry reading's `guided` is the quantity the probe ranks on;
        // `raw` and `max` are the fold's own either way.
        let entry = match probe.exponent {
            Some(exponent) => energy::fold_with_exponent(&self.state, exponent),
            None => energy::fold(&self.state),
        };
        let entry_blocking = blocking_rows(&self.state);
        let entry_proposals = self.descent.proposals;
        let entry_poses_fingerprint = poses_fingerprint(&self.state.poses);
        let entry_weights_fingerprint = weights_fingerprint(&self.state);
        let entry_stream_fingerprint =
            stream_fingerprint(self.descent.stream_key().into(), entry_proposals);
        let mut snapshots: Vec<RowSnapshot> = vec![RowSnapshot::of(&self.state)];
        let mut snapshot = self.state.clone();
        let mut meter = StrikeMeter::for_phase(params.strikes, Phase::Explore, entry.raw);
        let mut batch_sample_evaluations = 0u64;
        let mut iterations = 0u64;
        let mut rollbacks = Vec::new();
        let mut records: Vec<ReplayIteration> = Vec::new();
        let mut band_entered_at = None;
        let mut evaluations_cumulative = 0u64;
        let mut evaluations_to_band = None;
        let mut fork_report: Option<ForkReport> = None;
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
            // stops here and reports (and, under `--certify=1`, makes that
            // one call after the stop: `certify_band_entry`).
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
            let forking = params.fork == Some(iterations);
            let samples_before = self.trace.work.sample_evaluations;
            let tournament = self.replay_tournament(workers, params.bite, &probe, forking);
            iterations += 1;
            batch_sample_evaluations = self.trace.work.sample_evaluations - samples_before;
            evaluations_cumulative += tournament.all_evaluations;
            let stream: StreamKey = self.descent.stream_key().into();
            let proposals = self.descent.proposals;
            snapshots.push(RowSnapshot::of(&self.state));
            records.push(ReplayIteration {
                iteration: iterations,
                raw_after: tournament.totals.raw,
                guided_after: tournament.totals.guided,
                max_after_mm: tournament.totals.max_violation_mm,
                winner: tournament.winner as u32,
                contested: tournament.contested,
                new_minimum: false,
                blocking: blocking_rows(&self.state),
                evaluations_all_workers: tournament.all_evaluations,
                evaluations_winner: tournament.winner_evaluations,
                continuation_evaluations: tournament.all_stats.continuation_evaluations,
                queued_relocates: tournament.winner_stats.queued_relocates,
                queued_relocates_all_workers: tournament.all_stats.queued_relocates,
                stats_all_workers: tournament.all_stats,
                stats_winner: tournament.winner_stats,
                relocates: tournament
                    .winner_trace
                    .relocates
                    .iter()
                    .map(ReplayRelocate::from)
                    .collect(),
                column_rows: Vec::new(),
                core_poses: Vec::new(),
                poses_fingerprint: poses_fingerprint(&self.state.poses),
                weights_fingerprint: weights_fingerprint(&self.state),
                stream_fingerprint: stream_fingerprint(stream, proposals),
                stream,
                proposals,
            });
            if forking {
                let mut relocates: Vec<ForkRelocate> = Vec::new();
                for sink in tournament.forks {
                    relocates.extend(sink.relocates);
                }
                let mut would_move = ForkWouldMove::default();
                for relocate in &relocates {
                    would_move.p2 += u64::from(relocate.best_under.p2.is_some());
                    would_move.p1 += u64::from(relocate.best_under.p1.is_some());
                    would_move.p075 += u64::from(relocate.best_under.p075.is_some());
                    would_move.p05 += u64::from(relocate.best_under.p05.is_some());
                }
                fork_report = Some(ForkReport {
                    sweep: iterations,
                    reached: true,
                    exponents: FORK_EXPONENTS,
                    relocates,
                    would_move,
                    deciding_exponent: probe.exponent.unwrap_or(2.0),
                });
                // The band reading the next turn would have made, so a fork
                // that lands in the band still reports it.
                let totals = energy::fold(&self.state);
                if totals.max_violation_mm <= band {
                    band_entered_at = Some(iterations);
                    evaluations_to_band = Some(evaluations_cumulative);
                }
                break "fork";
            }
        };

        // The detached publication check, after the stop.
        let certification = params.certify.then(|| self.certify_band_entry(stop));

        // `--fork=<sweep>` names a sweep the run may never reach; say so
        // rather than leave `fork: null` beside `stop != "fork"`.
        let fork = match (fork_report, params.fork) {
            (Some(report), _) => Some(ForkOutcome::Reached(report)),
            (None, Some(sweep)) => Some(ForkOutcome::NotReached(ForkNotReached {
                sweep: sweep + 1,
                reached: false,
                stop,
                iterations,
            })),
            (None, None) => None,
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
            let row = identity_row(record, traced, entry_proposals, count);
            if row.equal {
                identity_pass += 1;
            } else {
                identity_fail += 1;
                if diverges_at.is_none() {
                    diverges_at = Some(record.iteration);
                }
            }
            identity.push(row);
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

        // The column, from the states the replay left behind.
        let blocking_frames: Vec<Vec<BlockingRow>> = std::iter::once(entry_blocking.clone())
            .chain(records.iter().map(|record| record.blocking.clone()))
            .collect();
        let weight_frames: Vec<Vec<f64>> = snapshots
            .iter()
            .map(|snapshot| snapshot.weights.clone())
            .collect();
        let relocate_frames: Vec<Vec<ReplayRelocate>> =
            records.iter().map(|record| record.relocates.clone()).collect();
        let column_input = ColumnInput {
            count,
            blocking: &blocking_frames,
            weights: &weight_frames,
            relocates: &relocate_frames,
            band_entry_iteration: band_entered_at,
        };
        let column = analyse_column(&column_input);
        let column_longest_lived = analyse_longest_lived_column(&column_input);
        for (record, snapshot) in records.iter_mut().zip(snapshots.iter().skip(1)) {
            record.column_rows = column
                .rows
                .iter()
                .map(|&id| {
                    ColumnRowReading(
                        id,
                        snapshot.violations[id as usize],
                        snapshot.weights[id as usize],
                    )
                })
                .collect();
            record.core_poses = column
                .core_members
                .iter()
                .map(|&piece| {
                    let pose = snapshot.poses[piece as usize];
                    CorePose(piece, pose.tx_mm, pose.ty_mm, pose.theta_deg)
                })
                .collect();
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
            exponent: probe.exponent,
            entry_raw: entry.raw,
            entry_guided: entry.guided,
            entry_max_mm: entry.max_violation_mm,
            entry_blocking,
            entry_poses_fingerprint,
            entry_weights_fingerprint,
            entry_stream_fingerprint,
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
            column,
            column_longest_lived,
            fork,
            certification,
            iterations: records,
        }
    }

    /// The detached publication check of [`CertificationReport`]: the
    /// unchanged [`Engine::attempt_publication`] once, on the state the
    /// replay stopped at, only when it stopped at band entry. The refusal
    /// it reports is the checkpoint's own string or, when the live path
    /// pushed no checkpoint, [`LIVE_PATH_RETURNED_NOTHING`]; the gates are
    /// never re-derived here, because a re-derivation is not what the live
    /// path said and this report exists to say only that.
    fn certify_band_entry(&mut self, stop: &'static str) -> CertificationReport {
        let totals = energy::fold(&self.state);
        let proxy_depth_mm =
            super::state::raw_source_depth_mm(&self.state.geometry, &self.contract);
        let target_depth_mm = self.state.target_depth_mm;
        let incumbent_depth_mm = self.incumbent.raw_source_depth_mm;
        let path = "Engine::attempt_publication -> publish::attempt -> \
                    validate_placements_against_contract (unchanged)";
        if stop != "band-entry" {
            return CertificationReport {
                attempted: false,
                reason: Some(format!(
                    "the replay did not enter the band (stop = {stop}); the live path is only \
                     called where the live loop would call it"
                )),
                published: false,
                refusal: None,
                depth_mm: None,
                proxy_depth_mm,
                target_depth_mm,
                incumbent_depth_mm,
                improved_incumbent: false,
                exact_calls: 0,
                max_violation_mm: totals.max_violation_mm,
                checkpoint: None,
                path,
            };
        }
        let checkpoints_before = self.trace.checkpoints.len();
        let exact_before = self.trace.work.exact_checkpoints;
        let outcome = self.attempt_publication();
        let exact_calls = self.trace.work.exact_checkpoints - exact_before;
        let checkpoint = (self.trace.checkpoints.len() > checkpoints_before)
            .then(|| CheckpointFields::from(&self.trace.checkpoints[checkpoints_before]));
        // Read, never derived: the checkpoint's own string, or the verbatim
        // record that the live path returned nothing to read.
        let refusal = match (&outcome.publication, &checkpoint) {
            (Some(_), _) => None,
            (None, Some(checkpoint)) => Some(checkpoint.refusal.clone().unwrap_or_else(|| {
                // `publish::attempt` sets a refusal on every checkpoint it
                // returns without a publication; should that ever change,
                // this records the checkpoint as it came.
                "the live path pushed a checkpoint with no publication and `refusal: null`"
                    .to_owned()
            })),
            (None, None) => Some(LIVE_PATH_RETURNED_NOTHING.to_owned()),
        };
        CertificationReport {
            attempted: true,
            reason: None,
            published: outcome.publication.is_some(),
            refusal,
            depth_mm: outcome
                .publication
                .as_ref()
                .map(|publication| publication.raw_source_depth_mm),
            proxy_depth_mm,
            target_depth_mm,
            incumbent_depth_mm,
            improved_incumbent: outcome.improved,
            exact_calls,
            max_violation_mm: totals.max_violation_mm,
            checkpoint,
            path,
        }
    }

    /// [`Engine::tournament`]'s steps 1-7 with [`Descent::worker_sweep_replay`]
    /// in every slot: clone, set the stream ordinal, sweep in scoped threads,
    /// join in ordinal order, select the minimum guided Φ stable by ordinal
    /// (under the exponent probe each slot's guided Φ is `sum w v^p`, from
    /// [`Descent::worker_sweep_replay`]), install, one GLS pass (unchanged:
    /// the weight growth stays on `v / v_max`). Every slot records its
    /// committed relocates through the microscope's `SweepTrace` (the
    /// winner's is kept) and, when `forking`, carries a [`ForkSink`].
    fn replay_tournament(
        &mut self,
        workers: usize,
        bite: u64,
        probe: &ReplayProbeConfig,
        forking: bool,
    ) -> ReplayTournament {
        let count = self.state.poses.len();
        let mut slots: Vec<ReplaySlot> = Vec::with_capacity(workers);
        for ordinal in 0..workers {
            let mut descent = self.descent.clone();
            descent.set_stream(bite, ordinal as u64);
            slots.push(ReplaySlot {
                state: self.state.clone(),
                descent,
                work: super::diagnostics::WorkVector::default(),
                stats: ReplaySweepStats::default(),
                trace: SweepTrace::new(count),
                fork: forking.then(|| ForkSink {
                    worker: ordinal as u32,
                    relocates: Vec::new(),
                }),
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
                Some(&mut slot.trace),
                slot.fork.as_mut(),
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
                                Some(&mut slot.trace),
                                slot.fork.as_mut(),
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
        let forks: Vec<ForkSink> = slots
            .iter_mut()
            .filter_map(|slot| slot.fork.take())
            .collect();
        let slot = slots.swap_remove(winner);
        let winner_stats = slot.stats;
        let winner_evaluations = slot.work.sample_evaluations;
        let winner_trace = slot.trace;
        self.state = slot.state;
        self.descent = slot.descent;
        self.trace.sweeps += 1;
        energy::gls_update(&mut self.state);
        self.trace.work.weight_updates += 1;
        // `guided_after` is the probe's ranking quantity; raw and max are
        // the fold's own (the identity gate compares those and the winner).
        let totals = match probe.exponent {
            Some(exponent) => energy::fold_with_exponent(&self.state, exponent),
            None => energy::fold(&self.state),
        };
        ReplayTournament {
            totals,
            winner,
            contested,
            all_stats,
            winner_stats,
            all_evaluations,
            winner_evaluations,
            winner_trace,
            forks,
        }
    }
}

/// **The identity gate's one comparison**, of one replay iteration against
/// the traced sweep with the same iteration number: the three scalars, the
/// committed relocates through [`first_relocate_difference`], the
/// evaluation counts, and the stream fingerprint against the one the traced
/// sweep's key implies. `replay_separation` builds every `identity[]` row
/// with this and nothing else, and it is `pub` so a test can run the very
/// same comparison on a traced sweep it has perturbed and watch the gate
/// fail - without that the gate's negative is never exercised, and a gate
/// that cannot fail proves nothing. `entry_proposals` is the master
/// descent's proposal ordinal at the capsule: the traced sweep's key is the
/// one it drew from, after it the master holds the winner's clone one
/// iteration on, and the ordinal has advanced by one per piece per sweep,
/// so the fingerprint the trace implies is `(seed, bite, iteration + 1,
/// worker)` at `entry + iteration * pieces`.
pub fn identity_row(
    record: &ReplayIteration,
    traced: &TracedSweep,
    entry_proposals: u64,
    pieces: usize,
) -> IdentityRow {
    let scalars_equal = traced.raw_after.to_bits() == record.raw_after.to_bits()
        && traced.max_after_mm.to_bits() == record.max_after_mm.to_bits()
        && traced.winner == record.winner;
    let evaluations_equal = traced.evaluations_all_workers == record.evaluations_all_workers
        && traced.evaluations_winner == record.evaluations_winner;
    let relocates_first_difference =
        first_relocate_difference(&record.relocates, &traced.relocates);
    let relocates_equal = relocates_first_difference.is_none();
    let stream_fingerprint_trace = traced.stream.map(|key| {
        stream_fingerprint(
            StreamKey {
                seed: key.seed,
                bite: key.bite,
                iteration: key.iteration + 1,
                worker: key.worker,
            },
            entry_proposals + record.iteration * pieces as u64,
        )
    });
    let stream_equal = stream_fingerprint_trace
        .as_ref()
        .map(|trace| *trace == record.stream_fingerprint);
    let equal = scalars_equal && relocates_equal && evaluations_equal;
    IdentityRow {
        iteration: record.iteration,
        raw_trace: traced.raw_after,
        raw_replay: record.raw_after,
        max_trace: traced.max_after_mm,
        max_replay: record.max_after_mm,
        winner_trace: traced.winner,
        winner_replay: record.winner,
        scalars_equal,
        poses_fingerprint: record.poses_fingerprint.clone(),
        weights_fingerprint: record.weights_fingerprint.clone(),
        stream_fingerprint: record.stream_fingerprint.clone(),
        stream_fingerprint_trace,
        stream_equal,
        evaluations_all_workers: record.evaluations_all_workers,
        evaluations_winner: record.evaluations_winner,
        evaluations_all_workers_trace: traced.evaluations_all_workers,
        evaluations_winner_trace: traced.evaluations_winner,
        relocates_replay: record.relocates.len() as u64,
        relocates_trace: traced.relocates.len() as u64,
        relocates_equal,
        relocates_first_difference,
        evaluations_equal,
        equal,
    }
}

/// Where the replay's committed relocates first differ from the traced
/// sweep's: `None` when they match one for one (same piece order, same
/// `dx/dy/dtheta` bits, same changed rows with the same before/after bits).
/// The message names the relocate's index and piece, and the changed row's
/// index when that is where they part.
pub fn first_relocate_difference(
    replay: &[ReplayRelocate],
    traced: &[TracedRelocate],
) -> Option<String> {
    if replay.len() != traced.len() {
        return Some(format!(
            "relocate count {} (replay) vs {} (trace)",
            replay.len(),
            traced.len()
        ));
    }
    for (index, (ours, theirs)) in replay.iter().zip(traced).enumerate() {
        if ours.piece != theirs.piece {
            return Some(format!(
                "relocate {index}: piece {} vs {}",
                ours.piece, theirs.piece
            ));
        }
        if ours.dx_mm.to_bits() != theirs.dx_mm.to_bits()
            || ours.dy_mm.to_bits() != theirs.dy_mm.to_bits()
            || ours.dtheta_deg.to_bits() != theirs.dtheta_deg.to_bits()
        {
            return Some(format!(
                "relocate {index} (piece {}): displacement ({:e}, {:e}, {:e}) vs ({:e}, {:e}, {:e})",
                ours.piece,
                ours.dx_mm,
                ours.dy_mm,
                ours.dtheta_deg,
                theirs.dx_mm,
                theirs.dy_mm,
                theirs.dtheta_deg
            ));
        }
        if ours.rows.len() != theirs.rows.len() {
            return Some(format!(
                "relocate {index} (piece {}): {} changed rows vs {}",
                ours.piece,
                ours.rows.len(),
                theirs.rows.len()
            ));
        }
        for (row, (mine, its)) in ours.rows.iter().zip(&theirs.rows).enumerate() {
            if mine.0 != its.0
                || mine.1.to_bits() != its.1.to_bits()
                || mine.2.to_bits() != its.2.to_bits()
            {
                return Some(format!(
                    "relocate {index} (piece {}), changed row {row}: [{}, {:e}, {:e}] vs [{}, {:e}, {:e}]",
                    ours.piece, mine.0, mine.1, mine.2, its.0, its.1, its.2
                ));
            }
        }
    }
    None
}
