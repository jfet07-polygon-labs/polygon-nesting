//! The parallel work currency: a price for every operator class, in one unit.
//!
//! # Why there is a second currency at all
//!
//! The shipped work meter is [`crate::search::portfolio`]'s
//! `work_units_now()`: candidate queries plus a constant per **narrow-phase**
//! exact pair test. It is exact, deterministic and cheap, and every pinned
//! number in this repository - the four regression gates, the opportunity
//! ledger's spends, `work=40000000`, the plan mode's `portfolio.plan.units` -
//! is denominated in it. It is not going to change, and nothing in this module
//! changes it.
//!
//! What it is *not* is comparable across operator classes.
//! `docs/experiments/basin-race/` §4.4 measured, inside one phase of one run:
//!
//! | operator | wall share | work share | units per second |
//! |---|---:|---:|---:|
//! | mode 20 | 70.8% | 0.0123% | 92.7 |
//! | mode 22 | 25.0% | 48.98% | 1,047,216 |
//! | mode 34 | 4.1% | 51.01% | 6,628,431 |
//!
//! A second of mode 34 costs **71,500x** what a second of mode 20 costs, in
//! the currency the coordinator's affordability rule, its share ceilings and
//! its class ranking all spend. So a work-denominated ceiling cannot bound the
//! constructor: the draw is, to the budget, very nearly free.
//!
//! # What this module is
//!
//! A **parallel** currency, spec-keyed and off by default, built out of the
//! one pattern the campaign already trusts: the compression schedule's own
//! meter (`CompressionSchedule::work_units`), which the coordinator settles as
//! `max(global_delta, operator_self_units)`. That pattern is generalised here
//! from one operator to every class.
//!
//! Two halves, and the split is the whole design:
//!
//! * **the counts are deterministic.** Every input is a counter the engine
//!   already increments, from two places: five of the process-wide profiling
//!   array, differenced across one operator call, and four the *operator*
//!   reports for the call itself, plus the schedule's confirmations. See
//!   [`ClassCounts`] for why the second group had to exist. Two processes
//!   running the same work-budgeted arm see the same vector, which is what
//!   makes a budget denominated in this currency reproducible at all.
//! * **the weights are a machine profile.** `ns` per count, per class, fitted
//!   on one box (§ [`WORK_CURRENCY_PROFILE`]). They are *not* deterministic
//!   facts about the search; they are facts about the hardware, and a
//!   different box wants a different table. That is why they are a named
//!   constant with a driver that refits them rather than a number inline at a
//!   call site.
//!
//! The unit is chosen so the currency can be settled with `max` against the
//! shipped meter without ever *lowering* a price: see
//! [`WORK_CURRENCY_REFERENCE_RATE`].
//!
//! # Integer arithmetic, deliberately
//!
//! Every price in here is integer. The debit path feeds
//! `BudgetMeter::self_metered_debit`, which is `u64` for the reason Sol review
//! 6 §1 gave - a 53-bit mantissa between a counter that is exact and a budget
//! compared against it is a reproducibility bug waiting for a large enough
//! run. Weights are stored scaled by [`WORK_CURRENCY_SCALE`] so a fitted
//! multiplier of `2.375` is the integer `2375` and nothing rounds twice.

/// The fixed-point scale the per-count weights in [`ClassPrice`] are stored
/// at: a weight of `1_000` prices one count at one currency unit.
///
/// Three decimal digits is enough for every fitted weight in
/// [`WORK_CURRENCY_PROFILE`] and small enough that the accumulator cannot
/// overflow: the largest count any single operator call in the measured band
/// reports is under 2^32, and the largest weight is under 2^32, so the product
/// fits `u64` with room. `saturating_*` throughout regardless.
pub const WORK_CURRENCY_SCALE: u64 = 1_000;

/// The shipped meter's own two coefficients, at this module's scale.
///
/// Derived from [`crate::search::portfolio::WORK_UNITS_PER_EXACT_PAIR_TEST`]
/// rather than written out, and that is not tidiness. The first cut of this
/// module wrote `43_000` here from memory; the real constant is `5`, so every
/// class self-priced at 38x the shipped meter per exact pair test and the
/// `max` settlement charged the difference on **every operator in the run**,
/// not only on the class the currency was built for. It survived a unit test,
/// because the test computed its own expectation with the same wrong literal -
/// the failure mode Sol review 8 §1 names, a test that "ricopia la
/// conclusione". What caught it was `chargedExtraUnits` being non-zero on a
/// mode-22 call in a run with no mode-20 calls in it at all, which is why that
/// field is reported per call rather than summed.
///
/// `the_default_price_is_derived_from_the_shipped_meter_not_copied_from_it`
/// is the regression, and it compares against `work_units_now()`'s own
/// constant so the two cannot drift apart again.
const SHIPPED_CANDIDATE_QUERY: u64 = WORK_CURRENCY_SCALE;
const SHIPPED_EXACT_PAIR_TEST: u64 =
    crate::search::portfolio::WORK_UNITS_PER_EXACT_PAIR_TEST * WORK_CURRENCY_SCALE;

/// The rate, in shipped-meter units per second, that one currency unit is
/// pinned to.
///
/// This is the number that makes `max(global_delta, self_units)` safe. The
/// settlement rule can only ever *raise* what an operator is charged - that is
/// what makes it impossible for a repricing to manufacture budget a run did
/// not have - so the currency has to price every class at a rate no *lower*
/// than the rate the shipped meter already charges it at. Concretely: for a
/// class the shipped meter retires at `r_c` units per second, a self-price of
/// `wall * R` is at least the global delta whenever `R >= r_c`.
///
/// `2_600_000` is above the pooled rate of every class that carries the
/// campaign's spend (mode 22 at 2.55 M/s over 227 measured calls, mode 34 at
/// 1.72 M/s over 117) and below the two classes the shipped meter already
/// over-prices relative to them (mode 23's pooled 3.61 M/s). Those two are
/// deliberately left alone: `max` keeps their global delta, which is the
/// higher price, and a currency whose job is to stop under-pricing must not
/// become a discount.
pub const WORK_CURRENCY_REFERENCE_RATE: u64 = 2_600_000;

/// One operator call's own deterministic count vector.
///
/// **Two sources, and the second one is the whole finding.** The first four
/// fields are deltas of `profiling::counter_totals()` across one call - the
/// array the shipped meter already reads, which a work-budgeted run has armed
/// anyway. The next four are the *operator's own* account of itself,
/// `GeneralPersistentVacancyWorkDiagnostics`, which is a per-call structure
/// and needs no delta.
///
/// The second group exists because of what `docs/experiments/work-currency/`
/// §1.2 measured: over seven coordinator mode-20 calls totalling **9.353
/// seconds**, the profiling array recorded **zero** candidate queries, **zero**
/// neighbour tests, **zero** collision polygon builds and 165 exact pair
/// tests. The constructor is not merely under-priced by the two counters the
/// meter reads - it is invisible to the entire array, because the layered
/// construction scores through its own position-source pipeline and never
/// enters the relaxed lane's `score_placement`. No weighting of the profiling
/// counters can price it, at any exchange rate. Its own diagnostics, in the
/// same call, report 9.8 M position-source attempts and 13.1 M pair visits.
///
/// So this is the generalisation of the compression schedule's self-meter and
/// not of the coordinator's counter: an operator that knows what it did says
/// so, and the currency asks it rather than watching a global array.
///
/// `AcceptedMoves`, `EffectivePieceMoves` and `PublicationAttempts` are
/// deliberately absent from both groups: they are outcomes rather than work,
/// and a currency that priced an *accepted* move would charge an operator for
/// succeeding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassCounts {
    // ---- the profiling array, as a delta across the call ----
    pub candidate_queries: u64,
    pub exact_pair_tests: u64,
    pub collision_builds: u64,
    pub neighbor_tests: u64,
    pub full_rescores: u64,
    // ---- the operator's own per-call account ----
    /// `work.position_source_attempts`: the constructor's analogue of a
    /// candidate query - one position the placement pipeline considered.
    pub position_source_attempts: u64,
    /// `work.returned_positions`: the subset that survived to be scored.
    pub returned_positions: u64,
    /// `work.experimental_pair_visits + work.validator_pair_visits`: the
    /// operator's own analogue of an exact pair test.
    pub pair_visits: u64,
    /// `work.experimental_collision_builds + work.validator_collision_builds`:
    /// transformed-and-offset collision polygons the call constructed. This is
    /// the count the profiling array's `CollisionPolygonBuilds` would carry if
    /// `search-profiling` were compiled, and it is zero in every shipped
    /// binary - which is why the currency reads the operator's copy.
    pub operator_collision_builds: u64,
    /// Whole-layout exact confirmations the compression schedule attempted.
    /// Zero for every operator that is not a mode-34 slice.
    pub confirmations: u64,
}

impl ClassCounts {
    /// The component-wise difference `after - before` over the **profiling**
    /// half, saturating at zero. The operator's own half is per-call already
    /// and is carried through from `after` untouched.
    ///
    /// Saturating rather than wrapping because the profiling registry sums
    /// over thread blocks and a block that retired between the two reads would
    /// otherwise turn a small negative into 2^64.
    pub fn delta(after: &Self, before: &Self) -> Self {
        Self {
            candidate_queries: after
                .candidate_queries
                .saturating_sub(before.candidate_queries),
            exact_pair_tests: after
                .exact_pair_tests
                .saturating_sub(before.exact_pair_tests),
            collision_builds: after
                .collision_builds
                .saturating_sub(before.collision_builds),
            neighbor_tests: after.neighbor_tests.saturating_sub(before.neighbor_tests),
            full_rescores: after.full_rescores.saturating_sub(before.full_rescores),
            position_source_attempts: after.position_source_attempts,
            returned_positions: after.returned_positions,
            pair_visits: after.pair_visits,
            operator_collision_builds: after.operator_collision_builds,
            confirmations: after.confirmations,
        }
    }

    /// Whether this call reported no countable work at all.
    ///
    /// Read by the fitting driver rather than by the engine: a class whose
    /// calls are all empty is a class the currency cannot price, and that is a
    /// finding rather than a zero to average in.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The per-count weights of one operator class, scaled by
/// [`WORK_CURRENCY_SCALE`].
///
/// A weight is `ns_per_count * WORK_CURRENCY_REFERENCE_RATE / 1e9 *
/// WORK_CURRENCY_SCALE` - that is, "how many currency units one of these
/// counts is worth", which is the nanoseconds it takes on the profile box
/// converted at the reference rate. The driver that fits them
/// (`docs/experiments/work-currency/drivers/fitprofile.py`) prints exactly
/// that arithmetic per class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassPrice {
    pub candidate_query: u64,
    pub exact_pair_test: u64,
    pub collision_build: u64,
    pub neighbor_test: u64,
    pub full_rescore: u64,
    pub position_source_attempt: u64,
    pub returned_position: u64,
    pub pair_visit: u64,
    pub operator_collision_build: u64,
    pub confirmation: u64,
    /// A flat charge for the call itself, in whole currency units (**not**
    /// scaled): the part of a class's wall that is neither per-query nor
    /// per-pair - a constructor's sort and layer bookkeeping, a slice's entry
    /// repair, the caches a call builds before its first query.
    ///
    /// It is a *floor* on one call, not a rate, so it cannot be out-run by a
    /// call that does nothing; and it is zero for every class whose own counts
    /// already predict its wall, because a floor that is not needed is a
    /// constant that will be wrong on the next request.
    pub per_call: u64,
}

impl ClassPrice {
    /// What one call of this class charges itself, in currency units.
    ///
    /// Integer throughout: the scaled dot product is accumulated in `u64` and
    /// divided by [`WORK_CURRENCY_SCALE`] exactly once, at the end, so the
    /// result is a function of the counts and the table and of nothing else.
    pub fn units(&self, counts: &ClassCounts) -> u64 {
        let scaled = self
            .candidate_query
            .saturating_mul(counts.candidate_queries)
            .saturating_add(self.exact_pair_test.saturating_mul(counts.exact_pair_tests))
            .saturating_add(self.collision_build.saturating_mul(counts.collision_builds))
            .saturating_add(self.neighbor_test.saturating_mul(counts.neighbor_tests))
            .saturating_add(self.full_rescore.saturating_mul(counts.full_rescores))
            .saturating_add(
                self.position_source_attempt
                    .saturating_mul(counts.position_source_attempts),
            )
            .saturating_add(
                self.returned_position
                    .saturating_mul(counts.returned_positions),
            )
            .saturating_add(self.pair_visit.saturating_mul(counts.pair_visits))
            .saturating_add(
                self.operator_collision_build
                    .saturating_mul(counts.operator_collision_builds),
            )
            .saturating_add(self.confirmation.saturating_mul(counts.confirmations));
        (scaled / WORK_CURRENCY_SCALE).saturating_add(self.per_call)
    }
}

/// The machine profile: one [`ClassPrice`] per operator class.
///
/// **These are hardware numbers, not engine numbers.** They were fitted on the
/// campaign's x86_64 box (16 cores) from the per-class wall and per-class
/// count vectors in `docs/experiments/work-currency/evidence/rates.json`; a
/// different box wants a different table and `drivers/fitprofile.py` is how it
/// gets one. Nothing about the *counts* is in here, and nothing in here is
/// read unless the currency is armed.
///
/// The default arm - the class the table does not name - is
/// [`DEFAULT_CLASS_PRICE`], and it is deliberately the cheap one: an unmeasured
/// class must not be repriced on a guess.
pub fn price_for(mode: usize) -> ClassPrice {
    WORK_CURRENCY_PROFILE
        .iter()
        .find(|(class, _)| *class == mode)
        .map(|(_, price)| *price)
        .unwrap_or(DEFAULT_CLASS_PRICE)
}

/// See [`price_for`]. Fitted on the box named in the module docs by
/// `drivers/fitprofile.py`; the fit and its residuals are
/// `docs/experiments/work-currency/` §2.
pub const WORK_CURRENCY_PROFILE: &[(usize, ClassPrice)] = &[
    // mode 20 - the constructor draw, and the one class in the measured set
    // the shipped meter under-prices at all.
    //
    // The meter reads **89 units per second** here against mode 22's 2.72 M -
    // a ratio of 3.4e-05 - and it cannot do better: §1.2 measured the whole
    // profiling array at zero over 9.246 s of draw except 165 exact pair
    // tests. So this class's price comes entirely from its own account, and
    // `operator_collision_build` is the count that carries it: a
    // transformed-and-offset collision polygon the construction built. Of the
    // five candidates the fit ranked it is the one whose charge lands closest
    // to `wall * REFERENCE_RATE` across the three fixtures (geometric RMS of
    // the ratio 1.703, against 1.811 for the runner-up), and it is also the
    // count the profiling array's own `CollisionPolygonBuilds` *would* have
    // carried if `search-profiling` were compiled - which it is not in any
    // shipped binary, and is why the currency reads the operator's copy.
    //
    // `candidate_query` and `exact_pair_test` keep the shipped meter's own
    // coefficients, so a draw that somehow did enter the relaxed lane is never
    // charged *less* than the global counter would have charged it.
    (
        20,
        ClassPrice {
            candidate_query: SHIPPED_CANDIDATE_QUERY,
            exact_pair_test: SHIPPED_EXACT_PAIR_TEST,
            collision_build: 0,
            neighbor_test: 0,
            full_rescore: 0,
            position_source_attempt: 0,
            returned_position: 0,
            pair_visit: 0,
            operator_collision_build: WORK_CURRENCY_M20_COLLISION_BUILD_WEIGHT,
            confirmation: 0,
            per_call: 0,
        },
    ),
];

/// Mode 20's fitted weight on `operator_collision_builds`, scaled by
/// [`WORK_CURRENCY_SCALE`]: **82.506 currency units per collision build**,
/// from a median 31,513 builds per second on the profile box.
///
/// Its own constant because it is the one number in this module a different
/// box has to refit, and a reader looking for "what would I change" should
/// find one name rather than a field inside a table.
/// `drivers/fitprofile.py` is how a different box gets one; run twice hours
/// apart on two builds it returned 82,506 and 82,605, which is **0.12%** and
/// is this round's only statement about how reproducible the fit is.
///
/// **Its residual is wide, and stating that is the point.** Charged against
/// `wall * WORK_CURRENCY_REFERENCE_RATE` on the seven measured calls the
/// charge lands at 0.999 and 1.023 on mixed-61, 0.792-0.817 on shapes-17 and
/// 2.586-2.628 on triangle-20: a spread of **3.32x** across three fixtures,
/// and within a fixture 2.4% and 3.2%. The comparison that puts it in
/// proportion is the shipped meter's own spread on the *same seven calls*,
/// which is **5.63x** (38.8 to 218.6 units per second) at 1/29,000 of the
/// level. The currency does not make a draw's price exact; it makes it the
/// right order of magnitude, with a within-class spread tighter than the one
/// the shipped meter already tolerates - and that is the difference between a
/// ceiling that binds and one that does not.
pub const WORK_CURRENCY_M20_COLLISION_BUILD_WEIGHT: u64 = 82_506;

/// The price of a class the profile does not name: the shipped meter's own
/// two counts and nothing else, so an unmeasured class self-prices at exactly
/// what the global counter already charges it and `max` is a no-op.
///
/// This is the fail-safe direction. A class the currency has never measured
/// must not be repriced by extrapolation from a class it has: the whole
/// finding this module exists for is that two operator classes' rates differ
/// by four and a half orders of magnitude, so there is no such thing as a
/// representative class.
pub const DEFAULT_CLASS_PRICE: ClassPrice = ClassPrice {
    candidate_query: SHIPPED_CANDIDATE_QUERY,
    exact_pair_test: SHIPPED_EXACT_PAIR_TEST,
    collision_build: 0,
    neighbor_test: 0,
    full_rescore: 0,
    position_source_attempt: 0,
    returned_position: 0,
    pair_visit: 0,
    operator_collision_build: 0,
    confirmation: 0,
    per_call: 0,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression for the bug in [`SHIPPED_EXACT_PAIR_TEST`]'s docs.
    ///
    /// It asserts against `work_units_now()`'s **own** constant, not against a
    /// literal retyped here, because a test that retypes the constant it is
    /// checking passes whatever the constant is - which is exactly how the
    /// `43` survived. The pinned pair below is a real measured mode-22 call
    /// from `evidence/rates.json`, and the third assertion states the number
    /// the run produced, so a change to the shipped meter breaks this test
    /// loudly rather than silently re-deriving itself.
    #[test]
    fn the_default_price_is_derived_from_the_shipped_meter_not_copied_from_it() {
        use crate::search::portfolio::WORK_UNITS_PER_EXACT_PAIR_TEST;
        let counts = ClassCounts {
            candidate_queries: 2_007_788,
            exact_pair_tests: 1_082,
            // The three counts an unmeasured class must be charged *nothing*
            // for, or the currency would reprice a class it never measured.
            neighbor_tests: 7_247_175,
            full_rescores: 1_912,
            pair_visits: 999_999,
            ..ClassCounts::default()
        };
        let shipped = counts.candidate_queries
            + WORK_UNITS_PER_EXACT_PAIR_TEST * counts.exact_pair_tests;
        assert_eq!(DEFAULT_CLASS_PRICE.units(&counts), shipped);
        // And the number that call actually reported to the coordinator.
        assert_eq!(shipped, 2_013_198);
    }

    #[test]
    fn a_delta_of_a_smaller_reading_saturates_rather_than_wrapping() {
        let before = ClassCounts {
            candidate_queries: 10,
            ..ClassCounts::default()
        };
        let after = ClassCounts::default();
        assert_eq!(ClassCounts::delta(&after, &before), ClassCounts::default());
    }

    #[test]
    fn the_operators_own_half_is_not_differenced() {
        // The profiling half is a process-global reading and has to be
        // differenced; the operator's half is already one call's account and
        // must not be. A delta that subtracted it would charge the *second*
        // mode-20 call of a run for less than the first, which is the bug this
        // separation exists to make impossible.
        let before = ClassCounts {
            candidate_queries: 100,
            operator_collision_builds: 99_000,
            ..ClassCounts::default()
        };
        let after = ClassCounts {
            candidate_queries: 150,
            operator_collision_builds: 99_066,
            ..ClassCounts::default()
        };
        let delta = ClassCounts::delta(&after, &before);
        assert_eq!(delta.candidate_queries, 50);
        assert_eq!(delta.operator_collision_builds, 99_066);
    }

    #[test]
    fn a_mode_twenty_draw_is_priced_four_orders_of_magnitude_above_the_meter() {
        // The measured mixed-61 race draw of `docs/experiments/work-currency/`
        // §2.3, row 1: 3.147 s of wall, 310 shipped-meter units, 99,066
        // collision builds of its own. The currency has to turn that into
        // something the affordability rule can see, and §4.1 is what happens
        // when it does.
        let counts = ClassCounts {
            operator_collision_builds: 99_066,
            ..ClassCounts::default()
        };
        let shipped = 310u64;
        let priced = price_for(20).units(&counts);
        // 3.147 s at the reference rate is 8,183,300 units. The pinned charge
        // is asserted exactly rather than within a band, so a change to the
        // weight or to the arithmetic has to be re-recorded here and in §2.3
        // together.
        assert_eq!(priced, 8_173_539);
        assert_eq!(priced / shipped, 26_366);
    }

    #[test]
    fn an_unnamed_class_is_charged_exactly_what_the_shipped_meter_charges_it() {
        // Mode 22's own measured call: the currency must be a no-op on it,
        // because `max(global, class)` with a class price below the global
        // delta keeps the global delta - and a currency that repriced a class
        // it measured as already comparable would be moving a number for no
        // reason. Modes 22, 23, 26 and 34 are all in this arm.
        use crate::search::portfolio::WORK_UNITS_PER_EXACT_PAIR_TEST;
        let counts = ClassCounts {
            candidate_queries: 2_007_788,
            exact_pair_tests: 1_082,
            neighbor_tests: 7_247_175,
            full_rescores: 1_912,
            ..ClassCounts::default()
        };
        let shipped = counts.candidate_queries
            + WORK_UNITS_PER_EXACT_PAIR_TEST * counts.exact_pair_tests;
        for mode in [22usize, 23, 26, 27, 30, 31, 34] {
            assert_eq!(
                price_for(mode).units(&counts),
                shipped,
                "mode {mode} must self-price at the shipped meter"
            );
        }
    }

    #[test]
    fn every_named_class_prices_the_shipped_meters_own_counts_at_its_own_rate() {
        // A class in the profile may add counts, and it may not *discount* the
        // two the shipped meter reads: `max` protects the budget from a low
        // self-price, but a class whose table zeroed those two would make the
        // reported `classUnits` a number no reader could compare against
        // `globalUnits`, and §3's gap table is exactly that comparison.
        for (mode, price) in WORK_CURRENCY_PROFILE {
            assert_eq!(
                price.candidate_query, DEFAULT_CLASS_PRICE.candidate_query,
                "mode {mode} discounts candidate queries"
            );
            assert_eq!(
                price.exact_pair_test, DEFAULT_CLASS_PRICE.exact_pair_test,
                "mode {mode} discounts exact pair tests"
            );
        }
    }

    #[test]
    fn the_price_is_integer_and_therefore_reproducible() {
        // Two orderings of the same accumulation must agree exactly. A float
        // dot product over ten terms would not promise this, and the budget
        // this feeds is compared for bit equality across two processes.
        let counts = ClassCounts {
            candidate_queries: 7,
            exact_pair_tests: 11,
            collision_builds: 13,
            neighbor_tests: 17,
            full_rescores: 19,
            position_source_attempts: 23,
            returned_positions: 29,
            pair_visits: 31,
            operator_collision_builds: 37,
            confirmations: 41,
        };
        let price = ClassPrice {
            candidate_query: 1_001,
            exact_pair_test: 5_003,
            collision_build: 7,
            neighbor_test: 11,
            full_rescore: 13,
            position_source_attempt: 17,
            returned_position: 19,
            pair_visit: 23,
            operator_collision_build: 82_506,
            confirmation: 29,
            per_call: 5,
        };
        let expected = (1_001 * 7
            + 5_003 * 11
            + 7 * 13
            + 11 * 17
            + 13 * 19
            + 17 * 23
            + 19 * 29
            + 23 * 31
            + 82_506 * 37
            + 29 * 41)
            / WORK_CURRENCY_SCALE
            + 5;
        assert_eq!(price.units(&counts), expected);
    }

    #[test]
    fn a_saturating_count_cannot_panic_or_wrap_the_budget() {
        let counts = ClassCounts {
            operator_collision_builds: u64::MAX,
            ..ClassCounts::default()
        };
        // The only contract is that this terminates with a finite number the
        // budget can compare; a saturated price is a budget that is over, which
        // is the fail-closed direction.
        assert!(price_for(20).units(&counts) > 0);
    }
}
