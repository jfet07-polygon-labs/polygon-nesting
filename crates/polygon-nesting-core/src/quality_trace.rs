//! The quality frontier trace: depth versus *time and work*, event by event.
//!
//! [`profiling`](crate::profiling) answers "where did the milliseconds go" for
//! a whole stream. This module answers a different question, and the one the
//! roadmap review named as the missing measurement: *when* did the search
//! reach each depth, how much work had it done by then, and which operator
//! produced the layout that got it there.
//!
//! The unit of the trace is one **exact-valid candidate** — every complete or
//! partial layout that passes
//! [`validate_and_measure_placements`](crate::search::general_fast) — not only
//! the ones that go on to become the engine's published result. That
//! distinction is the whole point: the documented from-scratch lineage of this
//! engine's deepest layouts runs through constructor basins that were *worse*
//! than the incumbent at the moment they were created, so a curve drawn from
//! public incumbents alone cannot show where the value came from.
//!
//! # Design constraints
//!
//! * **Compiled out by default.** Every entry point below is a `#[cfg]` pair:
//!   with `quality-trace` off, the recording functions have empty bodies, the
//!   scope guard is `()`, and nothing survives into the generated code. This
//!   is the `profiling::deep` pattern, chosen for the same reason — the sites
//!   sit next to very hot, very large generated functions and a runtime branch
//!   near them has been measured to move inlining decisions.
//! * **Nothing here may reach a search decision.** Wall-clock readings and
//!   work ordinals are not reproducible; a search that consulted them would
//!   stop being deterministic. The trace is written, never read back.
//! * **Cold sites only.** A trace event fires once per exact validation, once
//!   per operator scope, and once per publication decision. The hottest of
//!   those is the exact validator, which already rebuilds and pairwise-tests
//!   every collision polygon in the layout; formatting one JSON line next to
//!   that is not measurable. No site in this module is inside a candidate
//!   scan, a pair test, or a pole loop.
//! * **Work ordinals come from the existing counters.** [`init`] switches
//!   `profiling` recording on so that [`crate::profiling::Counter`] totals are
//!   live, and each event carries a snapshot of them. No new counting site is
//!   added to any hot loop; the ordinals a trace reports are the same
//!   quantities a profiled run reports, sampled at event time instead of at
//!   the end. Those sites are not free — a paired A/B puts them at 1.17x on
//!   the mode-22 gate stream — so `POLYGON_NESTING_QUALITY_TRACE_COUNTERS=0`
//!   leaves them alone and gives a run an undistorted clock with zero
//!   ordinals. Every event says which it is, in the run header's
//!   `workOrdinalsArmed`.
//!
//! # What an event stream contains
//!
//! One JSONL file, one object per line, written to the path in
//! `POLYGON_NESTING_QUALITY_TRACE`. The `event` field discriminates:
//!
//! | `event` | emitted where | carries |
//! |---|---|---|
//! | `run` | [`init`] | schema version, build features, wall-clock origin |
//! | `scopeEnter` / `scopeExit` | [`scope`] | operator identity, seed, parent fingerprint, work ordinals |
//! | `exactCandidate` | the exact validator, on success | raw depth, fingerprint, validated piece count, work ordinals |
//! | `incumbent` | the relaxed loop's protected update | new public depth, fingerprint |
//! | `publication` | the adoption rule | outcome and the reason a candidate was refused |
//! | `modeResult` | a deep operator's own report | mode, exact validity, depth, parent, failure reason |
//!
//! Every event carries `t` (seconds since [`init`]) and the work ordinals, so
//! a curve can be drawn against either axis. Deltas between consecutive
//! `scopeEnter`/`scopeExit` pairs give the per-operator work attribution.
//!
//! # Honest limits
//!
//! * The deep-operator geometry counters (`exactPairTests` and
//!   `collisionPolygonBuilds` inside `general_persistent_vacancy`) are behind
//!   `search-profiling`, which distorts a mode-20 stream by about 4.5%. A
//!   trace built without that feature reports those two ordinals as the
//!   *non-deep* totals only; the `run` header states which, in
//!   `deepCountersCompiledIn`, so no reader has to guess.
//! * Scopes are thread-local. Every site instrumented here runs on the thread
//!   that owns the operator, and events raised outside any scope report
//!   `operator: "unscoped"` rather than being attributed to a neighbour.

use std::sync::atomic::{AtomicU64, Ordering};

/// The event-stream schema version. Bump on any field removal or rename.
pub const SCHEMA_VERSION: u32 = 1;

/// The environment variable naming the JSONL sink.
pub const SINK_ENV: &str = "POLYGON_NESTING_QUALITY_TRACE";

/// The environment variable that switches the work ordinals off (`0`).
///
/// Default on. See [`sink::open`] for why this is a knob: the counting sites
/// cost about 17% of a mode-22 stream, so a run measuring *time* to quality
/// wants them off and a run measuring *work* per phase wants them on, and no
/// single run can honestly be both.
pub const COUNTERS_ENV: &str = "POLYGON_NESTING_QUALITY_TRACE_COUNTERS";

/// Whether the trace sites are compiled into this build.
pub const COMPILED_IN: bool = cfg!(feature = "quality-trace");

/// How a traced candidate was disposed of by the code that produced it.
///
/// A candidate the engine has not yet judged is [`Disposition::Seen`]; the
/// three terminal values are set by the site that makes the decision, so a
/// reader never has to infer a disposition from ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// Exact-valid and observed, with no judgement recorded at this site.
    Seen,
    /// Became the public incumbent (the engine's own reported result).
    PublicIncumbent,
    /// Retained as a search state even though it is not the public incumbent.
    Archived,
    /// Not retained.
    Discarded,
}

impl Disposition {
    /// The stable reporting name.
    pub const fn name(self) -> &'static str {
        match self {
            Disposition::Seen => "seen",
            Disposition::PublicIncumbent => "publicIncumbent",
            Disposition::Archived => "archived",
            Disposition::Discarded => "discarded",
        }
    }
}

/// A count of proxy-feasible states handed to the exact tier.
///
/// This is the trace's own cold counter rather than a `profiling::Counter`,
/// because the quantity is only defined at the proxy/exact boundary — a
/// complete or partial state the proxy tier called feasible, offered to the
/// validator — and those boundaries are crossed thousands of times per stream,
/// not tens of millions. Incrementing it therefore costs nothing measurable
/// and needs no hot site.
static PROXY_SURVIVORS: AtomicU64 = AtomicU64::new(0);

/// Records `amount` proxy-feasible states reaching the exact tier.
#[inline(always)]
pub fn proxy_survivors(amount: u64) {
    if COMPILED_IN {
        PROXY_SURVIVORS.fetch_add(amount, Ordering::Relaxed);
    }
}

/// The trace's current proxy-survivor total.
pub fn proxy_survivor_total() -> u64 {
    PROXY_SURVIVORS.load(Ordering::Relaxed)
}

#[cfg(feature = "quality-trace")]
mod sink {
    use std::cell::RefCell;
    use std::fmt::Write as _;
    use std::fs::File;
    use std::io::{BufWriter, Write as _};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    use crate::profiling::{self, Counter};

    use super::{Disposition, PROXY_SURVIVORS, SCHEMA_VERSION};

    /// One operator frame. Frames nest; the innermost names the event.
    #[derive(Clone, Debug)]
    pub(super) struct Frame {
        pub(super) operator: String,
        pub(super) seed: u64,
        pub(super) parent_fingerprint: Option<String>,
        pub(super) depth: usize,
    }

    static NEXT_THREAD: AtomicU64 = AtomicU64::new(0);

    thread_local! {
        static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
        /// A reusable line buffer, so an event allocates nothing after the
        /// first one on each thread.
        static LINE: RefCell<String> = const { RefCell::new(String::new()) };
        /// A stable per-thread ordinal, assigned on first use.
        ///
        /// Scopes are thread-local, so an event raised on a worker thread
        /// inside a scope opened on the calling thread reports `unscoped`.
        /// Rather than guess an attribution for it, the stream says which
        /// thread raised it and a reader can separate the pool's work from the
        /// scoped thread's by that field alone.
        static THREAD: u64 = NEXT_THREAD.fetch_add(1, Ordering::Relaxed);
    }

    struct Writer {
        out: BufWriter<File>,
    }

    static WRITER: OnceLock<Mutex<Option<Writer>>> = OnceLock::new();
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    static ACTIVE: AtomicBool = AtomicBool::new(false);
    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn writer() -> &'static Mutex<Option<Writer>> {
        WRITER.get_or_init(|| Mutex::new(None))
    }

    #[inline(always)]
    pub(super) fn active() -> bool {
        ACTIVE.load(Ordering::Relaxed)
    }

    /// Opens the sink at `path` and starts the clock.
    ///
    /// `counters` arms `profiling` recording, which is what makes the work
    /// ordinals live. It is a knob rather than a constant because the two
    /// things a frontier trace is used for want opposite answers: a work
    /// attribution needs the ordinals, and a *time*-to-quality curve wants the
    /// clock the production build runs on. The counting sites are not free -
    /// measured at a 1.17x paired median on the mode-22 gate stream - so a run
    /// that only wants the timeline turns them off and reports zero ordinals
    /// rather than quietly reporting a stretched clock.
    pub(super) fn open(path: &str, counters: bool, run_fields: &str) -> std::io::Result<()> {
        let file = File::create(path)?;
        {
            let mut slot = writer().lock().unwrap_or_else(|error| error.into_inner());
            *slot = Some(Writer {
                out: BufWriter::with_capacity(1 << 20, file),
            });
        }
        let _ = ORIGIN.set(Instant::now());
        if counters {
            profiling::set_enabled(true);
        }
        ACTIVE.store(true, Ordering::Relaxed);
        emit(|line| {
            let _ = write!(
                line,
                "\"event\":\"run\",\"schemaVersion\":{SCHEMA_VERSION},\
                 \"workOrdinalsArmed\":{counters},\
                 \"deepCountersCompiledIn\":{},\"searchProfiling\":{},{run_fields}",
                profiling::deep::COMPILED_IN,
                cfg!(feature = "search-profiling"),
            );
        });
        Ok(())
    }

    /// Flushes and closes the sink.
    pub(super) fn close() {
        if !active() {
            return;
        }
        emit(|line| {
            let _ = write!(line, "\"event\":\"end\"");
        });
        ACTIVE.store(false, Ordering::Relaxed);
        let mut slot = writer().lock().unwrap_or_else(|error| error.into_inner());
        if let Some(writer) = slot.as_mut() {
            let _ = writer.out.flush();
        }
        *slot = None;
    }

    fn elapsed_seconds() -> f64 {
        ORIGIN
            .get()
            .map_or(0.0, |origin| origin.elapsed().as_secs_f64())
    }

    /// Formats one event and appends it to the sink.
    ///
    /// `body` writes the event-specific fields; the envelope (sequence number,
    /// elapsed time, frame identity, work ordinals) is written here so every
    /// line carries the same axes.
    pub(super) fn emit(body: impl FnOnce(&mut String)) {
        if !active() {
            return;
        }
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let elapsed = elapsed_seconds();
        let totals = profiling::counter_totals();
        let frame = FRAMES.with(|frames| frames.borrow().last().cloned());
        LINE.with(|buffer| {
            let mut buffer = buffer.borrow_mut();
            buffer.clear();
            let line = &mut *buffer;
            line.push('{');
            let thread = THREAD.with(|ordinal| *ordinal);
            let _ = write!(
                line,
                "\"seq\":{seq},\"t\":{elapsed:.9},\"thread\":{thread},"
            );
            match &frame {
                Some(frame) => {
                    let _ = write!(line, "\"operator\":");
                    push_json_string(line, &frame.operator);
                    let _ = write!(
                        line,
                        ",\"scopeDepth\":{},\"seed\":{},",
                        frame.depth, frame.seed
                    );
                    let _ = write!(line, "\"parentFingerprint\":");
                    match &frame.parent_fingerprint {
                        Some(value) => push_json_string(line, value),
                        None => line.push_str("null"),
                    }
                    line.push(',');
                }
                None => {
                    line.push_str(
                        "\"operator\":\"unscoped\",\"scopeDepth\":0,\"seed\":null,\
                         \"parentFingerprint\":null,",
                    );
                }
            }
            let _ = write!(
                line,
                "\"work\":{{\"candidateQueries\":{},\"neighborTests\":{},\
                 \"effectiveMoves\":{},\"acceptedMoves\":{},\"fullRescores\":{},\
                 \"publicationAttempts\":{},\"exactPairTests\":{},\
                 \"collisionPolygonBuilds\":{},\"proxySurvivors\":{}}},",
                totals[Counter::CandidateQueries as usize],
                totals[Counter::NeighborTests as usize],
                totals[Counter::EffectivePieceMoves as usize],
                totals[Counter::AcceptedMoves as usize],
                totals[Counter::FullRescores as usize],
                totals[Counter::PublicationAttempts as usize],
                totals[Counter::ExactPairTests as usize],
                totals[Counter::CollisionPolygonBuilds as usize],
                PROXY_SURVIVORS.load(Ordering::Relaxed),
            );
            body(line);
            line.push_str("}\n");
            let mut slot = writer().lock().unwrap_or_else(|error| error.into_inner());
            if let Some(writer) = slot.as_mut() {
                let _ = writer.out.write_all(line.as_bytes());
            }
        });
    }

    /// Appends `value` as a JSON string, escaping the characters JSON forbids.
    pub(super) fn push_json_string(line: &mut String, value: &str) {
        line.push('"');
        for character in value.chars() {
            match character {
                '"' => line.push_str("\\\""),
                '\\' => line.push_str("\\\\"),
                '\n' => line.push_str("\\n"),
                '\r' => line.push_str("\\r"),
                '\t' => line.push_str("\\t"),
                character if (character as u32) < 0x20 => {
                    let _ = write!(line, "\\u{:04x}", character as u32);
                }
                character => line.push(character),
            }
        }
        line.push('"');
    }

    /// Writes an `f64` field that may be non-finite, as JSON `null` if so.
    pub(super) fn push_json_f64(line: &mut String, value: f64) {
        if value.is_finite() {
            // `ryu_js` is the crate's own shortest round-tripping formatter;
            // using it here keeps a traced depth byte-identical to the depth
            // the engine reports in its result JSON.
            line.push_str(ryu_js::Buffer::new().format_finite(value));
        } else {
            line.push_str("null");
        }
    }

    pub(super) fn push_frame(operator: String, seed: u64, parent_fingerprint: Option<String>) {
        FRAMES.with(|frames| {
            let mut frames = frames.borrow_mut();
            let depth = frames.len();
            frames.push(Frame {
                operator,
                seed,
                parent_fingerprint,
                depth,
            });
        });
    }

    pub(super) fn pop_frame() {
        FRAMES.with(|frames| {
            frames.borrow_mut().pop();
        });
    }

    pub(super) fn disposition_field(line: &mut String, disposition: Disposition) {
        let _ = write!(line, "\"disposition\":\"{}\"", disposition.name());
    }
}

/// The RAII guard returned by [`scope`]; carries no data when compiled out.
#[cfg(feature = "quality-trace")]
#[derive(Debug)]
pub struct Scope {
    armed: bool,
}

/// The RAII guard returned by [`scope`]; carries no data when compiled out.
#[cfg(not(feature = "quality-trace"))]
pub type Scope = ();

#[cfg(feature = "quality-trace")]
impl Drop for Scope {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        sink::emit(|line| {
            let _ = std::fmt::Write::write_str(line, "\"event\":\"scopeExit\"");
        });
        sink::pop_frame();
    }
}

/// Opens the sink named by `POLYGON_NESTING_QUALITY_TRACE`, if it is set.
///
/// `run_fields` is a caller-supplied JSON fragment (no braces, no leading
/// comma) describing the invocation — mode, seeds, request digest — so the
/// stream is self-describing without this crate knowing what a harness runs.
/// Returns whether a sink was opened.
#[cfg(feature = "quality-trace")]
pub fn init(run_fields: &str) -> bool {
    let Ok(path) = std::env::var(SINK_ENV) else {
        return false;
    };
    if path.is_empty() {
        return false;
    }
    let counters = std::env::var(COUNTERS_ENV).map_or(true, |value| value != "0");
    match sink::open(&path, counters, run_fields) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("quality trace: could not open {path}: {error}");
            false
        }
    }
}

/// Opens the sink. Compiled out.
#[cfg(not(feature = "quality-trace"))]
pub fn init(_run_fields: &str) -> bool {
    false
}

/// Flushes and closes the sink.
#[cfg(feature = "quality-trace")]
pub fn finish() {
    sink::close();
}

/// Flushes and closes the sink. Compiled out.
#[cfg(not(feature = "quality-trace"))]
pub fn finish() {}

/// Whether a sink is currently recording.
#[cfg(feature = "quality-trace")]
#[inline(always)]
pub fn active() -> bool {
    sink::active()
}

/// Whether a sink is currently recording. Always false.
#[cfg(not(feature = "quality-trace"))]
#[inline(always)]
pub fn active() -> bool {
    false
}

/// Opens an operator scope, naming the work that follows until the guard drops.
///
/// `operator` is a dotted identity (`"m0.epoch"`, `"mode20.construct"`,
/// `"coupled.boundaryProjection"`); `parent_fingerprint` is the fingerprint of
/// the state this operator descends from, when it descends from a tracked one.
#[cfg(feature = "quality-trace")]
pub fn scope(operator: impl Into<String>, seed: u64, parent_fingerprint: Option<&str>) -> Scope {
    if !active() {
        return Scope { armed: false };
    }
    sink::push_frame(operator.into(), seed, parent_fingerprint.map(str::to_owned));
    sink::emit(|line| {
        let _ = std::fmt::Write::write_str(line, "\"event\":\"scopeEnter\"");
    });
    Scope { armed: true }
}

/// Opens an operator scope. Compiled out.
#[cfg(not(feature = "quality-trace"))]
#[inline(always)]
pub fn scope(_operator: &str, _seed: u64, _parent_fingerprint: Option<&str>) -> Scope {}

/// Records one exact-valid candidate.
///
/// Called from the exact validator's success path, so this fires for every
/// layout the search proves legal — complete or partial, published or not.
#[cfg(feature = "quality-trace")]
pub fn exact_candidate(
    raw_depth_mm: f64,
    envelope_depth_mm: f64,
    piece_count: usize,
    fingerprint: &str,
) {
    use std::fmt::Write as _;
    sink::emit(|line| {
        let _ = write!(line, "\"event\":\"exactCandidate\",\"rawDepthMm\":");
        sink::push_json_f64(line, raw_depth_mm);
        let _ = write!(line, ",\"envelopeDepthMm\":");
        sink::push_json_f64(line, envelope_depth_mm);
        let _ = write!(line, ",\"pieceCount\":{piece_count},\"fingerprint\":");
        sink::push_json_string(line, fingerprint);
        let _ = write!(line, ",");
        sink::disposition_field(line, Disposition::Seen);
    });
}

/// Records one exact-valid candidate. Compiled out.
#[cfg(not(feature = "quality-trace"))]
#[inline(always)]
pub fn exact_candidate(
    _raw_depth_mm: f64,
    _envelope_depth_mm: f64,
    _piece_count: usize,
    _fingerprint: &str,
) {
}

/// Records a change to the engine's public incumbent.
#[cfg(feature = "quality-trace")]
pub fn incumbent(depth_mm: f64, piece_count: usize, fingerprint: &str, source: &str) {
    use std::fmt::Write as _;
    sink::emit(|line| {
        let _ = write!(line, "\"event\":\"incumbent\",\"depthMm\":");
        sink::push_json_f64(line, depth_mm);
        let _ = write!(line, ",\"pieceCount\":{piece_count},\"fingerprint\":");
        sink::push_json_string(line, fingerprint);
        let _ = write!(line, ",\"source\":");
        sink::push_json_string(line, source);
        let _ = write!(line, ",");
        sink::disposition_field(line, Disposition::PublicIncumbent);
    });
}

/// Records a change to the engine's public incumbent. Compiled out.
#[cfg(not(feature = "quality-trace"))]
#[inline(always)]
pub fn incumbent(_depth_mm: f64, _piece_count: usize, _fingerprint: &str, _source: &str) {}

/// Records one publication-adoption decision and why it went that way.
#[cfg(feature = "quality-trace")]
pub fn publication(
    disposition: Disposition,
    published_depth_mm: f64,
    legacy_depth_mm: f64,
    reason: &str,
) {
    use std::fmt::Write as _;
    sink::emit(|line| {
        let _ = write!(line, "\"event\":\"publication\",\"publishedDepthMm\":");
        sink::push_json_f64(line, published_depth_mm);
        let _ = write!(line, ",\"legacyDepthMm\":");
        sink::push_json_f64(line, legacy_depth_mm);
        let _ = write!(line, ",\"reason\":");
        sink::push_json_string(line, reason);
        let _ = write!(line, ",");
        sink::disposition_field(line, disposition);
    });
}

/// Records one publication-adoption decision. Compiled out.
#[cfg(not(feature = "quality-trace"))]
#[inline(always)]
pub fn publication(
    _disposition: Disposition,
    _published_depth_mm: f64,
    _legacy_depth_mm: f64,
    _reason: &str,
) {
}

/// Records a deep operator's own final report.
#[cfg(feature = "quality-trace")]
pub fn mode_result(
    mode: usize,
    exact_valid: bool,
    depth_mm: Option<f64>,
    parent_depth_mm: Option<f64>,
    fingerprint: Option<&str>,
    failure_reason: Option<&str>,
) {
    use std::fmt::Write as _;
    sink::emit(|line| {
        let _ = write!(
            line,
            "\"event\":\"modeResult\",\"mode\":{mode},\"exactValid\":{exact_valid},\"depthMm\":"
        );
        sink::push_json_f64(line, depth_mm.unwrap_or(f64::NAN));
        let _ = write!(line, ",\"parentDepthMm\":");
        sink::push_json_f64(line, parent_depth_mm.unwrap_or(f64::NAN));
        let _ = write!(line, ",\"fingerprint\":");
        match fingerprint {
            Some(value) => sink::push_json_string(line, value),
            None => line.push_str("null"),
        }
        let _ = write!(line, ",\"failureReason\":");
        match failure_reason {
            Some(value) => sink::push_json_string(line, value),
            None => line.push_str("null"),
        }
        let _ = write!(line, ",");
        sink::disposition_field(
            line,
            if exact_valid {
                Disposition::Archived
            } else {
                Disposition::Discarded
            },
        );
    });
}

/// Records a deep operator's own final report. Compiled out.
#[cfg(not(feature = "quality-trace"))]
#[inline(always)]
pub fn mode_result(
    _mode: usize,
    _exact_valid: bool,
    _depth_mm: Option<f64>,
    _parent_depth_mm: Option<f64>,
    _fingerprint: Option<&str>,
    _failure_reason: Option<&str>,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default build must not carry the trace: this is the gate that keeps
    /// a diagnostics module out of the engine every consumer runs.
    #[test]
    fn the_trace_is_off_unless_its_feature_is_on() {
        assert_eq!(COMPILED_IN, cfg!(feature = "quality-trace"));
        if !COMPILED_IN {
            assert!(!active());
            assert!(!init("\"probe\":true"));
        }
    }

    /// Disposition names are a wire contract; a rename must break a test
    /// rather than a downstream plot.
    #[test]
    fn disposition_names_are_stable() {
        assert_eq!(Disposition::Seen.name(), "seen");
        assert_eq!(Disposition::PublicIncumbent.name(), "publicIncumbent");
        assert_eq!(Disposition::Archived.name(), "archived");
        assert_eq!(Disposition::Discarded.name(), "discarded");
    }
}
