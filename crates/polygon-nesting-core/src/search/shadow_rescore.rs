//! The shadow-rescore audit: does the incremental score equal a from-scratch
//! one, after every accepted move?
//!
//! The relaxed sweep maintains its layout score as a delta. It scores one
//! candidate row against the layout, installs the accepted row, and updates the
//! dense pair state and the running totals in place; it does not rescore the
//! layout. That is the whole point of the delta, and it is also the whole risk:
//! a delta that drifts from what a complete score would have said is a silent
//! wrong answer, not a crash.
//!
//! This module is the audit that closes that gap. Compiled in under the
//! `shadow-rescore` feature, it lets the sweep recompute the *complete* score
//! after every accepted move and compare it against the incrementally
//! maintained one, then reports what it found. It is a measurement build: it
//! roughly doubles the geometric work of a sweep and it is not wired into any
//! default path.
//!
//! # What "agrees" means, and why it is not simply `==`
//!
//! Two quantities in the tracker are *rows* — the per-piece boundary entries,
//! the dense pair entries, and the collision list. A row is one geometric
//! measurement of one pair or one piece. Nothing about the order in which rows
//! are visited can change a row, so rows are required to be **bit-identical**,
//! and any difference is a real disagreement.
//!
//! Two are *running `f64` sums* built from those rows — the per-piece incident
//! totals, and the boundary and weighted totals. Their last bit depends on
//! accumulation order, which is exactly what a delta changes: the incremental
//! path adds one row's change to a total that has been accumulated over the
//! whole sweep, while a complete score adds every row in layout order. This is
//! the same distinction the coupled rollback auditor already draws (see
//! `RollbackMagnitude` in `general_relaxed`), and the same rule applies here:
//! derived sums agree if they are within one `f64` unit in the last place, and
//! the widest gap ever seen is reported so the claim stays measured rather than
//! assumed.
//!
//! The weighted total additionally gets the treatment the audit's own gate
//! demands: **the same summation order in both paths.** A complete score
//! accumulates it interleaved with the boundary walk; the incremental path sums
//! it over the ordered collision list. Comparing those two directly would be
//! comparing two different expressions, so the shadow's weighted total is
//! recomputed in the incremental path's order before the comparison. What is
//! then being tested is the delta, not the summation order.
//!
//! # Reading the report
//!
//! * `checks` — accepted moves audited.
//! * `structuralDisagreements` — audits where the two trackers described
//!   *different layouts*: a different set of colliding pairs, a different row
//!   count, a different boundary violation count. **This is the number that
//!   must be zero.** A non-zero value is a delta that has lost the layout, and
//!   no rounding argument can excuse it.
//! * `magnitudeOnlyAudits` — audits where the structure matched exactly but at
//!   least one row's `f64` magnitude differed. See the operand-order note in
//!   `shadow_tracker_disagreement`: the proxy pressure kernels sum a pole-pair
//!   series with the first operand outermost, so a pair read as `(moving,
//!   fixed)` and the same pair read as `(lower, higher)` are two summation
//!   orders over the same terms. This is a property of the engine's scoring,
//!   not of any one delta.
//! * `maxMagnitudeUlps` — the widest row-magnitude gap seen, in `f64` units in
//!   the last place.
//! * `derivedGapAudits` / `maxDerivedUlps` — audits where every row matched but
//!   a running sum differed, and the widest such gap in `f64` ulps.
//! * `firstStructuralDisagreement` / `firstMagnitudeDisagreement` — the first
//!   of each, rendered.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Whether the audit is compiled into this build.
pub const COMPILED_IN: bool = cfg!(feature = "shadow-rescore");

static CHECKS: AtomicU64 = AtomicU64::new(0);
static STRUCTURAL_DISAGREEMENTS: AtomicU64 = AtomicU64::new(0);
static MAGNITUDE_ONLY_AUDITS: AtomicU64 = AtomicU64::new(0);
static MAX_MAGNITUDE_ULPS: AtomicU64 = AtomicU64::new(0);
static DERIVED_GAP_AUDITS: AtomicU64 = AtomicU64::new(0);
static MAX_DERIVED_ULPS: AtomicU64 = AtomicU64::new(0);

fn first_structural() -> &'static Mutex<Option<String>> {
    static FIRST: Mutex<Option<String>> = Mutex::new(None);
    &FIRST
}

fn first_magnitude() -> &'static Mutex<Option<String>> {
    static FIRST: Mutex<Option<String>> = Mutex::new(None);
    &FIRST
}

/// One audit whose rows all matched bit for bit.
pub fn record_agreement(derived_ulps: u64) {
    CHECKS.fetch_add(1, Ordering::Relaxed);
    if derived_ulps > 0 {
        DERIVED_GAP_AUDITS.fetch_add(1, Ordering::Relaxed);
        MAX_DERIVED_ULPS.fetch_max(derived_ulps, Ordering::Relaxed);
    }
}

/// One audit whose structure matched but whose row magnitudes did not.
pub fn record_magnitude_only(rendered: String, worst_ulps: u64) {
    CHECKS.fetch_add(1, Ordering::Relaxed);
    MAGNITUDE_ONLY_AUDITS.fetch_add(1, Ordering::Relaxed);
    MAX_MAGNITUDE_ULPS.fetch_max(worst_ulps, Ordering::Relaxed);
    if let Ok(mut slot) = first_magnitude().lock() {
        slot.get_or_insert(rendered);
    }
}

/// One audit whose two trackers described different layouts.
pub fn record_disagreement(rendered: String) {
    CHECKS.fetch_add(1, Ordering::Relaxed);
    STRUCTURAL_DISAGREEMENTS.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut slot) = first_structural().lock() {
        slot.get_or_insert(rendered);
    }
}

/// Clears every tally. Called once before a measured stream.
pub fn reset() {
    CHECKS.store(0, Ordering::Relaxed);
    STRUCTURAL_DISAGREEMENTS.store(0, Ordering::Relaxed);
    MAGNITUDE_ONLY_AUDITS.store(0, Ordering::Relaxed);
    MAX_MAGNITUDE_ULPS.store(0, Ordering::Relaxed);
    DERIVED_GAP_AUDITS.store(0, Ordering::Relaxed);
    MAX_DERIVED_ULPS.store(0, Ordering::Relaxed);
    for slot in [first_structural(), first_magnitude()] {
        if let Ok(mut slot) = slot.lock() {
            *slot = None;
        }
    }
}

/// What the audit saw over the stream so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShadowRescoreSnapshot {
    /// Accepted moves audited.
    pub checks: u64,
    /// Audits where the two trackers described different layouts. Must be zero.
    pub structural_disagreements: u64,
    /// Audits where the structure matched but a row magnitude did not.
    pub magnitude_only_audits: u64,
    /// The widest row-magnitude gap seen, in `f64` ulps.
    pub max_magnitude_ulps: u64,
    /// Audits where every row matched but a running sum did not.
    pub derived_gap_audits: u64,
    /// The widest running-sum gap seen, in `f64` ulps.
    pub max_derived_ulps: u64,
    /// The first structural disagreement, rendered.
    pub first_structural_disagreement: Option<String>,
    /// The first magnitude-only disagreement, rendered.
    pub first_magnitude_disagreement: Option<String>,
}

/// Reads the tallies. Safe to call at any time; call it at a barrier.
pub fn snapshot() -> ShadowRescoreSnapshot {
    ShadowRescoreSnapshot {
        checks: CHECKS.load(Ordering::Relaxed),
        structural_disagreements: STRUCTURAL_DISAGREEMENTS.load(Ordering::Relaxed),
        magnitude_only_audits: MAGNITUDE_ONLY_AUDITS.load(Ordering::Relaxed),
        max_magnitude_ulps: MAX_MAGNITUDE_ULPS.load(Ordering::Relaxed),
        derived_gap_audits: DERIVED_GAP_AUDITS.load(Ordering::Relaxed),
        max_derived_ulps: MAX_DERIVED_ULPS.load(Ordering::Relaxed),
        first_structural_disagreement: first_structural().lock().ok().and_then(|slot| slot.clone()),
        first_magnitude_disagreement: first_magnitude().lock().ok().and_then(|slot| slot.clone()),
    }
}

/// The gap between two running sums, in `f64` units in the last place.
///
/// Values that are not both finite, or that differ in sign, are infinitely far
/// apart: those are disagreements about the answer, not about the order the
/// terms were added in.
pub fn derived_ulp_distance(first: f64, second: f64) -> u64 {
    if first == second {
        return 0;
    }
    if !first.is_finite()
        || !second.is_finite()
        || first.is_sign_negative() != second.is_sign_negative()
    {
        return u64::MAX;
    }
    first.to_bits().abs_diff(second.to_bits())
}
