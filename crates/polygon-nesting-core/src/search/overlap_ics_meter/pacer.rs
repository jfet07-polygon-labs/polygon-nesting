//! **The calibrated-work pacer, as a primitive. Not wired, and it cannot read
//! a clock.**
//!
//! docs/economics-round-spec.md, funded change 3, verbatim:
//!
//! > The file pins request hash, currency version, binary/feature key,
//! > workers=8, executor implementation, per-phase safe units/s; read/write
//! > separate; no live probe on a gated trajectory; **80/20 by calibrated
//! > units; compress decay by consumed compress-work; stop only between master
//! > batches.** Wording: "10-second calibrated work plan" - quality
//! > deterministic, wall a distribution (no governor exists).
//!
//! # The clock rule is structural here, not a convention
//!
//! The evidence audit's clean finding - *"`Instant` appears in exactly one
//! place under `search/overlap_ics/` … a fixed-work trajectory cannot read a
//! clock at all"* - is what makes two-process bit identity a proof. A pacer is
//! the one component with a motive to break it, because the thing it replaces
//! (`Pacer::Wall`) is the one component that legitimately reads one.
//!
//! Three defences, in increasing strength:
//!
//! 1. `std::time` does not appear in this file. There is no clock to read.
//! 2. [`WorkPlanPacer`] is nevertheless **handed** a clock, and never calls
//!    it. The type parameter exists so that the absence is testable rather
//!    than merely true today.
//! 3. [`PoisonedClock`] panics when read. The vector drives a whole synthetic
//!    trajectory - both phases, a strike, the compress decay, the phase
//!    boundary, several hundred batches - holding one, and then asserts it was
//!    never touched. A future edit that reaches for a clock turns that vector
//!    red on the line that reached.
//!
//! # "Stop only between master batches", made mechanical
//!
//! There is no `should_stop()` on this type. The only way to learn whether a
//! phase is spent is to be handed a [`BatchBoundary`], and the only ways to
//! get one are [`WorkPlanPacer::entry_boundary`] - before the first batch -
//! and [`WorkPlanPacer::charge_batch`] - immediately after one. A caller
//! cannot ask the question in the middle of a tournament because there is
//! nothing to ask it of.
//!
//! # 80/20, spent in units
//!
//! The frozen share splits the **wall budget**, and each half is then
//! converted at its own phase's safe rate. That is the only reading that
//! survives per-phase rates, and it keeps `EXPLORE_TIME_RATIO` doing exactly
//! what it did before: deciding what fraction of the run explore gets. What
//! changes is what the trajectory *counts down* - calibrated units, not
//! seconds - which is the whole point: quality becomes deterministic while the
//! wall stays a distribution.
//!
//! The share is a **parameter, not a literal**, in this module. `0.8` is
//! frozen verbatim by the spec and lives in `ScheduleConfig`; a copy of it
//! here would be a second place for it to drift.
//!
//! # Compress decay by consumed compress-work
//!
//! [`WorkPlanPacer::compress_step`] feeds `homotopy::time_based_step` the
//! *consumed compress units over the compress allocation* - the same monotone
//! `[0, 1]` the shipped `Pacer::FixedWork` feeds it as bite ordinal over bite
//! quota, with the wall removed and nothing else changed. The decay code is
//! the frozen one, called and not copied.

use std::cell::Cell;

use crate::search::overlap_ics::homotopy;
use crate::search::overlap_ics::icscal::{
    BinaryKey, CurrencyVersion, Executor, PlanKey, PlanPhase, WorkPlan,
};
use crate::search::overlap_ics_meter::currency::{Currency, WorkTerms};

/// A source of seconds. The calibrated pacer is handed one and never calls it.
pub trait PlanClock {
    fn elapsed_seconds(&self) -> f64;
}

/// The default: a trajectory paced by a calibrated plan has no clock, and
/// saying so out loud is cheaper than a comment.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoClock;

impl PlanClock for NoClock {
    fn elapsed_seconds(&self) -> f64 {
        panic!("a calibrated-work trajectory has no clock; this call is the defect")
    }
}

/// **The test double.** Counts reads and panics on the first one.
///
/// The panic is the stop; the counter is for a caller that wants to assert the
/// absence positively - [`PoisonedClock::reads`] must be zero after a
/// trajectory that completed.
#[derive(Debug, Default)]
pub struct PoisonedClock {
    reads: Cell<u64>,
}

impl PoisonedClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many times the pacer reached for a clock. Must be zero.
    pub fn reads(&self) -> u64 {
        self.reads.get()
    }
}

impl PlanClock for PoisonedClock {
    fn elapsed_seconds(&self) -> f64 {
        self.reads.set(self.reads.get() + 1);
        panic!("the pacer read a poisoned clock: a calibrated trajectory must not read one")
    }
}

/// Whether a plan may be spent on the question being asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanMatch {
    /// Every key field agrees; the plan is this run's plan.
    Hit,
    /// The first field that did not, named.
    Miss(String),
}

impl PlanMatch {
    pub fn is_hit(&self) -> bool {
        matches!(self, Self::Hit)
    }
}

/// **Hit or miss, field by field, in the order a mismatch matters.**
///
/// Not a reader: it takes two already-constructed keys, so nothing here can
/// acquire a plan, parse one, or go looking for one on disk. The spec's
/// "read/write separate" is a statement about entry points, and this is
/// neither.
///
/// A miss is not an error. It is the honest answer that this box has never
/// measured this question, and the caller's only correct response is to
/// calibrate offline - never to probe live, which is the clause that keeps a
/// gated trajectory deterministic.
pub fn match_plan(wanted: &PlanKey, plan: &PlanKey) -> PlanMatch {
    fn differs(field: &str, wanted: &str, found: &str) -> Option<PlanMatch> {
        (wanted != found)
            .then(|| PlanMatch::Miss(format!("{field}: wanted `{wanted}`, plan has `{found}`")))
    }
    fn binary(wanted: &BinaryKey, found: &BinaryKey) -> Option<PlanMatch> {
        differs(
            "binaryKey.executableSha256",
            &wanted.executable_sha256,
            &found.executable_sha256,
        )
        .or_else(|| {
            (wanted.features != found.features).then(|| {
                PlanMatch::Miss(format!(
                    "binaryKey.features: wanted {:?}, plan has {:?}",
                    wanted.features, found.features
                ))
            })
        })
    }
    differs(
        "requestSha256",
        &wanted.request_sha256,
        &plan.request_sha256,
    )
    .or_else(|| {
        differs(
            "currencyVersion",
            wanted.currency_version.as_str(),
            plan.currency_version.as_str(),
        )
    })
    .or_else(|| binary(&wanted.binary_key, &plan.binary_key))
    .or_else(|| {
        (wanted.workers != plan.workers).then(|| {
            PlanMatch::Miss(format!(
                "workers: wanted {}, plan has {}",
                wanted.workers, plan.workers
            ))
        })
    })
    .or_else(|| differs("executor", wanted.executor.as_str(), plan.executor.as_str()))
    .unwrap_or(PlanMatch::Hit)
}

/// What the pacer says at a master-batch boundary, and the only place it says
/// anything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BatchBoundary {
    pub phase: PlanPhase,
    /// `U` of the batch just charged. Zero at [`WorkPlanPacer::entry_boundary`].
    pub units_charged: u64,
    pub phase_consumed: u64,
    pub phase_allocation: u64,
    /// `consumed >= allocation`: the phase has no room for another batch.
    pub phase_exhausted: bool,
    /// Batches charged to this phase so far.
    pub phase_batches: u64,
    /// `homotopy::time_based_step` of the consumed compress fraction. The
    /// compress bite's TimeBased parameter, with no wall in it.
    pub compress_step: f64,
}

/// **The pacer.** Units in, a boundary verdict out, no clock anywhere.
///
/// It owns no engine state and no filesystem handle. Everything it knows came
/// from a plan somebody else read and terms somebody else counted, which is
/// what makes "no live probe on a gated trajectory" a property of the type
/// rather than a rule about how to use it.
#[derive(Debug)]
pub struct WorkPlanPacer<C: PlanClock = NoClock> {
    /// Held, never called. See the module docs.
    clock: C,
    key: PlanKey,
    currency: Currency,
    explore_allocation: u64,
    compress_allocation: u64,
    explore_consumed: u64,
    compress_consumed: u64,
    explore_batches: u64,
    compress_batches: u64,
    budget_seconds: f64,
    explore_ratio: f64,
}

impl<C: PlanClock> WorkPlanPacer<C> {
    /// A pacer for `budget_seconds` of calibrated work, split `explore_ratio`
    /// / `1 - explore_ratio` and converted at each phase's own safe rate.
    ///
    /// `explore_ratio` is the caller's frozen `EXPLORE_TIME_RATIO`; it is
    /// clamped to `[0, 1]` exactly as `Pacer::new` clamps it, so the two
    /// pacers cannot disagree about a nonsense input.
    ///
    /// Refuses a plan that does not carry both phases, a plan whose currency
    /// version is not the currency being spent, and a budget that is not a
    /// positive finite number of seconds. A phase whose allocation rounds to
    /// zero units is also a refusal: a phase with no budget is not a phase the
    /// trajectory can enter, and discovering that at run time would look like
    /// a strike.
    pub fn from_plan(
        plan: &WorkPlan,
        currency: &Currency,
        budget_seconds: f64,
        explore_ratio: f64,
        clock: C,
    ) -> Result<Self, String> {
        plan.validate()?;
        if plan.key.currency_version != currency.version {
            return Err(format!(
                "the plan is denominated in {} and the currency offered is {}",
                plan.key.currency_version.as_str(),
                currency.version.as_str()
            ));
        }
        if currency.version == CurrencyVersion::U1Weighted
            && plan.currency.as_ref() != currency.coefficients.as_ref()
        {
            return Err("the plan's pinned coefficients are not the ones being spent".to_owned());
        }
        if !budget_seconds.is_finite() || budget_seconds <= 0.0 {
            return Err(format!(
                "{budget_seconds} seconds is not a budget a plan can be spent over"
            ));
        }
        let ratio = explore_ratio.clamp(0.0, 1.0);
        let rate = |phase: PlanPhase| -> Result<f64, String> {
            plan.phases
                .iter()
                .find(|row| row.phase == phase)
                .map(|row| row.safe_units_per_second)
                .ok_or_else(|| format!("the plan carries no {} rate", phase.as_str()))
        };
        // 80/20 of the wall budget, each half converted at its own phase's
        // safe rate. `floor`, because a fractional unit is not a unit and
        // rounding one up would spend budget the rate did not promise.
        //
        // Compress takes the **remainder**, `budget - explore`, and not
        // `budget * (1 - ratio)`. That is what `Pacer::Wall` does - explore
        // ends at `total_s * ratio` and compress runs to `total_s` - and it is
        // also the only form that is exact in binary: `10.0 * (1.0 - 0.8)` is
        // 1.9999999999999996, which silently loses a unit of the compress
        // allocation and every number derived from it.
        let explore_seconds = budget_seconds * ratio;
        let compress_seconds = budget_seconds - explore_seconds;
        let explore_allocation = (rate(PlanPhase::Explore)? * explore_seconds).floor();
        let compress_allocation = (rate(PlanPhase::Compress)? * compress_seconds).floor();
        for (phase, allocation) in [
            (PlanPhase::Explore, explore_allocation),
            (PlanPhase::Compress, compress_allocation),
        ] {
            if !allocation.is_finite() || allocation < 1.0 {
                return Err(format!(
                    "{}: an allocation of {allocation} units is not a phase",
                    phase.as_str()
                ));
            }
        }
        Ok(Self {
            clock,
            key: plan.key.clone(),
            currency: *currency,
            explore_allocation: explore_allocation as u64,
            compress_allocation: compress_allocation as u64,
            explore_consumed: 0,
            compress_consumed: 0,
            explore_batches: 0,
            compress_batches: 0,
            budget_seconds,
            explore_ratio: ratio,
        })
    }

    /// The clock the pacer was handed and never calls. Present so a vector can
    /// interrogate it; nothing in this type's own code path touches it.
    pub fn clock(&self) -> &C {
        &self.clock
    }

    pub fn key(&self) -> &PlanKey {
        &self.key
    }

    pub fn currency(&self) -> &Currency {
        &self.currency
    }

    pub fn budget_seconds(&self) -> f64 {
        self.budget_seconds
    }

    /// The frozen share, as this pacer received it.
    pub fn explore_ratio(&self) -> f64 {
        self.explore_ratio
    }

    pub fn allocation(&self, phase: PlanPhase) -> u64 {
        match phase {
            PlanPhase::Explore => self.explore_allocation,
            PlanPhase::Compress => self.compress_allocation,
        }
    }

    pub fn consumed(&self, phase: PlanPhase) -> u64 {
        match phase {
            PlanPhase::Explore => self.explore_consumed,
            PlanPhase::Compress => self.compress_consumed,
        }
    }

    pub fn remaining(&self, phase: PlanPhase) -> u64 {
        self.allocation(phase).saturating_sub(self.consumed(phase))
    }

    /// **The compress bite's decay parameter.** Consumed compress units over
    /// the compress allocation, through the frozen `time_based_step`.
    ///
    /// Defined at every moment, not only in compress: before compress starts
    /// it is the start of the range, which is what `time_based_step(0, limit)`
    /// returns anyway.
    pub fn compress_step(&self) -> f64 {
        homotopy::time_based_step(
            self.compress_consumed as f64,
            self.compress_allocation as f64,
        )
    }

    /// The boundary before the first batch of a phase. `units_charged` is
    /// zero: nothing has been spent yet.
    pub fn entry_boundary(&self, phase: PlanPhase) -> BatchBoundary {
        self.boundary(phase, 0)
    }

    /// **Charge one completed master batch, and answer at the boundary.**
    ///
    /// `terms` are the batch's own five counters - a delta, not a running
    /// total. [`WorkTerms::since`] is how a caller turns cumulative readings
    /// into one, and charging a cumulative reading here is precisely the
    /// spec's worst-ranked defect: "persistent-slot leakage / double-debit
    /// ('stable but false' work accounting)". The pacer cannot detect it -
    /// it has no way to know what it was handed - so the FAST identity that
    /// the per-batch deltas sum to the trajectory's own work vector is the
    /// tripwire, and it is not optional.
    pub fn charge_batch(&mut self, phase: PlanPhase, terms: &WorkTerms) -> BatchBoundary {
        let units = self.currency.units(terms);
        match phase {
            PlanPhase::Explore => {
                self.explore_consumed = self.explore_consumed.saturating_add(units);
                self.explore_batches += 1;
            }
            PlanPhase::Compress => {
                self.compress_consumed = self.compress_consumed.saturating_add(units);
                self.compress_batches += 1;
            }
        }
        self.boundary(phase, units)
    }

    fn boundary(&self, phase: PlanPhase, units_charged: u64) -> BatchBoundary {
        let consumed = self.consumed(phase);
        let allocation = self.allocation(phase);
        BatchBoundary {
            phase,
            units_charged,
            phase_consumed: consumed,
            phase_allocation: allocation,
            phase_exhausted: consumed >= allocation,
            phase_batches: match phase {
                PlanPhase::Explore => self.explore_batches,
                PlanPhase::Compress => self.compress_batches,
            },
            compress_step: self.compress_step(),
        }
    }
}

/// The two things a plan is keyed on that this module can restate without a
/// reader: the currency version, and the executor implementation.
///
/// Wave 1's rule is that nothing in the write path may also decide. These are
/// the *decision's* halves, kept away from the writer for the same reason.
pub fn plan_is_for(key: &PlanKey, currency: CurrencyVersion, executor: Executor) -> bool {
    key.currency_version == currency && key.executor == executor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::overlap_ics::icscal::{PhasePlan, SCHEMA};
    use crate::search::overlap_ics_meter::currency::{Coefficients, MeasuredPrices, Rounding};

    const SHA: &str = "ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3";
    const BIN: &str = "d9ef083e41beeee1cf189773a04ce7d789fc238dadcb349c6ad05e55cbd8120d";

    fn key(version: CurrencyVersion) -> PlanKey {
        PlanKey {
            request_sha256: SHA.to_owned(),
            currency_version: version,
            binary_key: BinaryKey {
                executable_sha256: BIN.to_owned(),
                features: vec!["overlap-ics".to_owned()],
            },
            workers: 8,
            executor: Executor::EphemeralScope,
        }
    }

    fn phase(phase: PlanPhase, rate: f64) -> PhasePlan {
        PhasePlan {
            phase,
            safe_units_per_second: rate,
            measured_units_per_second: rate / 0.8,
            safety_factor: 0.8,
            observed_units: 1_000_000,
            observed_seconds: 1.0,
            derivation: "test".to_owned(),
        }
    }

    fn plan_u0() -> WorkPlan {
        WorkPlan::new(
            key(CurrencyVersion::U0Samples),
            vec![
                phase(PlanPhase::Explore, 2_000_000.0),
                phase(PlanPhase::Compress, 1_000_000.0),
            ],
            "test",
        )
    }

    fn coefficients() -> Coefficients {
        Coefficients {
            b_master_batch: 300,
            e_publication_call: 2_500,
            r_repair_row: 40,
            d_disruption_move: 9_000,
            measured: MeasuredPrices {
                b_master_batch: 299.5,
                e_publication_call: 2_499.5,
                r_repair_row: 39.5,
                d_disruption_move: 8_999.5,
                base_ns_per_sample_evaluation: 100.0,
            },
            rounding: Rounding::ConservativeCeil,
        }
    }

    fn plan_u1() -> WorkPlan {
        WorkPlan::new(
            key(CurrencyVersion::U1Weighted),
            vec![
                phase(PlanPhase::Explore, 2_000_000.0),
                phase(PlanPhase::Compress, 1_000_000.0),
            ],
            "test",
        )
        .with_currency(coefficients())
    }

    fn terms(samples: u64, batches: u64, calls: u64, rows: u64, moves: u64) -> WorkTerms {
        WorkTerms {
            sample_evaluations: samples,
            master_batches: batches,
            actual_publication_attempt_calls: calls,
            repair_rows: rows,
            disruption_moves: moves,
        }
    }

    // ------------------------------------------------------- the clock rule --

    /// **The clock-poison vector.**
    ///
    /// A whole synthetic trajectory - both phases, several hundred batches,
    /// the compress decay read on every one of them, the phase boundary, the
    /// exhaustion verdicts - under a clock that panics when read. If any of it
    /// reaches for a clock, this test fails on the line that reached.
    #[test]
    fn a_paced_trajectory_never_reads_a_clock() {
        let plan = plan_u1();
        let currency = Currency::u1(coefficients());
        let mut pacer =
            WorkPlanPacer::from_plan(&plan, &currency, 10.0, 0.8, PoisonedClock::new()).unwrap();

        let mut boundary = pacer.entry_boundary(PlanPhase::Explore);
        let mut explore_batches = 0u64;
        while !boundary.phase_exhausted {
            boundary = pacer.charge_batch(
                PlanPhase::Explore,
                &terms(11_203 + explore_batches % 7_929, 1, 0, 0, 0),
            );
            // The decay is read every batch, exactly as a compress bite would.
            let _ = pacer.compress_step();
            explore_batches += 1;
            assert!(explore_batches < 100_000);
        }
        assert!(
            explore_batches > 100,
            "the explore phase was trivially short"
        );

        let mut boundary = pacer.entry_boundary(PlanPhase::Compress);
        let mut steps = vec![boundary.compress_step];
        let mut compress_batches = 0u64;
        while !boundary.phase_exhausted {
            boundary = pacer.charge_batch(PlanPhase::Compress, &terms(9_000, 1, 1, 3, 0));
            steps.push(boundary.compress_step);
            compress_batches += 1;
            assert!(compress_batches < 100_000);
        }
        assert!(compress_batches > 10);

        // The decay is monotone in consumed compress work and nothing else.
        for pair in steps.windows(2) {
            assert!(pair[1] <= pair[0], "the compress decay went backwards");
        }
        assert_eq!(
            pacer.clock().reads(),
            0,
            "the pacer reached for a clock {} times",
            pacer.clock().reads()
        );
    }

    /// The default clock is not a clock either: reading it is the defect, and
    /// it says so instead of returning a plausible zero.
    #[test]
    #[should_panic(expected = "has no clock")]
    fn the_default_clock_refuses_to_be_read() {
        let _ = NoClock.elapsed_seconds();
    }

    /// And the poisoned one panics rather than merely counting, so a vector
    /// that forgot to assert `reads() == 0` still fails.
    #[test]
    #[should_panic(expected = "poisoned clock")]
    fn the_poisoned_clock_panics_when_read() {
        let _ = PoisonedClock::new().elapsed_seconds();
    }

    // ------------------------------------------------------------- 80 / 20 --

    /// The frozen share splits the wall budget; each half converts at its own
    /// phase's safe rate. 10 s at 2 000 000 explore units/s and 1 000 000
    /// compress units/s is 16 000 000 and 2 000 000.
    #[test]
    fn eighty_twenty_is_spent_in_calibrated_units() {
        let plan = plan_u0();
        let pacer = WorkPlanPacer::from_plan(&plan, &Currency::U0, 10.0, 0.8, NoClock).unwrap();
        assert_eq!(pacer.allocation(PlanPhase::Explore), 16_000_000);
        assert_eq!(pacer.allocation(PlanPhase::Compress), 2_000_000);
        assert_eq!(pacer.explore_ratio(), 0.8);
    }

    /// The share is a parameter: this module holds no copy of `0.8`, and a
    /// caller that passes something else gets that something else. The clamp
    /// matches `Pacer::new`'s.
    #[test]
    fn the_share_is_a_parameter_and_clamps_like_the_shipped_pacer() {
        let plan = plan_u0();
        for (ratio, explore, compress) in [
            (0.5, 10_000_000u64, 5_000_000u64),
            (1.0, 20_000_000, 0),
            (2.0, 20_000_000, 0),
            (-1.0, 0, 10_000_000),
        ] {
            let built = WorkPlanPacer::from_plan(&plan, &Currency::U0, 10.0, ratio, NoClock);
            match built {
                Ok(pacer) => {
                    assert_eq!(pacer.allocation(PlanPhase::Explore), explore);
                    assert_eq!(pacer.allocation(PlanPhase::Compress), compress);
                }
                // A zero-unit phase is a refusal, not a phase.
                Err(message) => assert!(
                    explore == 0 || compress == 0,
                    "ratio {ratio} refused unexpectedly: {message}"
                ),
            }
        }
    }

    // ------------------------------------------------------- the bookkeeping --

    /// Consumption is `U` of each batch's own delta, and it accumulates
    /// exactly - no rounding per batch, because the coefficients are integers.
    #[test]
    fn units_are_consumed_between_master_batches() {
        let plan = plan_u1();
        let currency = Currency::u1(coefficients());
        let mut pacer = WorkPlanPacer::from_plan(&plan, &currency, 10.0, 0.8, NoClock).unwrap();
        let batch = terms(12_000, 1, 0, 0, 0);
        let per_batch = currency.units(&batch);
        assert_eq!(per_batch, 12_300);
        let entry = pacer.entry_boundary(PlanPhase::Explore);
        assert_eq!(entry.units_charged, 0);
        assert_eq!(entry.phase_consumed, 0);
        assert!(!entry.phase_exhausted);
        for index in 1..=100u64 {
            let boundary = pacer.charge_batch(PlanPhase::Explore, &batch);
            assert_eq!(boundary.units_charged, per_batch);
            assert_eq!(boundary.phase_consumed, per_batch * index);
            assert_eq!(boundary.phase_batches, index);
            assert_eq!(boundary.phase_allocation, 16_000_000);
        }
        assert_eq!(
            pacer.remaining(PlanPhase::Explore),
            16_000_000 - per_batch * 100
        );
        // Charging explore never touches compress.
        assert_eq!(pacer.consumed(PlanPhase::Compress), 0);
    }

    /// Exhaustion is `consumed >= allocation`, and the overshoot is at most
    /// one batch because the verdict is only ever read at a boundary.
    #[test]
    fn a_phase_is_exhausted_at_the_first_boundary_past_its_allocation() {
        let plan = plan_u0();
        let mut pacer = WorkPlanPacer::from_plan(&plan, &Currency::U0, 10.0, 0.8, NoClock).unwrap();
        let allocation = pacer.allocation(PlanPhase::Explore);
        let batch = terms(1_000_000, 0, 0, 0, 0);
        let mut last = pacer.entry_boundary(PlanPhase::Explore);
        while !last.phase_exhausted {
            last = pacer.charge_batch(PlanPhase::Explore, &batch);
        }
        assert!(last.phase_consumed >= allocation);
        assert!(last.phase_consumed - last.units_charged < allocation);
    }

    /// The compress decay is the frozen `time_based_step` of the consumed
    /// fraction: the range's start at zero consumption, its end at full.
    #[test]
    fn the_compress_decay_follows_consumed_compress_work() {
        let plan = plan_u0();
        let mut pacer = WorkPlanPacer::from_plan(&plan, &Currency::U0, 10.0, 0.8, NoClock).unwrap();
        let allocation = pacer.allocation(PlanPhase::Compress);
        assert_eq!(
            pacer.compress_step(),
            homotopy::time_based_step(0.0, allocation as f64)
        );
        pacer.charge_batch(PlanPhase::Compress, &terms(allocation / 2, 0, 0, 0, 0));
        assert_eq!(
            pacer.compress_step(),
            homotopy::time_based_step(allocation as f64 / 2.0, allocation as f64)
        );
        pacer.charge_batch(PlanPhase::Compress, &terms(allocation, 0, 0, 0, 0));
        // Past the allocation the step clamps at the end of the range rather
        // than running off it - `time_based_step` clamps the fraction.
        assert_eq!(pacer.compress_step(), homotopy::time_based_step(1.0, 1.0));
    }

    // ---------------------------------------------------------- hit / miss --

    #[test]
    fn an_identical_key_is_a_hit() {
        assert_eq!(
            match_plan(
                &key(CurrencyVersion::U0Samples),
                &key(CurrencyVersion::U0Samples)
            ),
            PlanMatch::Hit
        );
        assert!(match_plan(
            &key(CurrencyVersion::U1Weighted),
            &key(CurrencyVersion::U1Weighted)
        )
        .is_hit());
    }

    /// Every key field is a miss on its own, and the miss says which. A plan
    /// that matched on four of five would be a rate measured for a different
    /// question being spent on this one.
    #[test]
    fn every_key_field_is_a_miss_on_its_own() {
        let wanted = key(CurrencyVersion::U0Samples);

        let mut other = wanted.clone();
        other.request_sha256 = "0".repeat(64);
        assert!(
            matches!(match_plan(&wanted, &other), PlanMatch::Miss(m) if m.contains("requestSha256"))
        );

        let other = key(CurrencyVersion::U1Weighted);
        assert!(
            matches!(match_plan(&wanted, &other), PlanMatch::Miss(m) if m.contains("currencyVersion"))
        );

        let mut other = wanted.clone();
        other.binary_key.executable_sha256 = "1".repeat(64);
        assert!(
            matches!(match_plan(&wanted, &other), PlanMatch::Miss(m) if m.contains("executableSha256"))
        );

        let mut other = wanted.clone();
        other.binary_key.features.push("ics-profile".to_owned());
        assert!(
            matches!(match_plan(&wanted, &other), PlanMatch::Miss(m) if m.contains("features"))
        );

        let mut other = wanted.clone();
        other.workers = 4;
        assert!(matches!(match_plan(&wanted, &other), PlanMatch::Miss(m) if m.contains("workers")));

        let mut other = wanted.clone();
        other.executor = Executor::PersistentPool;
        assert!(
            matches!(match_plan(&wanted, &other), PlanMatch::Miss(m) if m.contains("executor"))
        );

        assert!(plan_is_for(
            &wanted,
            CurrencyVersion::U0Samples,
            Executor::EphemeralScope
        ));
        assert!(!plan_is_for(
            &wanted,
            CurrencyVersion::U0Samples,
            Executor::PersistentPool
        ));
    }

    /// The pacer refuses a plan denominated in a currency it is not being
    /// handed, and a `U1` plan whose pinned coefficients are not the ones
    /// being spent. Both are the "stable but false" failure with a version
    /// number attached.
    #[test]
    fn the_pacer_refuses_a_currency_mismatch() {
        assert!(WorkPlanPacer::from_plan(
            &plan_u0(),
            &Currency::u1(coefficients()),
            10.0,
            0.8,
            NoClock
        )
        .unwrap_err()
        .contains("denominated"));
        assert!(
            WorkPlanPacer::from_plan(&plan_u1(), &Currency::U0, 10.0, 0.8, NoClock)
                .unwrap_err()
                .contains("denominated")
        );
        let mut other = coefficients();
        other.r_repair_row = 41;
        assert!(
            WorkPlanPacer::from_plan(&plan_u1(), &Currency::u1(other), 10.0, 0.8, NoClock)
                .unwrap_err()
                .contains("pinned coefficients")
        );
    }

    /// A plan missing a phase rate, and a budget that is not a budget, are
    /// refusals rather than a pacer with a zero allocation somewhere.
    #[test]
    fn the_pacer_refuses_an_unpaceable_plan() {
        let explore_only = WorkPlan::new(
            key(CurrencyVersion::U0Samples),
            vec![phase(PlanPhase::Explore, 2_000_000.0)],
            "test",
        );
        assert!(
            WorkPlanPacer::from_plan(&explore_only, &Currency::U0, 10.0, 0.8, NoClock)
                .unwrap_err()
                .contains("compress")
        );
        for budget in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                WorkPlanPacer::from_plan(&plan_u0(), &Currency::U0, budget, 0.8, NoClock).is_err(),
                "budget {budget} was accepted"
            );
        }
        // A budget so small that a phase rounds to zero units is not a phase.
        assert!(WorkPlanPacer::from_plan(&plan_u0(), &Currency::U0, 1e-9, 0.8, NoClock).is_err());
    }

    // ------------------------------------------------------- the schema --

    /// **Wave 1's committed plan still parses, and re-serialises byte for
    /// byte.** The `currency` field is additive and optional, so the census's
    /// evidence file is not invalidated by this wave adding to the schema.
    #[test]
    fn the_committed_u0_plan_round_trips_unchanged() {
        const COMMITTED: &str = include_str!(
            "../../../../../docs/experiments/overlap-ics/economics-round/census/evidence/mixed61-w8-seed0.icscal.json"
        );
        let plan: WorkPlan = serde_json::from_str(COMMITTED).unwrap();
        assert_eq!(plan.schema, SCHEMA);
        assert_eq!(plan.key.currency_version, CurrencyVersion::U0Samples);
        assert!(plan.currency.is_none());
        let bytes = plan.to_bytes().unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            COMMITTED,
            "the wave-1 plan no longer re-serialises to its own bytes"
        );
    }

    /// The version and the coefficients have to agree, in both directions.
    #[test]
    fn a_plan_cannot_claim_a_currency_it_does_not_carry() {
        let mut u1_without = plan_u1();
        u1_without.currency = None;
        assert!(u1_without.validate().unwrap_err().contains("U1"));

        let u0_with = plan_u0().with_currency(coefficients());
        assert!(u0_with.validate().unwrap_err().contains("U0"));

        assert!(plan_u0().validate().is_ok());
        assert!(plan_u1().validate().is_ok());
    }

    /// A `U1` plan carries its coefficients through the file and back.
    #[test]
    fn a_u1_plan_carries_its_coefficients_through_the_file() {
        let plan = plan_u1();
        let bytes = plan.to_bytes().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\"bMasterBatch\": 300"));
        assert!(text.contains("conservative-ceil"));
        let parsed: WorkPlan = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, plan);
        assert_eq!(parsed.currency.unwrap().e_publication_call, 2_500);
    }
}
