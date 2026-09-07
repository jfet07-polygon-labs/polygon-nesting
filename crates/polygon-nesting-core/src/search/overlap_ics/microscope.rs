//! **The bite microscope: a buffered, per-relocate trace of the first hard
//! explore bite and the three explore bites after it.** `--bitemicroscope=1`
//! on the cutclose cell; diagnostic only; off by default and byte-identical
//! off; trajectory-identical on.
//!
//! # Why this exists
//!
//! `docs/experiments/overlap-ics/sparrow-warm-start/README.md` lines the first
//! twenty bites of both engines up on the identical constructor layout: bites
//! 1-16 cost the same, and then bite 17 at 180.07 mm costs Sparrow 17 passes
//! and us 37 master iterations, and the three bites after it cost 11, 11 and
//! 14 against 13, 1 and 1. Under `--proxymargin=8` one bite in fifteen still
//! stalls near the band for 50-119 iterations
//! (`docs/experiments/overlap-ics/proxy-margin/README.md`). The bite records
//! the benchmark already emits - iterations, attempts, disruptions, band
//! entries, `minRawPhi` - cannot say *why*: they aggregate a bite, and the
//! question is about the individual committed relocate that leaves the next
//! iteration necessary.
//!
//! GPT-6 Astra review 4 Q3 (`docs/astra-review-4-same-start-two-separators.md`)
//! ranks two causes to be told apart by **one traced cell plus detached
//! replays**, and review 3 Q1/Q2 (`docs/astra-review-3-the-separator.md`)
//! gives the measurement for each:
//!
//! 1. **the fine coordinate descent exits on its piece-relative limits**
//!    (`0.001 * min_dim`, 30-104 um on the fixture) with useful local descent
//!    remaining - measured by the fine-CD exit record (reason, exit incident
//!    maximum, final steps) and replayed detached at a finer termination;
//! 2. **a relocate transfers the blocking contact to a piece whose turn in
//!    the sweep has passed or was never scheduled** - the colliding set is
//!    collected once per sweep (`descent.rs::gauss_seidel`,
//!    `relocate.rs::colliding_permutation`) - measured by classifying, for
//!    every incident row a committed relocate changed, the *other* endpoint's
//!    scheduling status at that moment: `later`, `visited`, `absent` or
//!    `boundary`.
//!
//! Astra's reduced field list is what this module records, no more:
//! (a) a header with the trigger definition and the exposure status; (b) a
//! bite entry with the cut-moved mask, the initial positive-incident mask and
//! a replay capsule; (c) the tournament winner's sweep with all eight workers'
//! evaluation work charged; (d) every committed relocate of that sweep,
//! ran-but-unmoved included; (e) its incident row changes with the endpoint
//! status; (f) the fine-CD exit; (g) the control transitions - the actual
//! [`super::SeparateStop`] of every separation call, pool restores,
//! disruptions with swapped and follower ids, and the publication with the
//! installed repair pose deltas.
//!
//! # Rules the record keeps
//!
//! * **Origin is ancestry, not distance.** `origin` is `pool.best()` before
//!   the fine walk, exactly as `relocate.rs` charges it; a stay-origin walk
//!   can move and a container-origin winner can end near where it started.
//! * **Entry rows against the committed rebuild, never scratch candidates.**
//!   `relocate.rs::evaluate` installs every trial pose; a row born by a trial
//!   that lost is not a conflict birth. The row comparison is taken at
//!   `relocate` entry and after its final commit, and nowhere in between.
//! * **Losers' work is in the denominator.** `evaluationsAllWorkers` is the
//!   sum of the eight slots' sample evaluations; the winner's own share is in
//!   its relocate records.
//! * **Actual stop reasons.** `BiteRecord::attempts` counts *failed*
//!   separations; `separations[].stop` is every call's real stop.
//! * **No RNG, no decision.** Every hook reads; nothing it does reaches a
//!   counter key, a comparison or a control flow the engine decides on. The
//!   on/off identity is asserted in `tests.rs` on a fixed-work trajectory and
//!   demonstrated on mixed-61 in the commit that introduced the flag.
//!
//! # Trigger, fixed prospectively
//!
//! The first explore bite whose master-iteration count reaches
//! [`DEFAULT_TRIGGER_ITERATIONS`] (34, preserving the previous `> 33`
//! threshold) is retained with its complete history, and so are the next
//! [`DEFAULT_RETAIN_AFTER`] (three) explore bites, failures included. Every
//! bite is buffered from its beginning and an ordinary bite is discarded at
//! its end. Nothing is emitted before the timed region ends; if fewer than
//! four qualifying bites exist the report says so in `exposure`.
//!
//! # The replay capsule
//!
//! At every retained bite entry and after every pool restore + disruption
//! inside it - the resets a detached process cannot reconstruct from the
//! trace alone - a [`Capsule`] holds the poses, the GLS weights, the target
//! depth and the master descent stream's coordinates (`seed`, `bite`,
//! `iteration`, `proposals`; the [`super::relocate::RelocateKey`] a worker
//! draws from is that tuple plus its ordinal, recorded per selected sweep).
//! A strike rollback is reconstructible - it restores the state of an
//! iteration the trace names - and is recorded as an event, not a capsule.
//! `f64` values are written as-is; `serde_json` with `float_roundtrip` emits
//! the shortest representation that parses back to the same bits.
//!
//! # Diagnostic only
//!
//! The forbidden-rescue table in `docs/grok-review-12-reading-sparrow.md`
//! §5.2 (row "fixture as a seed") forbids starting a scored cell from a
//! known-good layout, and a replay capsule is exactly such a layout. This
//! flag is never a default, it is refused on every cell but `cutclose`, and
//! whenever it is on the benchmark document carries the top-level
//! `biteMicroscope` block with its `tripwire` field so a scorer can refuse
//! the cell. A microscope cell's ten-second depth is not a treatment score.

use serde::Serialize;

use super::descent::Descent;
use super::disrupt::DisruptOutcome;
use super::energy::{incident_raw, Totals};
use super::homotopy::Bite;
use super::publish::Publication;
use super::relocate::{RelocateKey, RelocateOutcome, RelocateProbe};
use super::state::{pair_count, pair_index, IcsState, Pose};
use super::SeparateStop;

/// The master-iteration count at which an explore bite becomes "the hard
/// bite": Astra review 4 Q3, "the first explore bite reaching 34 master
/// iterations, preserving the previous `>33` threshold".
pub const DEFAULT_TRIGGER_ITERATIONS: u64 = 34;

/// Explore bites retained after the trigger bite, failures included.
pub const DEFAULT_RETAIN_AFTER: u64 = 3;

/// The trigger. The benchmark always passes the defaults; a unit test lowers
/// the threshold to exercise retention on a small fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MicroscopeConfig {
    pub trigger_iterations: u64,
    pub retain_after: u64,
}

impl Default for MicroscopeConfig {
    fn default() -> Self {
        Self {
            trigger_iterations: DEFAULT_TRIGGER_ITERATIONS,
            retain_after: DEFAULT_RETAIN_AFTER,
        }
    }
}

// ------------------------------------------------------------- row identity --

/// A stable row id: `id < pairCount` is the pair row at that
/// [`pair_index`]; otherwise `id - pairCount = piece * 4 + side` with the side
/// in `L, R, B, T` order ([`super::state::EDGE_LEFT`] ..).
pub type RowId = u32;

pub fn pair_row_id(count: usize, first: usize, second: usize) -> RowId {
    let (first, second) = if first < second {
        (first, second)
    } else {
        (second, first)
    };
    pair_index(count, first, second) as RowId
}

pub fn boundary_row_id(count: usize, piece: usize, side: usize) -> RowId {
    (pair_count(count) + piece * 4 + side) as RowId
}

/// What a row id names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Pair { first: usize, second: usize },
    Boundary { piece: usize, side: usize },
}

/// Decodes a row id. The pair decode inverts [`pair_index`] by a scan over
/// `first`, which is `O(n)` and runs only inside the trace builder.
pub fn decode_row_id(count: usize, id: RowId) -> RowKind {
    let id = id as usize;
    let pairs = pair_count(count);
    if id >= pairs {
        let rest = id - pairs;
        return RowKind::Boundary {
            piece: rest / 4,
            side: rest % 4,
        };
    }
    let mut first = 0usize;
    while first + 1 < count {
        let row_start = pair_index(count, first, first + 1);
        let row_end = row_start + (count - first - 1);
        if id < row_end {
            return RowKind::Pair {
                first,
                second: first + 1 + (id - row_start),
            };
        }
        first += 1;
    }
    unreachable!("a pair row id below pair_count decodes")
}

/// The other endpoint of a row from `piece`'s point of view: `None` for a
/// boundary row.
pub fn other_endpoint(count: usize, id: RowId, piece: usize) -> Option<usize> {
    match decode_row_id(count, id) {
        RowKind::Pair { first, second } => Some(if first == piece { second } else { first }),
        RowKind::Boundary { .. } => None,
    }
}

/// The positive part of every row incident on `piece`, as `(id, violation)`,
/// pair rows in ascending other-piece order (the near set's order) and then
/// the four boundary rows. Boundary residuals are clamped at zero so a row
/// that is merely *less clear* is not a violation change.
pub fn incident_rows(state: &IcsState, piece: usize) -> Vec<(RowId, f64)> {
    let count = state.poses.len();
    let mut rows = Vec::with_capacity(state.near[piece].len() + 4);
    for &other in &state.near[piece] {
        let other = other as usize;
        let id = pair_row_id(count, piece, other);
        let violation = state.pair_rows[id as usize].violation_mm;
        if violation > 0.0 {
            rows.push((id, violation));
        }
    }
    for (side, row) in state.edge_rows[piece].iter().enumerate() {
        rows.push((boundary_row_id(count, piece, side), row.violation_mm.max(0.0)));
    }
    rows
}

/// Every positive row of the whole state: the end-of-sweep blocking set.
pub fn blocking_rows(state: &IcsState) -> Vec<BlockingRow> {
    let count = state.poses.len();
    let mut rows = Vec::new();
    for (index, row) in state.pair_rows.iter().enumerate() {
        if row.violation_mm > 0.0 {
            rows.push(BlockingRow(index as RowId, row.violation_mm));
        }
    }
    for (piece, edges) in state.edge_rows.iter().enumerate() {
        for (side, row) in edges.iter().enumerate() {
            if row.violation_mm > 0.0 {
                rows.push(BlockingRow(boundary_row_id(count, piece, side), row.violation_mm));
            }
        }
    }
    rows
}

// --------------------------------------------------------- endpoint status --

/// Where the other endpoint of a changed row stands in the sweep whose
/// relocate changed it. Astra review 3 Q1 rank 2's three classes plus the
/// boundary case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointStatus {
    /// Still to be visited in this sweep: it will get a turn (if it is still
    /// colliding when its turn comes).
    Later,
    /// Already had its turn in this sweep; a conflict handed to it waits for
    /// the next sweep.
    Visited,
    /// Not in this sweep's colliding-set permutation at all: a piece that was
    /// clear when the set was collected gets no turn until the next sweep
    /// (`descent.rs::gauss_seidel`, "a piece that *became* colliding during
    /// the sweep is deliberately not added").
    Absent,
    /// A boundary row: there is no other piece.
    Boundary,
}

/// Classifies `other` against the sweep's initial `order`, with the current
/// relocate at `position` in that order.
pub fn endpoint_status(order: &[usize], position: usize, other: Option<usize>) -> EndpointStatus {
    let Some(other) = other else {
        return EndpointStatus::Boundary;
    };
    match order.iter().position(|piece| *piece == other) {
        None => EndpointStatus::Absent,
        Some(at) if at < position => EndpointStatus::Visited,
        Some(_) => EndpointStatus::Later,
    }
}

// ------------------------------------------------------------------ records --

/// `[rowId, violationBeforeMm, violationAfterMm, otherEndpoint]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct RowChange(pub RowId, pub f64, pub f64, pub EndpointStatus);

/// `[rowId, residualMm]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct BlockingRow(pub RowId, pub f64);

/// The stream coordinates of one descent: the [`RelocateKey`] tuple.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamKey {
    pub seed: u64,
    pub bite: u64,
    pub iteration: u64,
    pub worker: u64,
}

impl From<RelocateKey> for StreamKey {
    fn from(key: RelocateKey) -> Self {
        Self {
            seed: key.seed,
            bite: key.bite,
            iteration: key.iteration,
            worker: key.worker,
        }
    }
}

/// Record (d)+(e)+(f): one committed relocate of the selected sweep.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocateRecord {
    pub piece: u32,
    /// Position in the sweep's initial order.
    pub position: u32,
    /// `stayPut` / `focused` / `container`: the ancestry of the winner.
    pub origin: &'static str,
    pub moved: bool,
    /// Centroid displacement vector and signed angle change.
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub dtheta_deg: f64,
    pub raw_before: f64,
    pub guided_before: f64,
    pub raw_after: f64,
    pub guided_after: f64,
    pub max_before_mm: f64,
    pub max_after_mm: f64,
    pub sample_evaluations: u64,
    /// Record (f).
    pub fine_cd: CdExitRecord,
    /// Record (e), `rowChangeColumns` in the header.
    pub rows: Vec<RowChange>,
}

/// Record (f): how the fine coordinate descent ended.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdExitRecord {
    /// `limits` (every step under its limit) or `iteration-cap`
    /// (`MAX_CD_STEPS`).
    pub reason: &'static str,
    pub exit_raw: f64,
    pub exit_guided: f64,
    /// The incident maximum at the exit pose, which is the committed pose.
    pub exit_max_mm: f64,
    pub final_step_x_mm: f64,
    pub final_step_y_mm: f64,
    pub final_rotation_step_deg: f64,
    pub translation_limit_mm: f64,
    pub rotation_limit_deg: f64,
    /// `+/-` candidate pairs evaluated by the fine walk.
    pub candidate_pairs: u32,
}

/// One worker's private buffer for one sweep. Built inside the worker
/// thread; only the tournament winner's is kept.
#[derive(Clone, Debug, Default)]
pub struct SweepTrace {
    pub count: usize,
    pub key: Option<StreamKey>,
    pub order: Vec<usize>,
    pub skipped: Vec<u32>,
    pub relocates: Vec<RelocateRecord>,
}

impl SweepTrace {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            ..Self::default()
        }
    }

    pub fn begin(&mut self, key: RelocateKey, order: &[usize]) {
        self.key = Some(key.into());
        self.order.clear();
        self.order.extend_from_slice(order);
    }

    /// Record (d)-(f) for one visited piece. A visit the sweep skipped
    /// (cleared earlier in the same sweep) is listed in `skipped`.
    pub fn observe_relocate(
        &mut self,
        position: usize,
        outcome: &RelocateOutcome,
        probe: &RelocateProbe,
    ) {
        if !outcome.ran {
            self.skipped.push(outcome.piece as u32);
            return;
        }
        let piece = outcome.piece;
        let rows = row_changes(
            self.count,
            piece,
            &self.order,
            position,
            &probe.entry_rows,
            &probe.committed_rows,
        );
        let max_of = |rows: &[(RowId, f64)]| rows.iter().fold(0.0f64, |acc, row| acc.max(row.1));
        let max_after = max_of(&probe.committed_rows);
        let exit = probe.fine_exit;
        self.relocates.push(RelocateRecord {
            piece: piece as u32,
            position: position as u32,
            origin: outcome.origin.label(),
            moved: outcome.moved,
            dx_mm: probe.dx_mm,
            dy_mm: probe.dy_mm,
            dtheta_deg: outcome.rotation_deg,
            raw_before: outcome.before.raw,
            guided_before: outcome.before.weighted,
            raw_after: outcome.after.raw,
            guided_after: outcome.after.weighted,
            max_before_mm: max_of(&probe.entry_rows),
            max_after_mm: max_after,
            sample_evaluations: outcome.sample_evaluations,
            fine_cd: CdExitRecord {
                reason: if exit.stalled { "limits" } else { "iteration-cap" },
                exit_raw: outcome.after.raw,
                exit_guided: outcome.after.weighted,
                exit_max_mm: max_after,
                final_step_x_mm: exit.final_steps[0],
                final_step_y_mm: exit.final_steps[1],
                final_rotation_step_deg: exit.final_rotation_step_deg,
                translation_limit_mm: exit.translation_limit_mm,
                rotation_limit_deg: exit.rotation_limit_deg,
                candidate_pairs: exit.candidate_pairs,
            },
            rows,
        });
    }
}

/// Record (e): the rows whose positive part changed between a relocate's
/// entry and its committed rebuild, with the other endpoint classified
/// against the sweep order. Births, deaths and changed persistent rows alike.
pub fn row_changes(
    count: usize,
    piece: usize,
    order: &[usize],
    position: usize,
    entry: &[(RowId, f64)],
    committed: &[(RowId, f64)],
) -> Vec<RowChange> {
    let mut ids: Vec<RowId> = entry.iter().chain(committed).map(|row| row.0).collect();
    ids.sort_unstable();
    ids.dedup();
    let value = |rows: &[(RowId, f64)], id: RowId| {
        rows.iter()
            .find(|row| row.0 == id)
            .map_or(0.0, |row| row.1)
    };
    ids.into_iter()
        .filter_map(|id| {
            let before = value(entry, id);
            let after = value(committed, id);
            if before.to_bits() == after.to_bits() {
                return None;
            }
            let status = endpoint_status(order, position, other_endpoint(count, id, piece));
            Some(RowChange(id, before, after, status))
        })
        .collect()
}

/// Record (c): the tournament winner's sweep, with every worker's work
/// charged.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepRecord {
    pub iteration: u64,
    pub winner: u32,
    pub contested: bool,
    pub stream: Option<StreamKey>,
    pub order: Vec<u32>,
    pub skipped: Vec<u32>,
    pub relocates: Vec<RelocateRecord>,
    pub evaluations_all_workers: u64,
    pub evaluations_winner: u64,
    pub raw_after: f64,
    pub guided_after: f64,
    pub max_after_mm: f64,
    pub blocking: Vec<BlockingRow>,
}

/// One master turn's reading at the band test: `[iteration, raw, guided,
/// maxMm, newMinimum, bandEntry, exactCalls]`.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct IterationSample(pub u64, pub f64, pub f64, pub f64, pub bool, pub bool, pub u64);

/// A strike rollback: `[atIteration, toIteration]`, the state restored being
/// the one the trace recorded at `toIteration` (weights kept).
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Rollback(pub u64, pub u64);

/// Record (g), one separation call.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeparationRecord {
    pub attempt: u64,
    /// The actual [`SeparateStop`]; `null` only if the trace was cut off
    /// inside the call, which cannot happen after the timed region.
    pub stop: Option<&'static str>,
    pub iterations: u64,
    pub min_raw: Option<f64>,
    pub band_entries: u64,
    pub exact_checkpoint_calls: u64,
    /// The iteration whose state the call handed back (the min-raw
    /// snapshot), or `null` when nothing was restored.
    pub restored_to_iteration: Option<u64>,
    pub rollbacks: Vec<Rollback>,
    pub samples: Vec<IterationSample>,
    pub sweeps: Vec<SweepRecord>,
}

/// Record (g), one pool restore followed by a disruption.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetRecord {
    /// The failed separation this reset follows (its `attempt`); the next
    /// separation is `attempt + 1`.
    pub after_attempt: u64,
    pub pool_size: u32,
    pub rank: u32,
    pub entry_raw_phi: f64,
    pub disruption_fired: bool,
    pub swapped: Option<[u32; 2]>,
    pub distinct: bool,
    pub followers: Vec<u32>,
    pub followers_capped: u32,
    /// Index into the report's `capsules[]`: the state the next separation
    /// starts from.
    pub capsule: u32,
}

/// `[piece, dxMm, dyMm, dthetaDeg]` of one installed repair move.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PoseDelta(pub u32, pub f64, pub f64, pub f64);

/// Record (g), the publication.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationRecord {
    pub attempt: u64,
    pub iteration: u64,
    pub target_depth_mm: f64,
    pub published_raw_depth_mm: f64,
    pub repair_rows: u64,
    pub repair_max_displacement_mm: f64,
    pub repair_depth_giveback_mm: f64,
    /// Pieces whose installed pose differs from the pre-repair pose.
    pub installed_pose_deltas: Vec<PoseDelta>,
}

/// The replay capsule.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capsule {
    pub bite: u64,
    /// `bite-entry` or `after-disruption`.
    pub label: &'static str,
    pub target_depth_mm: f64,
    /// `[txMm, tyMm, thetaDeg]` per piece.
    pub poses: Vec<[f64; 3]>,
    pub mirrored: Vec<bool>,
    /// Pair weights in [`pair_index`] order.
    pub pair_weights: Vec<f64>,
    /// `[L, R, B, T]` per piece.
    pub edge_weights: Vec<[f64; 4]>,
    /// The master descent's own stream at this point, read as it is. A
    /// worker of the next tournament clones this descent and draws from
    /// `(seed, capsule.bite, stream.iteration, ordinal)`: `iteration` is the
    /// coordinate that matters, while `stream.bite` and `stream.worker` are
    /// whatever the *last* winner's clone carried (the master itself never
    /// calls `set_stream`), so a replay sets them from `bite` and the
    /// selected sweep's `winner` rather than from here.
    pub stream: StreamKey,
    pub proposals: u64,
    pub raw: f64,
    pub guided: f64,
    pub max_mm: f64,
    /// The guided exponent the capture ran under (`super::guided_exponent`),
    /// present only when the knob was on. A replay's control folds at the
    /// process knob and reproduces this trace only at the same `p`, so the
    /// capsule says which `p` that is, and the benchmark refuses a
    /// `--guidedexponent` that disagrees with the document. Absent at the
    /// frozen engine's 2 so the default document is byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guided_exponent: Option<f64>,
}

impl Capsule {
    /// The live capture: [`Capsule::capture_under`] at the process knob.
    pub(super) fn capture(
        bite: u64,
        label: &'static str,
        state: &IcsState,
        descent: &Descent,
    ) -> Self {
        Self::capture_under(bite, label, state, descent, super::guided_exponent())
    }

    /// The capture at a named exponent. The live path passes the knob; a
    /// test passes `p` directly, because holding the process knob at
    /// `p != 2` for the length of a run makes every test that folds energy
    /// without `knob_lock` in that window see `p`, and the suite runs on
    /// parallel threads.
    pub(super) fn capture_under(
        bite: u64,
        label: &'static str,
        state: &IcsState,
        descent: &Descent,
        guided_exponent: f64,
    ) -> Self {
        let totals = super::energy::fold(state);
        Self {
            bite,
            label,
            target_depth_mm: state.target_depth_mm,
            poses: state
                .poses
                .iter()
                .map(|pose| [pose.tx_mm, pose.ty_mm, pose.theta_deg])
                .collect(),
            mirrored: state.poses.iter().map(|pose| pose.mirrored).collect(),
            pair_weights: state.pair_rows.iter().map(|row| row.weight).collect(),
            edge_weights: state
                .edge_rows
                .iter()
                .map(|rows| [rows[0].weight, rows[1].weight, rows[2].weight, rows[3].weight])
                .collect(),
            stream: descent.stream_key().into(),
            proposals: descent.proposals,
            raw: totals.raw,
            guided: totals.guided,
            max_mm: totals.max_violation_mm,
            guided_exponent: (guided_exponent != super::DEFAULT_GUIDED_EXPONENT)
                .then_some(guided_exponent),
        }
    }
}

/// Record (b) with everything under it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiteTrace {
    pub ordinal: u64,
    /// `trigger`, or `after-1` .. `after-3`.
    pub retained_as: String,
    pub parent_depth_mm: f64,
    pub target_depth_mm: f64,
    pub split_y_mm: f64,
    /// Pieces the cut translated.
    pub cut_moved: Vec<u32>,
    /// Pieces with positive incident raw loss right after the cut.
    pub initial_positive: Vec<u32>,
    pub master_iterations: u64,
    pub published: bool,
    /// Index into `capsules[]`.
    pub capsule: u32,
    pub separations: Vec<SeparationRecord>,
    pub resets: Vec<ResetRecord>,
    pub publication: Option<PublicationRecord>,
}

/// Record (a)'s trace-specific half. The benchmark adds the cell identities
/// and knobs it already emits.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exposure {
    pub trigger_iterations: u64,
    pub retain_after: u64,
    pub rule: &'static str,
    pub explore_bites_seen: u64,
    pub triggered: bool,
    pub trigger_bite: Option<u64>,
    pub retained_bites: Vec<u64>,
    pub complete: bool,
    pub status: String,
}

/// What the benchmark emits under `biteMicroscope`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroscopeReport {
    pub schema_version: u32,
    pub exposure: Exposure,
    pub pieces: u32,
    pub pair_count: u32,
    pub row_id_scheme: &'static str,
    pub row_change_columns: [&'static str; 4],
    pub blocking_row_columns: [&'static str; 2],
    pub iteration_sample_columns: [&'static str; 7],
    pub rollback_columns: [&'static str; 2],
    pub pose_delta_columns: [&'static str; 4],
    pub bites: Vec<BiteTrace>,
    pub capsules: Vec<Capsule>,
}

// ------------------------------------------------------------ the recorder --

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Buffering every explore bite, waiting for the trigger.
    Armed,
    /// The trigger fired; `remaining` further bites are retained.
    Retaining { remaining: u64 },
    /// Four bites retained, or exploration ended. Nothing is buffered.
    Done,
}

/// The buffer and its state machine. Owned by the engine while
/// `run_cutclose` runs with the flag on; `None` otherwise.
#[derive(Clone, Debug)]
pub struct BiteMicroscope {
    config: MicroscopeConfig,
    count: usize,
    stage: Stage,
    explore_bites_seen: u64,
    trigger_bite: Option<u64>,
    current: Option<BiteTrace>,
    current_capsules: Vec<Capsule>,
    /// The iteration whose state the live min-raw snapshot holds.
    snapshot_iteration: u64,
    retained: Vec<BiteTrace>,
    capsules: Vec<Capsule>,
}

impl BiteMicroscope {
    pub fn new(config: MicroscopeConfig, count: usize) -> Self {
        Self {
            config,
            count,
            stage: Stage::Armed,
            explore_bites_seen: 0,
            trigger_bite: None,
            current: None,
            current_capsules: Vec::new(),
            snapshot_iteration: 0,
            retained: Vec::new(),
            capsules: Vec::new(),
        }
    }

    /// `true` while a bite is being buffered: the tournament builds worker
    /// traces exactly then.
    pub fn buffering(&self) -> bool {
        self.current.is_some()
    }

    /// An explore bite has been cut and rebuilt. `poses_before` is the layout
    /// before the cut, from which the cut-moved mask is derived.
    pub fn begin_bite(
        &mut self,
        ordinal: u64,
        bite: &Bite,
        poses_before: &[Pose],
        state: &IcsState,
        descent: &Descent,
    ) {
        self.explore_bites_seen += 1;
        if self.stage == Stage::Done {
            return;
        }
        let cut_moved = poses_before
            .iter()
            .zip(&state.poses)
            .enumerate()
            .filter(|(_, (before, after))| {
                before.tx_mm.to_bits() != after.tx_mm.to_bits()
                    || before.ty_mm.to_bits() != after.ty_mm.to_bits()
                    || before.theta_deg.to_bits() != after.theta_deg.to_bits()
            })
            .map(|(piece, _)| piece as u32)
            .collect();
        let initial_positive = (0..state.poses.len())
            .filter(|piece| incident_raw(state, *piece) > 0.0)
            .map(|piece| piece as u32)
            .collect();
        self.current_capsules.clear();
        self.current_capsules
            .push(Capsule::capture(ordinal, "bite-entry", state, descent));
        self.current = Some(BiteTrace {
            ordinal,
            retained_as: String::new(),
            parent_depth_mm: bite.width_before_mm,
            target_depth_mm: bite.width_after_mm,
            split_y_mm: bite.split_y_mm,
            cut_moved,
            initial_positive,
            master_iterations: 0,
            published: false,
            capsule: 0,
            separations: Vec::new(),
            resets: Vec::new(),
            publication: None,
        });
    }

    pub fn begin_separation(&mut self, attempt: u64) {
        let Some(bite) = self.current.as_mut() else {
            return;
        };
        self.snapshot_iteration = 0;
        bite.separations.push(SeparationRecord {
            attempt,
            stop: None,
            iterations: 0,
            min_raw: None,
            band_entries: 0,
            exact_checkpoint_calls: 0,
            restored_to_iteration: None,
            rollbacks: Vec::new(),
            samples: Vec::new(),
            sweeps: Vec::new(),
        });
    }

    fn separation(&mut self) -> Option<&mut SeparationRecord> {
        self.current
            .as_mut()
            .and_then(|bite| bite.separations.last_mut())
    }

    /// The top of a master turn, after the meter has read the fold.
    pub fn observe_iteration(
        &mut self,
        iteration: u64,
        totals: Totals,
        new_minimum: bool,
        band_entry: bool,
    ) {
        if new_minimum {
            self.snapshot_iteration = iteration;
        }
        if let Some(separation) = self.separation() {
            separation.samples.push(IterationSample(
                iteration,
                totals.raw,
                totals.guided,
                totals.max_violation_mm,
                new_minimum,
                band_entry,
                0,
            ));
            if band_entry {
                separation.band_entries += 1;
            }
        }
    }

    /// The exact authorities were asked `calls` times at this turn's band
    /// entry.
    pub fn observe_exact_calls(&mut self, calls: u64) {
        if let Some(separation) = self.separation() {
            separation.exact_checkpoint_calls += calls;
            if let Some(sample) = separation.samples.last_mut() {
                sample.6 += calls;
            }
        }
    }

    pub fn observe_rollback(&mut self, at_iteration: u64) {
        let to = self.snapshot_iteration;
        if let Some(separation) = self.separation() {
            separation.rollbacks.push(Rollback(at_iteration, to));
        }
    }

    /// The tournament merged: the winner's private trace, every slot's
    /// sample evaluations, and the master state the winner installed.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_sweep(
        &mut self,
        trace: SweepTrace,
        winner: usize,
        contested: bool,
        evaluations_all_workers: u64,
        evaluations_winner: u64,
        totals: Totals,
        state: &IcsState,
    ) {
        let blocking = blocking_rows(state);
        let Some(separation) = self.separation() else {
            return;
        };
        let iteration = separation.sweeps.len() as u64 + 1;
        separation.sweeps.push(SweepRecord {
            iteration,
            winner: winner as u32,
            contested,
            stream: trace.key,
            order: trace.order.iter().map(|piece| *piece as u32).collect(),
            skipped: trace.skipped,
            relocates: trace.relocates,
            evaluations_all_workers,
            evaluations_winner,
            raw_after: totals.raw,
            guided_after: totals.guided,
            max_after_mm: totals.max_violation_mm,
            blocking,
        });
    }

    pub fn end_separation(
        &mut self,
        stop: SeparateStop,
        iterations: u64,
        min_raw: f64,
        restored: bool,
    ) {
        let restored_to = restored.then_some(self.snapshot_iteration);
        if let Some(separation) = self.separation() {
            separation.stop = Some(stop.label());
            separation.iterations = iterations;
            separation.min_raw = min_raw.is_finite().then_some(min_raw);
            separation.restored_to_iteration = restored_to;
        }
    }

    /// A pool entry was installed and the disruption ran; `state` is what the
    /// next separation starts from.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_reset(
        &mut self,
        after_attempt: u64,
        pool_size: usize,
        rank: usize,
        entry_raw_phi: f64,
        disruption: &DisruptOutcome,
        state: &IcsState,
        descent: &Descent,
    ) {
        let Some(bite) = self.current.as_mut() else {
            return;
        };
        let capsule = self.current_capsules.len() as u32;
        self.current_capsules
            .push(Capsule::capture(bite.ordinal, "after-disruption", state, descent));
        bite.resets.push(ResetRecord {
            after_attempt,
            pool_size: pool_size as u32,
            rank: rank as u32,
            entry_raw_phi,
            disruption_fired: disruption.fired,
            swapped: disruption
                .swapped
                .map(|(first, second)| [first as u32, second as u32]),
            distinct: disruption.distinct,
            followers: disruption.followers.iter().map(|piece| *piece as u32).collect(),
            followers_capped: disruption.followers_capped as u32,
            capsule,
        });
    }

    /// A dual-valid publication is about to be installed over `pre_repair`.
    pub fn observe_publication(
        &mut self,
        publication: &Publication,
        pre_repair: &[Pose],
        attempt: u64,
        iteration: u64,
        target_depth_mm: f64,
    ) {
        let Some(bite) = self.current.as_mut() else {
            return;
        };
        let deltas = pre_repair
            .iter()
            .zip(&publication.poses)
            .enumerate()
            .filter(|(_, (before, after))| {
                before.tx_mm.to_bits() != after.tx_mm.to_bits()
                    || before.ty_mm.to_bits() != after.ty_mm.to_bits()
                    || before.theta_deg.to_bits() != after.theta_deg.to_bits()
            })
            .map(|(piece, (before, after))| {
                PoseDelta(
                    piece as u32,
                    after.tx_mm - before.tx_mm,
                    after.ty_mm - before.ty_mm,
                    after.theta_deg - before.theta_deg,
                )
            })
            .collect();
        bite.publication = Some(PublicationRecord {
            attempt,
            iteration,
            target_depth_mm,
            published_raw_depth_mm: publication.raw_source_depth_mm,
            repair_rows: publication.repair_rows,
            repair_max_displacement_mm: publication.repair_max_displacement_mm,
            repair_depth_giveback_mm: publication.repair_depth_giveback_mm,
            installed_pose_deltas: deltas,
        });
    }

    /// The bite's record is complete: retain or discard.
    pub fn end_bite(&mut self, master_iterations: u64, published: bool) {
        let Some(mut bite) = self.current.take() else {
            return;
        };
        bite.master_iterations = master_iterations;
        bite.published = published;
        let retain = match self.stage {
            Stage::Armed if master_iterations >= self.config.trigger_iterations => {
                self.trigger_bite = Some(bite.ordinal);
                bite.retained_as = "trigger".to_owned();
                self.stage = if self.config.retain_after == 0 {
                    Stage::Done
                } else {
                    Stage::Retaining {
                        remaining: self.config.retain_after,
                    }
                };
                true
            }
            Stage::Retaining { remaining } => {
                let taken = self.config.retain_after - remaining + 1;
                bite.retained_as = format!("after-{taken}");
                self.stage = if remaining <= 1 {
                    Stage::Done
                } else {
                    Stage::Retaining {
                        remaining: remaining - 1,
                    }
                };
                true
            }
            Stage::Armed | Stage::Done => false,
        };
        if retain {
            let base = self.capsules.len() as u32;
            bite.capsule += base;
            for reset in &mut bite.resets {
                reset.capsule += base;
            }
            self.capsules.append(&mut self.current_capsules);
            self.retained.push(bite);
        } else {
            self.current_capsules.clear();
        }
    }

    /// Exploration ended: whatever is buffered is closed, and a bite the
    /// deadline interrupted is judged like any other.
    pub fn close_explore(&mut self) {
        self.current = None;
        self.current_capsules.clear();
        self.stage = Stage::Done;
    }

    pub fn finish(self) -> MicroscopeReport {
        let retained_bites: Vec<u64> = self.retained.iter().map(|bite| bite.ordinal).collect();
        let wanted = 1 + self.config.retain_after;
        let complete = self.trigger_bite.is_some() && retained_bites.len() as u64 == wanted;
        let status = match (self.trigger_bite, retained_bites.len() as u64) {
            (None, _) => format!(
                "absent: no explore bite reached {} master iterations in {} explore bites",
                self.config.trigger_iterations, self.explore_bites_seen
            ),
            (Some(bite), retained) if retained < wanted => format!(
                "truncated: trigger bite {bite} retained with {} of {} following explore bites; \
                 exploration ended first",
                retained - 1,
                self.config.retain_after
            ),
            (Some(bite), _) => format!(
                "complete: trigger bite {bite} and the {} explore bites after it",
                self.config.retain_after
            ),
        };
        MicroscopeReport {
            schema_version: 1,
            exposure: Exposure {
                trigger_iterations: self.config.trigger_iterations,
                retain_after: self.config.retain_after,
                rule: "first explore bite whose master-iteration count reaches triggerIterations; \
                       that bite and the next retainAfter explore bites (failures included) are \
                       retained; every other bite is discarded at its end",
                explore_bites_seen: self.explore_bites_seen,
                triggered: self.trigger_bite.is_some(),
                trigger_bite: self.trigger_bite,
                retained_bites,
                complete,
                status,
            },
            pieces: self.count as u32,
            pair_count: pair_count(self.count) as u32,
            row_id_scheme: "id < pairCount: pair row at pair_index(count, i, j) = i*count - i*(i+1)/2 + \
                            (j-i-1), i<j; otherwise (id - pairCount) = piece*4 + side, side in L,R,B,T",
            row_change_columns: ["rowId", "violationBeforeMm", "violationAfterMm", "otherEndpoint"],
            blocking_row_columns: ["rowId", "residualMm"],
            iteration_sample_columns: [
                "iteration",
                "rawPhi",
                "guidedPhi",
                "maxViolationMm",
                "newMinimum",
                "bandEntry",
                "exactCalls",
            ],
            rollback_columns: ["atIteration", "toIteration"],
            pose_delta_columns: ["piece", "dxMm", "dyMm", "dthetaDeg"],
            bites: self.retained,
            capsules: self.capsules,
        }
    }
}
