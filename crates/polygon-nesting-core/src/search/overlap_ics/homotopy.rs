//! **The strip homotopy: split-and-close, and the two bite schedules.**
//!
//! This file used to be a stub that returned the target it was handed. Under
//! docs/cutclose-relocate-spec.md, "The regime (frozen)", it is the shrink
//! itself, and the shrink is *not* an affine squeeze of every centroid. It is
//! Sparrow's `separator.rs::change_strip_width` (rev `14f4868f`, arXiv:2509.13329
//! Algorithms 12-13), read and re-implemented on our poses:
//!
//! ```text
//! cut the strip at `split`
//! every piece whose transformed source centroid is on the FAR side
//!     translates by  delta = W_new - W_old   (negative)
//! nothing else moves - not t_x, not theta, not the mirror
//! ```
//!
//! The near side stays where it is, so the layout is *closed* rather than
//! *scaled*: the overlap the bite creates is a seam along one line instead of a
//! proportional error everywhere, and every piece keeps the local packing it
//! already had. That is why the spec forbids the affine start
//! ([`compressed`] survives as a corpus/test factory and the live path must not
//! call it - Grok review 12 Round 2 §6.2, "affine compression remains a corpus
//! factory, not the live start").
//!
//! Our coordinate mapping, stated once: **their strip *width* is our long-axis
//! *depth*.** Their split is an `x`; ours is a `y`. Their `delta` is applied to
//! the translation's `x`; ours to `ty_mm`.
//!
//! Two schedules, both frozen before any wall number exists:
//!
//! * **explore** ([`EXPLORE_SHRINK_STEP`]): `W <- W (1 - 0.001)`, centre cut.
//!   Their `explore.rs` `shrink_step = 0.001` with `split_position = None`.
//! * **compress** ([`COMPRESS_SHRINK_RANGE`]): a `TimeBased` step interpolating
//!   `(0.0005, 0.00001)` against phase-elapsed / phase-limit, and a cut drawn
//!   uniformly across the strip. Their `compress.rs`
//!   `ShrinkDecayStrategy::TimeBased` with a uniform random split.
//!
//! Neither number is fitted to anything. "Shrink-step, sample counts, strike
//! limits, or 80/20 fitted to mixed-61 / 168.484" is a forbidden rescue.

use super::descent::counter_hash;
use super::relocate::transformed_centroid;
use super::state::{Contract, PieceSource, Pose};

/// The exploration bite, `0.1 %`. Sparrow `config.rs`'s `shrink_step`, and the
/// number the whole regime is a test of: 0.1 % of mixed-61's 182.976 is
/// 0.183 mm, which is inside the S1 basin the member already republishes.
pub const EXPLORE_SHRINK_STEP: f64 = 0.001;

/// The compression bite's `(start, end)`, `0.05 % -> 0.001 %`. Sparrow
/// `config.rs`'s `shrink_range` under `ShrinkDecayStrategy::TimeBased`.
pub const COMPRESS_SHRINK_RANGE: (f64, f64) = (0.0005, 0.00001);

/// **The compression range's opening step, overridable for measurement.**
///
/// The fourth inherited twenty-minute constant, and the one that mattered least
/// until the explore step moved: at `0.001` explore ran a hundred bites and used
/// nearly the whole wall, so compress polished whatever was left. At `0.032`
/// explore self-terminates in about five bites and **compress does most of the
/// work**, which makes the range it decays across a live parameter for the first
/// time. `0` keeps [`COMPRESS_SHRINK_RANGE`] exactly.
static COMPRESS_START_OVERRIDE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn set_compress_start_step(step: f64) {
    COMPRESS_START_OVERRIDE.store(step.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn compress_start_step() -> f64 {
    let value = f64::from_bits(COMPRESS_START_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed));
    if value > 0.0 && value < 1.0 {
        value
    } else {
        0.0
    }
}

/// [`COMPRESS_SHRINK_RANGE`] with the opening step overridden, keeping the
/// frozen `50:1` decay ratio between the two ends.
#[inline]
pub fn compress_shrink_range() -> (f64, f64) {
    let named = compress_start_step();
    if named > 0.0 {
        let (start, end) = COMPRESS_SHRINK_RANGE;
        (named, named * (end / start))
    } else {
        COMPRESS_SHRINK_RANGE
    }
}

/// The share of the post-constructor wall that belongs to exploration.
/// Sparrow `consts.rs`: `DEFAULT_EXPLORE_TIME_RATIO = 0.8`,
/// `DEFAULT_COMPRESS_TIME_RATIO = 0.2`.
pub const EXPLORE_TIME_RATIO: f64 = 0.8;

/// A domain tag so a compression cut cannot collide with a sample stream, a
/// permutation, a disruption draw or a pool rank.
const CUT_STREAM_TAG: u64 = 0x4355_545F_5350_4C54; // "CUT_SPLT"

/// A domain tag for the least-infeasible pool's rank draw.
const POOL_STREAM_TAG: u64 = 0x504F_4F4C_5241_4E4B; // "POOLRANK"

/// One bite, recorded whatever it did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bite {
    pub width_before_mm: f64,
    pub width_after_mm: f64,
    /// `W_new - W_old`, so a shrink is negative. This is what the far side's
    /// `ty_mm` is incremented by.
    pub delta_mm: f64,
    pub split_y_mm: f64,
    /// How many pieces were on the far side and therefore moved.
    pub moved_pieces: usize,
    /// The fractional step this bite was taken at: `0.001` in explore, the
    /// interpolated value in compress.
    pub step: f64,
}

/// The centre cut: **mid-depth**, the direct analogue of their
/// `strip_width / 2`.
///
/// Their container spans `[0, strip_width]` and their explore split is its
/// midpoint. Our depth convention measures from the sheet edge at `y = 0` to
/// `W` ([`super::state::raw_source_depth_mm`]), so `W / 2` is the same line.
#[inline]
pub fn centre_cut_mm(width_mm: f64) -> f64 {
    width_mm / 2.0
}

/// The compression cut: uniform in `(edge, W)`, drawn from the counter stream.
///
/// Grok review 12 Round 1 §3 fixes the interval - "split Y uniform in
/// `(edge, W)`" - and Round 2 §6.5 fixes the source - "seed-derived cut". This
/// is both: a `counter_hash` draw over that interval, never `Xoshiro` and never
/// a clock. Like theirs, the draw is allowed to land where it moves everything
/// or nothing; a cut that only ever split the material band would be a
/// different schedule.
pub fn uniform_cut_mm(contract: &Contract, width_mm: f64, seed: u64, bite: u64) -> f64 {
    let low = contract.physical_edge_clearance_mm();
    let high = width_mm;
    if !(high > low) {
        return (low + high) / 2.0;
    }
    low + unit_of(counter_hash(&[seed, bite, CUT_STREAM_TAG])) * (high - low)
}

/// **The explore step, overridable for measurement.** `0` means "use
/// [`EXPLORE_SHRINK_STEP`]", which is Sparrow `config.rs`'s `shrink_step` and a
/// Table 1 value the paper's §11.3 says was tuned for twenty-minute runs and
/// never re-tuned for other limits. Whether 0.1 % is still the right bite at a
/// ten-second budget is a measurement, and this is how it gets measured. The
/// default is unchanged, and a run that does not name it takes exactly the path
/// it always took.
static EXPLORE_STEP_OVERRIDE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn set_explore_shrink_step(step: f64) {
    EXPLORE_STEP_OVERRIDE.store(step.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn explore_shrink_step() -> f64 {
    let bits = EXPLORE_STEP_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed);
    let value = f64::from_bits(bits);
    if value > 0.0 && value < 1.0 {
        value
    } else {
        super::schedule_profile().explore_shrink_step()
    }
}

/// The exploration width after one bite.
#[inline]
pub fn explore_width_mm(width_mm: f64) -> f64 {
    width_mm * (1.0 - explore_shrink_step())
}

/// The compression width after one bite at `step`.
#[inline]
pub fn compress_width_mm(width_mm: f64, step: f64) -> f64 {
    width_mm * (1.0 - step)
}

/// **`ShrinkDecayStrategy::TimeBased`**: the step interpolated linearly across
/// [`COMPRESS_SHRINK_RANGE`] against `elapsed / limit`.
///
/// Pure, and deliberately so: the *clock* is the caller's, read once between
/// bites at a phase boundary, and this function is a function of two numbers.
/// A fixed-work replay hands it a bite ordinal and a bite quota - the same
/// monotone `[0, 1]` parameter with the wall removed - so the gates run the
/// identical trajectory code with no `Instant` anywhere in it.
///
/// `elapsed <= 0` gives the start of the range and `elapsed >= limit` the end;
/// a non-positive or non-finite limit gives the start, because an unmeasured
/// phase has not decayed.
pub fn time_based_step(elapsed: f64, limit: f64) -> f64 {
    let (start, end) = compress_shrink_range();
    if !(limit > 0.0) || !elapsed.is_finite() {
        return start;
    }
    let fraction = (elapsed / limit).clamp(0.0, 1.0);
    start + (end - start) * fraction
}

/// **Split-and-close.** Every piece whose transformed source centroid lies
/// strictly above `split_y_mm` gets `ty_mm += delta_mm`. Nothing else changes.
///
/// The three freezes are assertions about *bits*, not about magnitudes, and the
/// FAST tripwire reads them that way (docs/cutclose-relocate-spec.md, the
/// "cut-close bits" vector): `tx_mm`, `theta_deg` and `mirrored` are copied
/// through unchanged, and a near-side piece's `ty_mm` is copied through
/// unchanged too. A homotopy that also nudged `x`, or that scaled `y` instead
/// of translating it, would be the affine squeeze wearing a cut's name.
///
/// The centroid is recomputed from the pose and the source centroid rather than
/// read out of `Geometry::centroids`, so the decision does not depend on
/// whether a cache happens to be warm - the same reason
/// [`super::relocate::transformed_centroid`] exists.
///
/// Returns the number of pieces that moved.
pub fn split_and_close(
    sources: &[PieceSource],
    poses: &mut [Pose],
    delta_mm: f64,
    split_y_mm: f64,
) -> usize {
    let mut moved = 0usize;
    for (source, pose) in sources.iter().zip(poses.iter_mut()) {
        if transformed_centroid(source, *pose)[1] > split_y_mm {
            pose.ty_mm += delta_mm;
            moved += 1;
        }
    }
    moved
}

/// **The adaptive-step ceiling, `0` for off.**
///
/// The explore step is a constant: every bite is the same fraction of the
/// current width, whether the last one was absorbed without a single retry or
/// only after three pool restarts. That is a *schedule*, and it is the one
/// place in the engine where a difficulty signal is measured and then thrown
/// away - the coordinate descent has grown and shrunk its own step on exactly
/// this signal since the first round.
///
/// When this is non-zero the explore step grows by [`ADAPTIVE_STEP_GROW`] after
/// a bite that published on its first separation and halves after one that
/// needed a retry, clamped to `[base, ceiling]`. `0` restores the constant, so
/// a run that does not name it takes exactly the path it always took.
static ADAPTIVE_STEP_CEILING: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// The multiplier on a bite that published on its first separation.
pub const ADAPTIVE_STEP_GROW: f64 = 1.2;

/// The multiplier on a bite that needed a retry. Deliberately steeper than the
/// growth, and for the same reason `CD_STEP_FAIL` is: an over-large step is
/// paid for immediately and repeatedly, an under-large one only in lost ground.
pub const ADAPTIVE_STEP_FAIL: f64 = 0.5;

pub fn set_adaptive_step_ceiling(ceiling: f64) {
    ADAPTIVE_STEP_CEILING.store(ceiling.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn adaptive_step_ceiling() -> f64 {
    let value = f64::from_bits(ADAPTIVE_STEP_CEILING.load(std::sync::atomic::Ordering::Relaxed));
    if value > 0.0 && value < 1.0 {
        value
    } else {
        0.0
    }
}

/// **The adaptive-step floor, `0` for "the base step".**
///
/// The first spelling of the rule clamped the fall at the base step, which
/// makes the rule purely *upward* from a small start: it can only grow, and it
/// spends its first eight bites climbing out of a step that the constant sweep
/// then showed was sixteen times too small. That is why its control beat it.
///
/// A floor below the base inverts the rule: start large, where the layout has
/// headroom and a bite is absorbed without a retry, and let the halving take it
/// down as the strip tightens. `0` keeps the original clamp, so the falsified
/// arm stays reproducible.
static ADAPTIVE_STEP_FLOOR: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn set_adaptive_step_floor(floor: f64) {
    ADAPTIVE_STEP_FLOOR.store(floor.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn adaptive_step_floor() -> f64 {
    let value = f64::from_bits(ADAPTIVE_STEP_FLOOR.load(std::sync::atomic::Ordering::Relaxed));
    if value > 0.0 && value < 1.0 {
        value
    } else {
        0.0
    }
}

/// The explore step after a bite, given whether it published on its first
/// separation. `ceiling <= 0` returns `step` unchanged.
#[inline]
pub fn adapt_explore_step(step: f64, base: f64, ceiling: f64, first_attempt: bool) -> f64 {
    if !(ceiling > 0.0) {
        return step;
    }
    let floor = {
        let named = adaptive_step_floor();
        if named > 0.0 {
            named.min(base)
        } else {
            base
        }
    };
    if first_attempt {
        (step * ADAPTIVE_STEP_GROW).min(ceiling)
    } else {
        (step * ADAPTIVE_STEP_FAIL).max(floor)
    }
}

/// The whole explore bite, as one value: shrink the width, cut at the centre,
/// close the far side.
pub fn explore_bite(sources: &[PieceSource], poses: &mut [Pose], width_mm: f64) -> Bite {
    explore_bite_at(sources, poses, width_mm, explore_shrink_step())
}

/// [`explore_bite`] at a step the caller chose rather than the process knob.
pub fn explore_bite_at(
    sources: &[PieceSource],
    poses: &mut [Pose],
    width_mm: f64,
    step: f64,
) -> Bite {
    let width_after_mm = width_mm * (1.0 - step);
    let delta_mm = width_after_mm - width_mm;
    let split_y_mm = centre_cut_mm(width_mm);
    let moved_pieces = split_and_close(sources, poses, delta_mm, split_y_mm);
    Bite {
        width_before_mm: width_mm,
        width_after_mm,
        delta_mm,
        split_y_mm,
        moved_pieces,
        step,
    }
}

/// The whole compress bite: a time-decayed step, a uniform cut, the same close.
pub fn compress_bite(
    sources: &[PieceSource],
    poses: &mut [Pose],
    contract: &Contract,
    width_mm: f64,
    step: f64,
    seed: u64,
    bite: u64,
) -> Bite {
    let width_after_mm = compress_width_mm(width_mm, step);
    let delta_mm = width_after_mm - width_mm;
    let split_y_mm = uniform_cut_mm(contract, width_mm, seed, bite);
    let moved_pieces = split_and_close(sources, poses, delta_mm, split_y_mm);
    Bite {
        width_before_mm: width_mm,
        width_after_mm,
        delta_mm,
        split_y_mm,
        moved_pieces,
        step,
    }
}

/// **The least-infeasible pool's rank draw**: their `Normal(0, 0.25)` bias,
/// on our counter source.
///
/// `optimizer/explore.rs` keeps failed separations in a loss-sorted pool and
/// restores one by sampling a normal deviate, taking its magnitude and scaling
/// it by the pool length - so the best entries are drawn most often and a poor
/// one is still reachable. The deviate here is a Box-Muller transform of two
/// `counter_hash` uniforms rather than a `rand_distr::Normal` sample from
/// `Xoshiro`: same distribution, no `rand::` in this tree, and a function of the
/// key alone so two processes agree.
///
/// `libm::log`/`libm::cos` are used rather than `f64`'s, because this is not on
/// the live pose path - M17 pins `f64::sin_cos` only for poses, for identity
/// with the publication transform - and a vendored transcendental is the
/// stronger determinism here.
/// **The pool-restart spread, overridable for measurement.** `0` means the
/// frozen `0.25`, which is Sparrow `config.rs`'s
/// `solution_pool_distribution_stddev` and the fourth Table 1 value in this
/// campaign. If restarts are what the wall iteration cap buys - and the census
/// says they are - then how *far* a restart reaches into the pool is the next
/// thing that matters, and it is another twenty-minute constant.
static POOL_SPREAD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn set_pool_spread(spread: f64) {
    POOL_SPREAD.store(spread.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn pool_spread() -> f64 {
    let value = f64::from_bits(POOL_SPREAD.load(std::sync::atomic::Ordering::Relaxed));
    if value > 0.0 && value.is_finite() {
        value
    } else {
        0.25
    }
}

pub fn normal_biased_rank(len: usize, seed: u64, bite: u64, attempt: u64) -> usize {
    if len <= 1 {
        return 0;
    }
    let root = counter_hash(&[seed, bite, attempt, POOL_STREAM_TAG]);
    // Box-Muller needs a strictly positive first uniform; the smallest
    // representable draw of `unit_of` is 2^-53, so the guard is a formality
    // that costs nothing and cannot be reached into a logarithm of zero.
    let first = unit_of(counter_hash(&[root, 0])).max(f64::MIN_POSITIVE);
    let second = unit_of(counter_hash(&[root, 1]));
    let deviate = libm::sqrt(-2.0 * libm::log(first))
        * libm::cos(2.0 * std::f64::consts::PI * second);
    let scaled = libm::fabs(deviate * pool_spread()) * len as f64;
    if !scaled.is_finite() {
        return 0;
    }
    (scaled as usize).min(len - 1)
}

/// A `[0, 1)` uniform from 53 bits of a counter word. The same construction
/// [`super::descent::rotated_halton`] rotates by, written once.
#[inline]
fn unit_of(key: u64) -> f64 {
    (key >> 11) as f64 / (1u64 << 53) as f64
}

/// `L`: a safe request-level lower scale for the strip depth.
///
/// Two independent bounds, whichever is larger, plus the two edge clearances
/// the depth convention always contains:
///
/// * **area.** The material cannot be thinner than `total area / usable width`
///   however it is arranged.
/// * **the tallest piece.** A piece occupies at least its own minimum width
///   over all rotations, whatever angle it is placed at. `min(bbox width,
///   bbox height)` would over-state that and make `L` unsafe, which is why
///   `decomposition::minimum_width` computes the real supporting-line width.
///
/// This deliberately does not call `portfolio::area_lower_bound_depth_mm`: that
/// bound is offset with the miter/search allowance and so is not a statement
/// about raw material.
///
/// **Sag-aware, and asymmetrically so.** The usable width and the floor are
/// bounded by two *physical* sheet edges, which cost `edge + sag` each; the
/// depth this returns is in the sag-less publication convention, whose top term
/// is `depth_top_inset_mm`. On triangle-20 (`sag = 0.25`) that is
/// `60.0 + 5.25 + 5.0 = 70.25`, not the 70.0 the symmetric `2 * edge` produced
/// (Sol review 15 §A.1).
///
/// **It is a report, not a floor of the schedule.** `CutCloseRelocate` never
/// bisects toward `L`: the regime shrinks by a fixed 0.1 % from the last
/// *published* depth and stops when the wall does, so no target is ever chosen
/// by interpolating between a depth and a bound. The driver prints `L` beside
/// the curve so a reader can see how much room the material still has.
pub fn lower_scale_mm(sources: &[PieceSource], contract: &Contract) -> f64 {
    let edge = contract.physical_edge_clearance_mm();
    let usable_width = (contract.sheet_short_axis_mm - 2.0 * edge).max(f64::MIN_POSITIVE);
    let mut area = 0.0f64;
    let mut tallest = 0.0f64;
    for source in sources {
        area += source.area_mm2;
        tallest = tallest.max(source.min_width_mm);
    }
    (area / usable_width).max(tallest) + edge + contract.depth_top_inset_mm()
}

/// The affine factor that compresses a layout's centroids along the long axis
/// onto `target_mm`, found by bisection on the resulting depth.
///
/// **A corpus and test factory, not a live start.** See [`compressed`].
pub fn affine_compression_factor(
    sources: &[PieceSource],
    poses: &[Pose],
    contract: &Contract,
    target_mm: f64,
) -> f64 {
    let mut low = 0.0f64;
    let mut high = 1.0f64;
    if depth_after(sources, poses, contract, high) <= target_mm {
        return high;
    }
    for _ in 0..64 {
        let middle = (low + high) / 2.0;
        if depth_after(sources, poses, contract, middle) <= target_mm {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}

/// The poses an affine compression factor produces. Rigid per piece: `theta`
/// and the mirror are untouched and only the long-axis offset from the strip's
/// floor scales.
///
/// **This is a corpus/test factory and the live trajectory must not call it.**
/// Grok review 12 Round 2 §6.2 keeps it for `corpus.rs` - the numeric-soundness
/// population needs a deterministic family of *shocked* states, and a global
/// squeeze produces one cheaply - and §6.5 removes it from the regime by name:
/// "Start at constructor legal raw depth `D*` ... No affine live-start."
/// Grok Round 1 §4.4 lists "affine-compressing the constructor as the live
/// start" among the forbidden rescues. [`split_and_close`] is the live shrink.
///
/// [`super::Engine::from_constructor`] still calls it, and that is deliberate:
/// that constructor is now the *diagnostic-cell* factory (the throughput and
/// C175 cells shock a constructor layout on purpose), while the live loop
/// enters through [`super::Engine::from_constructor_at_depth`], which installs
/// the constructor's own poses at `T = D*` and never squeezes anything.
///
/// The floor is the **physical** bottom edge, `edge + sag`, because that is
/// what Phi's bottom row charges (Grok review 10 §B.2).
pub fn compressed(
    sources: &[PieceSource],
    poses: &[Pose],
    contract: &Contract,
    factor: f64,
) -> Vec<Pose> {
    let floor = contract.physical_edge_clearance_mm();
    sources
        .iter()
        .zip(poses)
        .map(|(source, pose)| {
            let (sin, cos) = super::state::pose_sin_cos(pose.theta_deg);
            let centre = super::state::apply_pose(
                source.centroid,
                pose.mirrored,
                sin,
                cos,
                pose.tx_mm,
                pose.ty_mm,
            );
            let shifted = floor + (centre[1] - floor) * factor;
            Pose {
                tx_mm: pose.tx_mm,
                ty_mm: pose.ty_mm + (shifted - centre[1]),
                theta_deg: pose.theta_deg,
                mirrored: pose.mirrored,
            }
        })
        .collect()
}

fn depth_after(
    sources: &[PieceSource],
    poses: &[Pose],
    contract: &Contract,
    factor: f64,
) -> f64 {
    let compressed = compressed(sources, poses, contract, factor);
    let mut deepest = f64::NEG_INFINITY;
    for (source, pose) in sources.iter().zip(&compressed) {
        let (sin, cos) = super::state::pose_sin_cos(pose.theta_deg);
        for point in &source.decomposition.ring {
            let placed = super::state::apply_pose(
                *point,
                pose.mirrored,
                sin,
                cos,
                pose.tx_mm,
                pose.ty_mm,
            );
            deepest = deepest.max(placed[1]);
        }
    }
    deepest + contract.sheet_edge_clearance_mm
}
