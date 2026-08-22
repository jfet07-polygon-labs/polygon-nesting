//! The skip pile's dump hook: every frontier the compression schedule's
//! feasibility clause suppressed, written out as poses so a third authority can
//! be asked about it.
//!
//! # What this is for
//!
//! `CompressionSchedule::due_for_confirmation` has two clauses and both are
//! cost control. The second one -
//!
//! ```text
//! if !proxy_feasible { self.confirmations_skipped_infeasible += 1; return false; }
//! ```
//!
//! - is the filter the round-envelope gate found sitting one level above the
//! confirmation it was measuring: on that gate's twelve-parent miter ladder
//! `confirmationsRefused` was `0` on all 108 runs, because 149 762 frontiers
//! never reached the confirmation at all. `proxy_feasible` is the *relaxed
//! surrogate's* verdict, and the surrogate's collision geometry is the
//! production **miter** offset. So the question this module exists to answer is
//! whether that proxy is hiding a released region: a frontier the disc kernel
//! would accept and the miter refuses.
//!
//! # Why it is written this way
//!
//! * **Compiled out by default.** The whole module is behind `skip-pile-dump`,
//!   the call site is behind the same `cfg`, and the default build therefore
//!   cannot contain a byte of it. The four pinned gates are what proves that
//!   claim rather than this paragraph.
//! * **Disarmed by default even when compiled.** The sink is opened only if
//!   `POLYGON_NESTING_SKIP_PILE_DUMP` names a path. A compiled-but-unarmed
//!   binary evaluates one `OnceLock` read per skip and writes nothing.
//! * **It reads; it does not decide.** Nothing here is returned to the search,
//!   no counter it keeps is published, and the hook runs *after*
//!   `due_for_confirmation` has already returned. The one cost is wall, and
//!   every cell this instrument is used on is **work**-capped, so wall is not
//!   in the trajectory. That is checkable rather than argued: an armed run must
//!   reproduce the committed cell's step digest, its skip count and its depth,
//!   and the driver asserts all three.
//! * **Deduplicated by placement fingerprint**, using the engine's own
//!   `general_placement_fingerprint`, so "distinct skipped frontier" means what
//!   the search means by it and not what this file invented.
//!
//! # The knobs
//!
//! | variable | meaning | default |
//! |---|---|---|
//! | `POLYGON_NESTING_SKIP_PILE_DUMP` | JSONL path; absent means disarmed | disarmed |
//! | `POLYGON_NESTING_SKIP_PILE_DUMP_CAP` | distinct records per process | 20000 |
//!
//! A record is one line of JSON whose `placements` array is exactly the pose
//! fixture shape `round_envelope_battery` already reads, so the scoring stage
//! needs no second reader.
//!
//! A **tally sidecar** is written beside the JSONL at `<path>.tally.json`,
//! carrying `written + duplicates + overCap`. It exists so the driver can check
//! the dump against the schedule's own `confirmationsSkippedInfeasible`
//! *exactly*: those three add to it, and a dump whose lines merely number fewer
//! than the skips would otherwise be indistinguishable from a dump that lost
//! records.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::{Mutex, OnceLock};

use super::general_fast::GeneralFastPlacement;

/// The environment door. Absent means disarmed, and disarmed is the default
/// even in a binary that carries the feature.
pub const DUMP_PATH_ENV: &str = "POLYGON_NESTING_SKIP_PILE_DUMP";
/// How many *distinct* frontiers one process may write.
pub const DUMP_CAP_ENV: &str = "POLYGON_NESTING_SKIP_PILE_DUMP_CAP";
/// Generous, because the cost this cap controls is disk. The scoring budget is
/// enforced downstream, by subsampling the dump, so that the sample spans the
/// whole ladder instead of its first N steps.
pub const DEFAULT_CAP: usize = 20_000;

struct Sink {
    writer: BufWriter<File>,
    tally_path: String,
    seen: HashSet<String>,
    cap: usize,
    written: usize,
    duplicates: usize,
    over_cap: usize,
}

impl Sink {
    /// Rewrites the tally sidecar. Called after every offer, including the ones
    /// that wrote nothing, because the counts that matter most to the driver -
    /// duplicates and over-cap - only move on those.
    fn write_tally(&self) {
        let text = format!(
            "{{\"written\":{},\"duplicates\":{},\"overCap\":{},\"cap\":{},\"offered\":{}}}",
            self.written,
            self.duplicates,
            self.over_cap,
            self.cap,
            self.written + self.duplicates + self.over_cap
        );
        let _ = std::fs::write(&self.tally_path, text);
    }
}

fn sink() -> Option<&'static Mutex<Sink>> {
    static SINK: OnceLock<Option<Mutex<Sink>>> = OnceLock::new();
    SINK.get_or_init(|| {
        let path = std::env::var(DUMP_PATH_ENV).ok()?;
        if path.is_empty() {
            return None;
        }
        let cap = std::env::var(DUMP_CAP_ENV)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(DEFAULT_CAP);
        let file = File::create(&path).ok()?;
        let sink = Sink {
            writer: BufWriter::new(file),
            tally_path: format!("{path}.tally.json"),
            seen: HashSet::new(),
            cap,
            written: 0,
            duplicates: 0,
            over_cap: 0,
        };
        // Written once at open, so an armed cell that skipped nothing at all
        // still leaves a tally rather than leaving the driver to guess whether
        // the run was armed.
        sink.write_tally();
        Some(Mutex::new(sink))
    })
    .as_ref()
}

/// Whether a record would be written if one were offered.
///
/// Called before the placements are materialised so that a disarmed run - and
/// a run that has filled its cap - pays one atomic load and no allocation.
pub fn armed() -> bool {
    match sink() {
        None => false,
        Some(lock) => match lock.lock() {
            Ok(guard) => guard.written < guard.cap,
            Err(_) => false,
        },
    }
}

/// One suppressed frontier, and the context that says where on the ladder it
/// sat.
///
/// Borrowed rather than owned throughout: the caller already holds all of it,
/// and a struct that took ownership would make the hook allocate on a path
/// whose whole point is that it is cheap when disarmed.
pub struct SkipRecord<'a> {
    /// The schedule's own step index, as the step row reports it.
    pub step: usize,
    pub steps_taken: usize,
    pub work_units: usize,
    /// The clamp the step had just lowered to.
    pub frontier_depth_mm: f64,
    /// The deepest exact-confirmed depth at the moment of the skip, in the
    /// schedule's clamp units.
    pub floor_depth_mm: f64,
    /// The incumbent: the best raw source depth this slice has published so
    /// far. This is the number a confirmation would have had to beat, so it is
    /// what turns "the miter refused a legal layout" into "the miter refused a
    /// legal layout that would have published".
    pub published_depth_mm: f64,
    pub parent_depth_mm: f64,
    pub requested_drop_mm: f64,
    /// The proxy's own reasons, which are what made `proxy_feasible` false.
    pub collision_pairs: usize,
    pub boundary_violations: usize,
    pub boundary_loss: f64,
    pub fingerprint: &'a str,
    pub placements: &'a [GeneralFastPlacement],
}

/// Files one suppressed frontier, if it is new and the cap has room.
///
/// Failures are swallowed on purpose. This is an instrument bolted to a search
/// loop; a full disk must not change what the search publishes, or the
/// measurement would be a function of the measuring. They are not swallowed
/// *silently*, though: a failed write returns without counting the offer
/// anywhere, so the sidecar's `offered` falls below the schedule's own skip
/// count and the driver's equality check fails. Losing records loudly is the
/// point.
pub fn record(entry: &SkipRecord<'_>) {
    let Some(lock) = sink() else {
        return;
    };
    let Ok(mut guard) = lock.lock() else {
        return;
    };
    if guard.written >= guard.cap {
        guard.over_cap += 1;
        guard.write_tally();
        return;
    }
    if !guard.seen.insert(entry.fingerprint.to_owned()) {
        guard.duplicates += 1;
        guard.write_tally();
        return;
    }
    let line = serde_json::json!({
        "seq": guard.written,
        "step": entry.step,
        "stepsTaken": entry.steps_taken,
        "workUnits": entry.work_units,
        "frontierDepthMm": entry.frontier_depth_mm,
        "floorDepthMm": entry.floor_depth_mm,
        "publishedDepthMm": entry.published_depth_mm,
        "parentDepthMm": entry.parent_depth_mm,
        "requestedDropMm": entry.requested_drop_mm,
        "collisionPairs": entry.collision_pairs,
        "boundaryViolations": entry.boundary_violations,
        "boundaryLoss": entry.boundary_loss,
        "fingerprint": entry.fingerprint,
        "placements": entry.placements.iter().map(|placement| serde_json::json!({
            "pieceId": placement.piece_id,
            "rotationDeg": placement.rotation_deg,
            "mirrored": placement.mirrored,
            "translateShortAxis": placement.translate_short_axis,
            "translateLongAxis": placement.translate_long_axis,
        })).collect::<Vec<_>>(),
    });
    let Ok(text) = serde_json::to_string(&line) else {
        return;
    };
    if writeln!(guard.writer, "{text}").is_err() {
        return;
    }
    // Flushed per record rather than at exit: there is no exit hook on this
    // path, the process is a benchmark that may be stopped, and a truncated
    // last line would be a sample this instrument silently lost.
    let _ = guard.writer.flush();
    guard.written += 1;
    guard.write_tally();
}

/// What the sink did, in process: `(written, duplicates, over cap)`.
///
/// The same three numbers the sidecar carries, and the sidecar is what the
/// driver reads — a separate process cannot call this. It exists because it is
/// the only way to distinguish *disarmed* (`None`) from *armed and full*
/// (`Some` with `written == cap`), which [`armed`] reports identically as
/// `false`, and that distinction is what this module's own test checks.
pub fn tally() -> Option<(usize, usize, usize)> {
    let lock = sink()?;
    let guard = lock.lock().ok()?;
    Some((guard.written, guard.duplicates, guard.over_cap))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disarmed_process_is_not_armed_and_records_nothing() {
        // The default of the default: no variable in this process's
        // environment, so the hook must be inert. `record` is called to prove
        // it does not panic on a disarmed sink rather than to prove it wrote
        // nothing - there is nowhere for it to have written.
        assert!(std::env::var(DUMP_PATH_ENV).is_err());
        assert!(!armed());
        assert_eq!(tally(), None);
        record(&SkipRecord {
            step: 0,
            steps_taken: 0,
            work_units: 0,
            frontier_depth_mm: 1.0,
            floor_depth_mm: 2.0,
            published_depth_mm: 2.5,
            parent_depth_mm: 3.0,
            requested_drop_mm: 1.0,
            collision_pairs: 1,
            boundary_violations: 0,
            boundary_loss: 0.0,
            fingerprint: "deadbeef",
            placements: &[],
        });
        assert!(!armed());
    }
}
