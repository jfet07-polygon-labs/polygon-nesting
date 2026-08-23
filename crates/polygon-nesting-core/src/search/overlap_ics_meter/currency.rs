//! **The calibrated-work currency `U`, and the check that may reject it.**
//!
//! docs/economics-round-spec.md, funded change 3, verbatim:
//!
//! > currency `U = sample_evaluations + B·master_batches
//! > + E·actual_publication_attempt_calls + R·repair_rows + D·disruption_moves`;
//! > B/E/R/D from timing-only microbenchmarks on all three fixtures,
//! > conservative rounding; **REJECT the currency if wall-prediction error
//! > >10 % on any transfer fixture.**
//!
//! Three things live here and nothing else: the currency itself
//! ([`Currency`], [`WorkTerms`]), the derivation of its coefficients from
//! timings ([`calibrate`]), and the reject check ([`transfer_check`]).
//! Deriving is deliberately separate from spending: [`Currency::units`] cannot
//! measure anything and [`calibrate`] cannot pace anything, which is the same
//! read/write separation `icscal` is built around and for the same reason -
//! *"no live probe on a gated trajectory"*.
//!
//! # Why the base unit is a sample evaluation and the other four are prices
//!
//! The formula is not a weighted average; it is a **vector reduced by an
//! exchange rate**, and the rate is "how many sample evaluations would have
//! cost the same wall". `sample_evaluations` therefore has an implicit
//! coefficient of exactly 1 and is never calibrated - it *is* the unit. The
//! four others are integers because a currency with fractional prices has a
//! rounding rule per addition instead of one rounding rule per round.
//!
//! # Which way "conservative" points
//!
//! A coefficient that is too **low** under-charges the term, so a trajectory
//! spends fewer units than the wall it really burns and **overruns**. A
//! coefficient that is too **high** over-charges it, so the trajectory stops
//! early and **under-runs**. The spec's only hard time clause is a p95 wall
//! ceiling; there is no floor. So conservative is *up*, and [`Rounding`] has
//! exactly one variant for that reason. It is the same direction
//! `icscal::PhasePlan`'s safety factor already points, and the two compose:
//! prices rounded up, spent against a rate discounted down.
//!
//! It is the **rounding** that is conservative, and only the rounding. Prices
//! themselves are pooled across the fixtures rather than taken at their
//! extremes; [`calibrate`]'s own docs record what happened when an earlier
//! draft inflated them as well, and why a currency that systematically
//! over-predicts wall fails the spec's >10 % clause rather than satisfying its
//! caution.
//!
//! # What the harness can and cannot see
//!
//! [`timings_from_rows`] turns the per-bite census rows the `ics-profile`
//! build already emits into the timings [`calibrate`] needs.
//!
//! **Every reading is wall, and that is not an accident.** `U` exists to
//! predict wall, so a price denominated in CPU nanoseconds cannot appear in
//! it: the base unit is read from `sweepCriticalNs`, the longest single worker
//! sweep, and not from `sweepTotalNs`, which sums eight of them. Mixing the
//! two under-prices a sample evaluation by the worker count and over-prices
//! every other term by it, which is a currency that is stable, arithmetically
//! consistent and wrong - this round's named failure mode. The wall of one
//! bite is then `sweepCritical + batch overhead + exact`, which is exactly
//! `barrierToBarrier`, plus the residual outside it.
//!
//! One of the readings is not direct, and the module says so rather than
//! hiding it: **no timer exists around `disrupt::disrupt`**, because adding
//! one would mean editing `search/overlap_ics/mod.rs`, which this wave may not
//! touch. `D` is therefore derived from the per-bite wall that a disrupting
//! bite spent *outside* its own barrier-to-barrier accounting - an **upper
//! bound** on the disruption's cost, since pool restore and pose installation
//! are in that residual too. An upper bound is the conservative direction, and
//! the transfer check is what decides whether the bound is tight enough to
//! ship. A later round that wants a tight `D` adds one timer and re-derives.

use serde::{Deserialize, Serialize};

use crate::search::overlap_ics::icscal::CurrencyVersion;
use crate::search::overlap_ics::profile::PhaseProfile;

/// The spec's rejection threshold: wall-prediction error above this on **any**
/// transfer fixture rejects the currency.
pub const WALL_PREDICTION_TOLERANCE: f64 = 0.10;

/// The five counted terms of one window - a bite, a phase or a whole
/// trajectory. Counters only: no field of this struct costs a clock read, so
/// the currency is measurable in a build with no profiling feature at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkTerms {
    /// The base unit. Implicit coefficient 1, never calibrated.
    pub sample_evaluations: u64,
    /// Master batches, i.e. eight-worker tournaments. `B`.
    pub master_batches: u64,
    /// Times the exact authorities were actually asked - **not** band
    /// entries. `E`. The audit's F4 split is what makes this term nameable.
    pub actual_publication_attempt_calls: u64,
    /// Rows the 16 µm repair moved. `R`.
    pub repair_rows: u64,
    /// Pieces an Algorithm-12 disruption moved, followers included. `D`.
    pub disruption_moves: u64,
}

impl WorkTerms {
    /// The terms of one bite, from the census profile that charged them to it.
    ///
    /// `PhaseProfile::iterations` is `master_batches` and
    /// `PhaseProfile::exact_calls` is `actual_publication_attempt_calls`; the
    /// mapping is here, once, so nothing downstream has to remember it.
    pub fn from_profile(profile: &PhaseProfile) -> Self {
        Self {
            sample_evaluations: profile.sample_evaluations,
            master_batches: profile.iterations,
            actual_publication_attempt_calls: profile.exact_calls,
            repair_rows: profile.repair_rows,
            disruption_moves: profile.disruption_moves,
        }
    }

    pub fn add(&mut self, other: &Self) {
        self.sample_evaluations = self
            .sample_evaluations
            .saturating_add(other.sample_evaluations);
        self.master_batches = self.master_batches.saturating_add(other.master_batches);
        self.actual_publication_attempt_calls = self
            .actual_publication_attempt_calls
            .saturating_add(other.actual_publication_attempt_calls);
        self.repair_rows = self.repair_rows.saturating_add(other.repair_rows);
        self.disruption_moves = self.disruption_moves.saturating_add(other.disruption_moves);
    }

    /// The delta between two cumulative readings, saturating at zero.
    ///
    /// The spec's worst-ranked defect class is "double-debit": work charged
    /// twice because a persistent slot was not zeroed. Deltas are how a caller
    /// charges a window rather than a running total, and this is the one place
    /// that subtraction is written.
    pub fn since(&self, earlier: &Self) -> Self {
        Self {
            sample_evaluations: self
                .sample_evaluations
                .saturating_sub(earlier.sample_evaluations),
            master_batches: self.master_batches.saturating_sub(earlier.master_batches),
            actual_publication_attempt_calls: self
                .actual_publication_attempt_calls
                .saturating_sub(earlier.actual_publication_attempt_calls),
            repair_rows: self.repair_rows.saturating_sub(earlier.repair_rows),
            disruption_moves: self
                .disruption_moves
                .saturating_sub(earlier.disruption_moves),
        }
    }
}

/// How a measured price became an integer coefficient. One variant, because
/// the spec licenses exactly one direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rounding {
    /// `ceil`. A price rounded up over-charges the term, which makes a plan
    /// stop early rather than overrun.
    #[serde(rename = "conservative-ceil")]
    ConservativeCeil,
}

impl Rounding {
    fn apply(self, price: f64) -> u64 {
        match self {
            Self::ConservativeCeil => price.ceil() as u64,
        }
    }
}

/// `B`, `E`, `R`, `D` - and the measured prices they were rounded from, so a
/// reader can see the rounding instead of trusting it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coefficients {
    /// Sample-evaluation equivalents of one master batch.
    pub b_master_batch: u64,
    /// ... of one actual publication attempt call.
    pub e_publication_call: u64,
    /// ... of one repair row.
    pub r_repair_row: u64,
    /// ... of one disruption move.
    pub d_disruption_move: u64,
    /// The unrounded prices, in the same order.
    pub measured: MeasuredPrices,
    pub rounding: Rounding,
}

/// The unrounded exchange rates, in sample-evaluation equivalents.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasuredPrices {
    pub b_master_batch: f64,
    pub e_publication_call: f64,
    pub r_repair_row: f64,
    pub d_disruption_move: f64,
    /// Nanoseconds of one sample evaluation - the denominator every price
    /// above was divided by. The cheapest reading across the fixtures.
    pub base_ns_per_sample_evaluation: f64,
}

/// The currency a plan is denominated in.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    pub version: CurrencyVersion,
    /// `None` for `U0`, which has no coefficients because it is
    /// `sample_evaluations` and nothing else.
    pub coefficients: Option<Coefficients>,
}

impl Currency {
    /// Wave 1's honest currency: `U = sample_evaluations`. Not the spec's `U`
    /// and must not be read as it.
    pub const U0: Self = Self {
        version: CurrencyVersion::U0Samples,
        coefficients: None,
    };

    /// The spec's currency, with measured coefficients.
    pub fn u1(coefficients: Coefficients) -> Self {
        Self {
            version: CurrencyVersion::U1Weighted,
            coefficients: Some(coefficients),
        }
    }

    /// **`U`.** Saturating, because a currency that wrapped would be the
    /// "stable but false" accounting the spec ranks first among its defects.
    pub fn units(&self, terms: &WorkTerms) -> u64 {
        let Some(coefficients) = self.coefficients.as_ref() else {
            return terms.sample_evaluations;
        };
        let mut units = terms.sample_evaluations;
        for (count, price) in [
            (terms.master_batches, coefficients.b_master_batch),
            (
                terms.actual_publication_attempt_calls,
                coefficients.e_publication_call,
            ),
            (terms.repair_rows, coefficients.r_repair_row),
            (terms.disruption_moves, coefficients.d_disruption_move),
        ] {
            units = units.saturating_add(count.saturating_mul(price));
        }
        units
    }

    /// A one-line summary for a driver's log. Never a file.
    pub fn summary(&self) -> String {
        match self.coefficients.as_ref() {
            None => format!("{} U=sampleEvaluations", self.version.as_str()),
            Some(c) => format!(
                "{} B={} E={} R={} D={}",
                self.version.as_str(),
                c.b_master_batch,
                c.e_publication_call,
                c.r_repair_row,
                c.d_disruption_move
            ),
        }
    }
}

// ---------------------------------------------------------------- calibration --

/// One of the five terms, as a timing addresses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Term {
    SampleEvaluation,
    MasterBatch,
    PublicationAttemptCall,
    RepairRow,
    DisruptionMove,
}

impl Term {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SampleEvaluation => "sample-evaluation",
            Self::MasterBatch => "master-batch",
            Self::PublicationAttemptCall => "publication-attempt-call",
            Self::RepairRow => "repair-row",
            Self::DisruptionMove => "disruption-move",
        }
    }

    /// The four calibrated terms, in the order `U` writes them.
    pub const PRICED: [Self; 4] = [
        Self::MasterBatch,
        Self::PublicationAttemptCall,
        Self::RepairRow,
        Self::DisruptionMove,
    ];
}

/// One timing-only measurement: how long `count` occurrences of `term` took on
/// `fixture`. Nothing here knows how the nanoseconds were obtained, which is
/// what lets a direct microbenchmark and a census residual be compared on the
/// same page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermTiming {
    pub fixture: String,
    pub term: Term,
    pub nanos: u64,
    pub count: u64,
    /// Free text: which cell, and whether the reading is direct or a bound.
    pub derivation: String,
}

impl TermTiming {
    fn ns_each(&self) -> f64 {
        self.nanos as f64 / self.count as f64
    }
}

/// What [`calibrate`] produced, with every intermediate a reader would
/// otherwise have to re-derive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    pub currency: Currency,
    /// Fixtures that contributed, sorted, so the record says how many the
    /// "all three fixtures" clause really saw.
    pub fixtures: Vec<String>,
    /// Every term's pooled price, the fixtures that contributed to it, and the
    /// per-fixture spread the pooling hides. [`Term::SampleEvaluation`] is in
    /// the list with `rounded: 1`, because the base unit is a price too and a
    /// reader should not have to know it is implicit.
    pub selected: Vec<TermPrice>,
}

/// One term's price, pooled across every fixture that could read it, with the
/// spread the pooling hides printed beside it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermPrice {
    pub term: Term,
    /// The fixtures that contributed a reading, sorted.
    pub fixtures: Vec<String>,
    /// Total nanoseconds over total occurrences: an occurrence-weighted mean,
    /// not a mean of the per-fixture rates, so a fixture with three
    /// occurrences does not outvote one with three thousand.
    pub pooled_nanos_each: f64,
    /// The cheapest and dearest per-fixture readings. **The spread is the
    /// honest error bar on this coefficient**, and a wide one is the first
    /// thing to look at when the transfer check rejects.
    pub cheapest_nanos_each: f64,
    pub dearest_nanos_each: f64,
    /// `pooled_nanos_each` over the pooled base sample-evaluation cost.
    pub sample_evaluation_equivalents: f64,
    pub rounded: u64,
}

/// Total nanoseconds over total occurrences for one term, with the cheapest
/// and dearest per-fixture readings and the fixtures that contributed.
fn pooled_price(timings: &[TermTiming], term: Term) -> Option<(f64, f64, f64, Vec<String>)> {
    let rows: Vec<&TermTiming> = timings.iter().filter(|row| row.term == term).collect();
    if rows.is_empty() {
        return None;
    }
    let nanos: u128 = rows.iter().map(|row| row.nanos as u128).sum();
    let count: u128 = rows.iter().map(|row| row.count as u128).sum();
    if count == 0 {
        return None;
    }
    let mut fixtures: Vec<String> = rows.iter().map(|row| row.fixture.clone()).collect();
    fixtures.sort();
    fixtures.dedup();
    let cheapest = rows
        .iter()
        .map(|row| row.ns_each())
        .fold(f64::INFINITY, f64::min);
    let dearest = rows.iter().map(|row| row.ns_each()).fold(0.0f64, f64::max);
    Some((nanos as f64 / count as f64, cheapest, dearest, fixtures))
}

/// **The derivation, in one function.**
///
/// * The base unit is the pooled nanoseconds per sample evaluation - total
///   sweep **wall** over total evaluations, across every fixture.
/// * Each priced term is the pooled nanoseconds per occurrence, over the same
///   base.
/// * The quotient is then rounded **up**.
///
/// # Which part of this is "conservative", and which part is not
///
/// The spec's sentence is *"convert them to equivalent sample-evaluation
/// units, and round conservatively"*. The **rounding** is conservative -
/// [`Rounding::ConservativeCeil`], for the reason in the module docs. The
/// **price** is not inflated on top of it, and an earlier draft of this
/// function that took the dearest per-fixture reading over the cheapest base
/// was measurably worse: on the three campaign cells it multiplied every
/// coefficient by the spread between the fixtures and moved the mixed-61 ->
/// shapes-17 transfer error from 16 % to 158 %. A currency exists to predict
/// wall; a rule that systematically over-predicts it is not cautious, it is
/// inaccurate, and the spec's own >10 % clause is what says so. The
/// per-fixture spread is printed instead of being baked in.
///
/// Refuses rather than guesses. A term with no reading, a zero count, a
/// non-finite reading or a fixture set that does not cover all three campaign
/// fixtures is an `Err`: a coefficient invented for a term nobody measured is
/// exactly the "stable but false" accounting this round exists to prevent, and
/// a zero coefficient would price that term free forever.
pub fn calibrate(
    timings: &[TermTiming],
    required_fixtures: &[&str],
) -> Result<Calibration, String> {
    let mut fixtures: Vec<String> = timings.iter().map(|row| row.fixture.clone()).collect();
    fixtures.sort();
    fixtures.dedup();
    for required in required_fixtures {
        if !fixtures.iter().any(|seen| seen == required) {
            return Err(format!(
                "the spec calibrates on all three fixtures; `{required}` has no timing"
            ));
        }
    }
    for row in timings {
        if row.count == 0 {
            return Err(format!(
                "{} on {}: a price needs occurrences, not {} in {} ns",
                row.term.as_str(),
                row.fixture,
                row.count,
                row.nanos
            ));
        }
        if !row.ns_each().is_finite() || row.ns_each() <= 0.0 {
            return Err(format!(
                "{} on {}: {} ns over {} occurrences is not a positive finite price",
                row.term.as_str(),
                row.fixture,
                row.nanos,
                row.count
            ));
        }
    }

    let (base_ns, base_cheapest, base_dearest, base_fixtures) =
        pooled_price(timings, Term::SampleEvaluation).ok_or_else(|| {
            "no sample-evaluation timing: the currency has no base unit".to_owned()
        })?;
    if !base_ns.is_finite() || base_ns <= 0.0 {
        return Err(format!(
            "the base unit is {base_ns} ns, which prices nothing"
        ));
    }

    let mut selected = Vec::with_capacity(Term::PRICED.len() + 1);
    selected.push(TermPrice {
        term: Term::SampleEvaluation,
        fixtures: base_fixtures,
        pooled_nanos_each: base_ns,
        cheapest_nanos_each: base_cheapest,
        dearest_nanos_each: base_dearest,
        sample_evaluation_equivalents: 1.0,
        rounded: 1,
    });
    for term in Term::PRICED {
        let (price_ns, cheapest, dearest, term_fixtures) =
            pooled_price(timings, term).ok_or_else(|| {
                format!(
                    "{} has no timing; `U` may not price a term nobody measured",
                    term.as_str()
                )
            })?;
        let equivalents = price_ns / base_ns;
        if !equivalents.is_finite() {
            return Err(format!("{}: price is not finite", term.as_str()));
        }
        selected.push(TermPrice {
            term,
            fixtures: term_fixtures,
            pooled_nanos_each: price_ns,
            cheapest_nanos_each: cheapest,
            dearest_nanos_each: dearest,
            sample_evaluation_equivalents: equivalents,
            rounded: Rounding::ConservativeCeil.apply(equivalents),
        });
    }

    let price = |term: Term| -> u64 {
        selected
            .iter()
            .find(|row| row.term == term)
            .map(|row| row.rounded)
            .unwrap_or_default()
    };
    let measured_price = |term: Term| -> f64 {
        selected
            .iter()
            .find(|row| row.term == term)
            .map(|row| row.sample_evaluation_equivalents)
            .unwrap_or_default()
    };
    let coefficients = Coefficients {
        b_master_batch: price(Term::MasterBatch),
        e_publication_call: price(Term::PublicationAttemptCall),
        r_repair_row: price(Term::RepairRow),
        d_disruption_move: price(Term::DisruptionMove),
        measured: MeasuredPrices {
            b_master_batch: measured_price(Term::MasterBatch),
            e_publication_call: measured_price(Term::PublicationAttemptCall),
            r_repair_row: measured_price(Term::RepairRow),
            d_disruption_move: measured_price(Term::DisruptionMove),
            base_ns_per_sample_evaluation: base_ns,
        },
        rounding: Rounding::ConservativeCeil,
    };
    Ok(Calibration {
        currency: Currency::u1(coefficients),
        fixtures,
        selected,
    })
}

// ------------------------------------------------- the publication cost split --

/// One bite's publication accounting: how many calls reached exact geometry,
/// how many rows the repair moved, and the wall both together spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationCell {
    pub calls: u64,
    pub repair_rows: u64,
    pub exact_ns: u64,
}

/// **Splitting `exact_ns` into a per-call price and a per-row price.**
///
/// `Engine::attempt_publication` is timed as one region, so `E` and `R` are
/// not separately readable from the census. They are separately *identifiable*
/// across bites, because most bites repair nothing: a bite with calls and no
/// rows prices `E` alone, and the bites that do repair carry `R`.
///
/// This is the ordinary two-parameter least-squares fit of
/// `exact_ns ≈ e·calls + r·rows`, with the non-negativity constraint applied
/// the standard way: solve unconstrained, and if either price comes out
/// negative, pin it at zero and re-solve the remaining one-parameter problem.
/// A negative price is not a cheaper term, it is a fit reading noise, and the
/// constraint is what keeps `R = -3` out of a currency.
///
/// Refuses on a degenerate design - all rows zero, or calls and rows perfectly
/// collinear - because then the two prices are not separable and any split is
/// an invention.
pub fn split_publication_cost(cells: &[PublicationCell]) -> Result<(f64, f64), String> {
    let mut sum_cc = 0.0f64;
    let mut sum_cw = 0.0f64;
    let mut sum_ww = 0.0f64;
    let mut sum_ct = 0.0f64;
    let mut sum_wt = 0.0f64;
    let mut any = false;
    for cell in cells {
        if cell.calls == 0 && cell.repair_rows == 0 {
            continue;
        }
        any = true;
        let c = cell.calls as f64;
        let w = cell.repair_rows as f64;
        let t = cell.exact_ns as f64;
        sum_cc += c * c;
        sum_cw += c * w;
        sum_ww += w * w;
        sum_ct += c * t;
        sum_wt += w * t;
    }
    if !any {
        return Err("no publication cell has a call or a repair row".to_owned());
    }
    if sum_ww <= 0.0 {
        return Err(
            "no repair row was ever observed; `R` cannot be priced from these cells".to_owned(),
        );
    }
    if sum_cc <= 0.0 {
        return Err("no publication call was ever observed; `E` cannot be priced".to_owned());
    }
    let determinant = sum_cc * sum_ww - sum_cw * sum_cw;
    // Collinear within a relative epsilon: `calls` and `rows` moved together on
    // every cell, so the fit cannot tell the two prices apart.
    if !determinant.is_finite() || determinant.abs() <= 1e-9 * sum_cc * sum_ww {
        return Err(
            "calls and repair rows are collinear across the cells; the split is not identifiable"
                .to_owned(),
        );
    }
    let e = (sum_ww * sum_ct - sum_cw * sum_wt) / determinant;
    let r = (sum_cc * sum_wt - sum_cw * sum_ct) / determinant;
    let (e, r) = if e < 0.0 {
        (0.0, sum_wt / sum_ww)
    } else if r < 0.0 {
        (sum_ct / sum_cc, 0.0)
    } else {
        (e, r)
    };
    if !e.is_finite() || !r.is_finite() {
        return Err("the publication split is not finite".to_owned());
    }
    Ok((e, r))
}

// -------------------------------------------------------------- the harness --

/// One bite's row of the census document, as the `ics-profile` example already
/// emits it. The harness reads these; it does not produce them, and it cannot
/// run anything.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BiteProfileRow {
    pub fixture: String,
    pub iterations: u64,
    pub barrier_to_barrier_ns: u64,
    pub prep_ns: u64,
    pub dispatch_ns: u64,
    /// The longest single worker sweep: the sweeps' **wall**, and therefore
    /// the base unit's numerator.
    pub sweep_critical_ns: u64,
    /// Every worker's sweep, summed. Recorded, never the base: it is CPU
    /// nanoseconds, roughly eight times the wall at eight workers, and
    /// dividing a serial overhead by it would under-price a sample evaluation
    /// by that factor and over-price every other term by it.
    pub sweep_total_ns: u64,
    pub merge_gls_ns: u64,
    pub exact_ns: u64,
    pub band_fold_ns: u64,
    pub snapshot_ns: u64,
    pub residual_ns: u64,
    pub sample_evaluations: u64,
    pub exact_calls: u64,
    pub repair_rows: u64,
    pub disruption_moves: u64,
}

/// One fixture's whole search wall, measured by the driver around the phases.
///
/// The census times regions *inside* `Engine::separate` and nothing outside
/// it, so the disruption's own cost is only visible as the part of the search
/// that the separations did not account for. This carries that outer number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellWall {
    pub fixture: String,
    /// The whole post-constructor search, in nanoseconds, as the driver timed
    /// it around the phases.
    pub search_ns: u64,
}

impl BiteProfileRow {
    /// Everything in the barrier that is **not** a worker sweep: preparation,
    /// dispatch, the ordinal merge and Algorithm-8 pass, the band fold, the
    /// snapshot copies and the loop's own residual. The per-batch overhead the
    /// `B` coefficient prices.
    fn batch_overhead_ns(&self) -> u64 {
        self.prep_ns
            .saturating_add(self.dispatch_ns)
            .saturating_add(self.merge_gls_ns)
            .saturating_add(self.band_fold_ns)
            .saturating_add(self.snapshot_ns)
            .saturating_add(self.residual_ns)
    }
}

/// A term one fixture could not price, and why.
///
/// **A skip is not a failure and it is not a zero.** The three campaign
/// fixtures have genuinely different economics - triangle-20 publishes every
/// bite in one iteration and never repairs, shapes-17 fails its first explore
/// bite and never reaches the band at all - so no single fixture spends all
/// five terms. The spec's "on all three fixtures" is about the *set* of
/// readings, and this is how the set records which fixture contributed what.
/// [`calibrate`] still refuses if a term ends up with no reading anywhere.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTiming {
    pub fixture: String,
    pub term: Term,
    pub reason: String,
}

/// What the harness produced: the timings, and the terms it could not price.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessReport {
    pub timings: Vec<TermTiming>,
    pub skipped: Vec<SkippedTiming>,
}

/// **The timing-only harness**: per-bite census rows in, priced timings out.
///
/// Two terms are direct readings of regions the census already times.
/// `E` and `R` share one timed region and are separated across bites, or - on
/// a fixture that never repaired - `E` is read directly and `R` is skipped.
/// `D` is the residual described in the module docs and is labelled as a bound
/// in its `derivation` string, so a document that ships it cannot look like a
/// direct measurement.
pub fn timings_from_rows(
    rows: &[BiteProfileRow],
    walls: &[CellWall],
) -> Result<HarnessReport, String> {
    let mut skipped: Vec<SkippedTiming> = Vec::new();
    let mut fixtures: Vec<String> = rows.iter().map(|row| row.fixture.clone()).collect();
    fixtures.sort();
    fixtures.dedup();
    let mut timings = Vec::new();
    for fixture in fixtures {
        let cell: Vec<&BiteProfileRow> = rows.iter().filter(|row| row.fixture == fixture).collect();
        let sum = |pick: fn(&BiteProfileRow) -> u64| -> u64 {
            cell.iter()
                .fold(0u64, |acc, row| acc.saturating_add(pick(row)))
        };
        let sweep_ns = sum(|row| row.sweep_critical_ns);
        let samples = sum(|row| row.sample_evaluations);
        if sweep_ns == 0 {
            return Err(format!(
                "{fixture}: no sweep nanoseconds. Build the cell with `--features ics-profile`."
            ));
        }
        timings.push(TermTiming {
            fixture: fixture.clone(),
            term: Term::SampleEvaluation,
            nanos: sweep_ns,
            count: samples,
            derivation: "direct: sweepCriticalNs (the sweeps' wall) over the bite's own \
                         all-workers sampleEvaluations"
                .to_owned(),
        });
        timings.push(TermTiming {
            fixture: fixture.clone(),
            term: Term::MasterBatch,
            nanos: sum(BiteProfileRow::batch_overhead_ns),
            count: sum(|row| row.iterations),
            derivation: "direct: prep + dispatch + mergeGls + bandFold + snapshot + residual"
                .to_owned(),
        });

        // `E` and `R` share one timed region and are separated across bites -
        // when the fixture repaired at all. When it did not, `E` is a direct
        // reading and `R` is skipped rather than fitted out of nothing.
        let calls = sum(|row| row.exact_calls);
        let repair = sum(|row| row.repair_rows);
        let exact_ns = sum(|row| row.exact_ns);
        let mut skip = |term: Term, reason: &str| {
            skipped.push(SkippedTiming {
                fixture: fixture.clone(),
                term,
                reason: reason.to_owned(),
            });
        };
        if calls == 0 {
            skip(
                Term::PublicationAttemptCall,
                "the cell never reached exact geometry: no call to price",
            );
            skip(
                Term::RepairRow,
                "the cell never reached exact geometry, so it never repaired",
            );
        } else if repair == 0 {
            timings.push(TermTiming {
                fixture: fixture.clone(),
                term: Term::PublicationAttemptCall,
                nanos: exact_ns,
                count: calls,
                derivation: "direct: exactNs over the calls, on a cell that never repaired"
                    .to_owned(),
            });
            skip(Term::RepairRow, "the cell repaired no row");
        } else {
            let publication: Vec<PublicationCell> = cell
                .iter()
                .map(|row| PublicationCell {
                    calls: row.exact_calls,
                    repair_rows: row.repair_rows,
                    exact_ns: row.exact_ns,
                })
                .collect();
            match split_publication_cost(&publication) {
                Ok((e_ns, r_ns)) => {
                    let fitted =
                        "fitted: non-negative least squares of exactNs on (calls, repairRows)";
                    timings.push(TermTiming {
                        fixture: fixture.clone(),
                        term: Term::PublicationAttemptCall,
                        nanos: (e_ns * calls as f64).round() as u64,
                        count: calls,
                        derivation: fitted.to_owned(),
                    });
                    timings.push(TermTiming {
                        fixture: fixture.clone(),
                        term: Term::RepairRow,
                        nanos: (r_ns * repair as f64).round() as u64,
                        count: repair,
                        derivation: fitted.to_owned(),
                    });
                }
                Err(error) => {
                    skip(Term::PublicationAttemptCall, &error);
                    skip(Term::RepairRow, &error);
                }
            }
        }

        // `D`: the bound, named as one.
        let moves = sum(|row| row.disruption_moves);
        let Some(wall) = walls.iter().find(|row| row.fixture == fixture) else {
            return Err(format!(
                "{fixture}: no search wall, so `D` has no residual to be bounded by"
            ));
        };
        let barriers = sum(|row| row.barrier_to_barrier_ns);
        let outside = wall.search_ns.saturating_sub(barriers);
        if moves == 0 {
            skip(Term::DisruptionMove, "the cell disrupted nothing");
        } else if outside == 0 {
            skip(
                Term::DisruptionMove,
                "the separations account for the whole search wall, so the residual is \
                 zero and `D` would be priced free",
            );
        } else {
            timings.push(TermTiming {
                fixture: fixture.clone(),
                term: Term::DisruptionMove,
                nanos: outside,
                count: moves,
                derivation: "UPPER BOUND: the whole search wall the separations' own \
                             barrier-to-barrier accounting did not claim - which also \
                             contains the pool restore, the pose installation, the cut and \
                             the publication commit. No timer exists around \
                             `disrupt::disrupt`, because adding one means editing \
                             search/overlap_ics/mod.rs. If the transfer check rejects, this \
                             bound is the first suspect."
                    .to_owned(),
            });
        }
    }
    Ok(HarnessReport { timings, skipped })
}

// -------------------------------------------------- the >10 % reject check --

/// One fixture's whole cell: what it counted, and how long it took.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureCell {
    pub fixture: String,
    pub terms: WorkTerms,
    pub seconds: f64,
}

/// One transfer prediction and its verdict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallPrediction {
    pub calibrated_on: String,
    pub transfer_fixture: String,
    pub units: u64,
    pub units_per_second: f64,
    pub predicted_seconds: f64,
    pub observed_seconds: f64,
    /// `|predicted - observed| / observed`.
    pub relative_error: f64,
    pub within_tolerance: bool,
}

/// The whole check: every fixture as the calibration fixture in turn, every
/// other as a transfer fixture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCheck {
    pub currency: Currency,
    pub tolerance: f64,
    pub predictions: Vec<WallPrediction>,
    /// The largest `relative_error` in `predictions`.
    pub worst_relative_error: f64,
    /// The spec's verdict. `false` **rejects the currency**.
    pub accepted: bool,
    /// The fixture pair that rejected it, when one did.
    pub rejected_by: Option<String>,
}

/// **`REJECT the currency if wall-prediction error >10 % on any transfer
/// fixture.`**
///
/// Leave-one-out over the cells: price a units-per-second rate on one fixture,
/// predict every other fixture's wall from its own counted terms, and compare
/// with what that fixture really took. A currency that transfers is one whose
/// exchange rate is a property of the machine rather than of the fixture it
/// was measured on; that is the whole claim `U` makes, and this is the only
/// thing that tests it.
///
/// Needs at least two cells. One cell can only predict itself, which it does
/// perfectly and meaninglessly.
pub fn transfer_check(currency: &Currency, cells: &[FixtureCell]) -> Result<TransferCheck, String> {
    if cells.len() < 2 {
        return Err(format!(
            "a transfer check needs at least two fixtures, not {}",
            cells.len()
        ));
    }
    for cell in cells {
        if !cell.seconds.is_finite() || cell.seconds <= 0.0 {
            return Err(format!(
                "{}: {} seconds is not a wall a rate can be priced against",
                cell.fixture, cell.seconds
            ));
        }
        if currency.units(&cell.terms) == 0 {
            return Err(format!("{}: zero units, so no rate exists", cell.fixture));
        }
    }
    let mut predictions = Vec::new();
    for source in cells {
        let units_per_second = currency.units(&source.terms) as f64 / source.seconds;
        for target in cells {
            if target.fixture == source.fixture {
                continue;
            }
            let units = currency.units(&target.terms);
            let predicted = units as f64 / units_per_second;
            let error = (predicted - target.seconds).abs() / target.seconds;
            predictions.push(WallPrediction {
                calibrated_on: source.fixture.clone(),
                transfer_fixture: target.fixture.clone(),
                units,
                units_per_second,
                predicted_seconds: predicted,
                observed_seconds: target.seconds,
                relative_error: error,
                within_tolerance: error <= WALL_PREDICTION_TOLERANCE,
            });
        }
    }
    let worst = predictions
        .iter()
        .map(|row| row.relative_error)
        .fold(0.0f64, f64::max);
    let rejected = predictions.iter().find(|row| !row.within_tolerance);
    Ok(TransferCheck {
        currency: *currency,
        tolerance: WALL_PREDICTION_TOLERANCE,
        worst_relative_error: worst,
        accepted: rejected.is_none(),
        rejected_by: rejected
            .map(|row| format!("{} -> {}", row.calibrated_on, row.transfer_fixture)),
        predictions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(samples: u64, batches: u64, calls: u64, rows: u64, moves: u64) -> WorkTerms {
        WorkTerms {
            sample_evaluations: samples,
            master_batches: batches,
            actual_publication_attempt_calls: calls,
            repair_rows: rows,
            disruption_moves: moves,
        }
    }

    fn coefficients(b: u64, e: u64, r: u64, d: u64) -> Coefficients {
        Coefficients {
            b_master_batch: b,
            e_publication_call: e,
            r_repair_row: r,
            d_disruption_move: d,
            measured: MeasuredPrices {
                b_master_batch: b as f64,
                e_publication_call: e as f64,
                r_repair_row: r as f64,
                d_disruption_move: d as f64,
                base_ns_per_sample_evaluation: 1.0,
            },
            rounding: Rounding::ConservativeCeil,
        }
    }

    /// `U` is the spec's five-term sum and nothing else, term by term.
    #[test]
    fn u_is_the_five_term_sum() {
        let currency = Currency::u1(coefficients(3, 5, 7, 11));
        assert_eq!(currency.units(&terms(0, 0, 0, 0, 0)), 0);
        assert_eq!(currency.units(&terms(100, 0, 0, 0, 0)), 100);
        assert_eq!(currency.units(&terms(0, 1, 0, 0, 0)), 3);
        assert_eq!(currency.units(&terms(0, 0, 1, 0, 0)), 5);
        assert_eq!(currency.units(&terms(0, 0, 0, 1, 0)), 7);
        assert_eq!(currency.units(&terms(0, 0, 0, 0, 1)), 11);
        assert_eq!(
            currency.units(&terms(1_000, 10, 4, 2, 6)),
            1_000 + 30 + 20 + 14 + 66
        );
    }

    /// `U0` is `sample_evaluations`, and no coefficient can leak into it.
    #[test]
    fn u0_is_sample_evaluations_alone() {
        assert_eq!(Currency::U0.units(&terms(7, 9, 9, 9, 9)), 7);
        assert_eq!(Currency::U0.version, CurrencyVersion::U0Samples);
        assert!(Currency::U0.coefficients.is_none());
    }

    /// The currency is additive over windows, which is what lets a pacer
    /// charge between master batches instead of re-totalling a trajectory.
    #[test]
    fn units_are_additive_over_windows() {
        let currency = Currency::u1(coefficients(3, 5, 7, 11));
        let first = terms(500, 6, 2, 1, 0);
        let second = terms(700, 9, 1, 0, 4);
        let mut both = first;
        both.add(&second);
        assert_eq!(
            currency.units(&both),
            currency.units(&first) + currency.units(&second)
        );
    }

    /// `since` is a delta and it saturates rather than wrapping, which is the
    /// double-debit tripwire in miniature.
    #[test]
    fn since_is_a_saturating_delta() {
        let later = terms(1_000, 10, 3, 2, 1);
        let earlier = terms(400, 4, 1, 0, 0);
        assert_eq!(later.since(&earlier), terms(600, 6, 2, 2, 1));
        // A reading that went backwards is clamped, never wrapped.
        assert_eq!(earlier.since(&later), terms(0, 0, 0, 0, 0));
    }

    #[test]
    fn work_terms_read_the_profile_the_census_emits() {
        let profile = PhaseProfile {
            iterations: 12,
            sample_evaluations: 34_567,
            exact_calls: 5,
            repair_rows: 3,
            disruption_moves: 7,
            ..PhaseProfile::default()
        };
        assert_eq!(
            WorkTerms::from_profile(&profile),
            terms(34_567, 12, 5, 3, 7)
        );
    }

    // ------------------------------------------------------- calibration --

    fn timing(fixture: &str, term: Term, nanos: u64, count: u64) -> TermTiming {
        TermTiming {
            fixture: fixture.to_owned(),
            term,
            nanos,
            count,
            derivation: "test".to_owned(),
        }
    }

    fn three_fixture_timings() -> Vec<TermTiming> {
        let mut rows = Vec::new();
        // Sample evaluations: 100 ns, 120 ns, 140 ns each. The cheapest, 100,
        // is the base.
        for (fixture, ns) in [("mixed-61", 100), ("shapes-17", 120), ("triangle-20", 140)] {
            rows.push(timing(fixture, Term::SampleEvaluation, ns * 1_000, 1_000));
        }
        // A master batch: 30 500 / 31 000 / 28 000 ns. The dearest is 31 000.
        for (fixture, ns) in [
            ("mixed-61", 30_500),
            ("shapes-17", 31_000),
            ("triangle-20", 28_000),
        ] {
            rows.push(timing(fixture, Term::MasterBatch, ns * 100, 100));
        }
        for (fixture, ns) in [
            ("mixed-61", 250_000),
            ("shapes-17", 240_000),
            ("triangle-20", 190_000),
        ] {
            rows.push(timing(fixture, Term::PublicationAttemptCall, ns * 20, 20));
        }
        for (fixture, ns) in [
            ("mixed-61", 4_000),
            ("shapes-17", 4_400),
            ("triangle-20", 3_900),
        ] {
            rows.push(timing(fixture, Term::RepairRow, ns * 50, 50));
        }
        for (fixture, ns) in [
            ("mixed-61", 900_000),
            ("shapes-17", 850_000),
            ("triangle-20", 700_000),
        ] {
            rows.push(timing(fixture, Term::DisruptionMove, ns * 10, 10));
        }
        rows
    }

    const FIXTURES: [&str; 3] = ["mixed-61", "shapes-17", "triangle-20"];

    /// The conservative rule: dearest term over cheapest base, rounded up.
    #[test]
    fn calibration_pools_across_the_fixtures_and_rounds_up() {
        let calibration = calibrate(&three_fixture_timings(), &FIXTURES).unwrap();
        let coefficients = calibration.currency.coefficients.unwrap();
        // Base: (100 + 120 + 140) ns x 1 000 each over 3 000 = 120 ns.
        assert_eq!(coefficients.measured.base_ns_per_sample_evaluation, 120.0);
        // (30 500 + 31 000 + 28 000) / 3 = 29 833.33 ns; / 120 = 248.6 -> 249.
        assert_eq!(coefficients.b_master_batch, 249);
        // (250 000 + 240 000 + 190 000) / 3 = 226 666.67; / 120 = 1 888.9 -> 1 889.
        assert_eq!(coefficients.e_publication_call, 1_889);
        // (4 000 + 4 400 + 3 900) / 3 = 4 100; / 120 = 34.2 -> 35.
        assert_eq!(coefficients.r_repair_row, 35);
        // (900 000 + 850 000 + 700 000) / 3 = 816 666.67; / 120 = 6 805.6 -> 6 806.
        assert_eq!(coefficients.d_disruption_move, 6_806);
        assert_eq!(calibration.currency.version, CurrencyVersion::U1Weighted);

        // Every coefficient is above the cheapest fixture's reading and below
        // the dearest, and the record prints both so the spread is visible
        // rather than hidden inside the pooled number.
        let batch = calibration
            .selected
            .iter()
            .find(|row| row.term == Term::MasterBatch)
            .unwrap();
        assert_eq!(batch.fixtures, FIXTURES.map(str::to_owned).to_vec());
        assert_eq!(batch.cheapest_nanos_each, 28_000.0);
        assert_eq!(batch.dearest_nanos_each, 31_000.0);
        assert!(batch.pooled_nanos_each > 28_000.0 && batch.pooled_nanos_each < 31_000.0);

        // The base unit is itself a priced term in the record, at 1.
        let base = calibration
            .selected
            .iter()
            .find(|row| row.term == Term::SampleEvaluation)
            .unwrap();
        assert_eq!(base.rounded, 1);
        assert_eq!(base.sample_evaluation_equivalents, 1.0);
    }

    /// Rounding is up, never to nearest: a price of 310.0001 is 311 units, and
    /// a price of exactly 310 stays 310.
    #[test]
    fn rounding_is_conservative_and_never_to_nearest() {
        // One reading per term, so the pooled price *is* the reading and the
        // rounding is the only thing under test.
        let base: Vec<TermTiming> = FIXTURES
            .iter()
            .map(|fixture| timing(fixture, Term::SampleEvaluation, 100_000, 1_000))
            .collect();
        let with_batch = |nanos: u64| {
            let mut rows = base.clone();
            rows.push(timing("mixed-61", Term::MasterBatch, nanos, 100));
            rows.push(timing("mixed-61", Term::PublicationAttemptCall, 1, 1));
            rows.push(timing("mixed-61", Term::RepairRow, 1, 1));
            rows.push(timing("mixed-61", Term::DisruptionMove, 1, 1));
            calibrate(&rows, &FIXTURES)
                .unwrap()
                .currency
                .coefficients
                .unwrap()
        };
        // 31 000.10 ns / 100 ns = 310.001 -> 311.
        let over = with_batch(3_100_010);
        assert_eq!(over.b_master_batch, 311);
        assert!(over.measured.b_master_batch > 310.0);
        assert!(over.measured.b_master_batch < 311.0);
        // 31 000 ns / 100 ns = 310 exactly, and `ceil` leaves it alone.
        let exact = with_batch(3_100_000);
        assert_eq!(exact.b_master_batch, 310);
        // A term cheaper than one sample evaluation still costs one unit: a
        // free term is a term that can be spent without limit.
        assert_eq!(over.r_repair_row, 1);
    }

    /// A missing fixture, a missing term and a zero count are all refusals.
    /// A currency that priced an unmeasured term free would be the spec's own
    /// worst defect class wearing a coefficient.
    #[test]
    fn calibration_refuses_rather_than_inventing_a_price() {
        let all = three_fixture_timings();

        let two_fixtures: Vec<TermTiming> = all
            .iter()
            .filter(|row| row.fixture != "triangle-20")
            .cloned()
            .collect();
        assert!(calibrate(&two_fixtures, &FIXTURES)
            .unwrap_err()
            .contains("triangle-20"));

        let no_repair: Vec<TermTiming> = all
            .iter()
            .filter(|row| row.term != Term::RepairRow)
            .cloned()
            .collect();
        assert!(calibrate(&no_repair, &FIXTURES)
            .unwrap_err()
            .contains("repair-row"));

        let mut zero_count = all.clone();
        zero_count.push(timing("mixed-61", Term::DisruptionMove, 5, 0));
        assert!(calibrate(&zero_count, &FIXTURES).is_err());

        let no_base: Vec<TermTiming> = all
            .iter()
            .filter(|row| row.term != Term::SampleEvaluation)
            .cloned()
            .collect();
        assert!(calibrate(&no_base, &[]).is_err());
    }

    // -------------------------------------------------- publication split --

    /// Exact recovery on noiseless data with a known `E` and `R`.
    #[test]
    fn the_publication_split_recovers_known_prices() {
        let e = 250_000.0f64;
        let r = 4_400.0f64;
        let cells: Vec<PublicationCell> = [(3u64, 0u64), (7, 0), (2, 11), (5, 40), (1, 2)]
            .iter()
            .map(|(calls, rows)| PublicationCell {
                calls: *calls,
                repair_rows: *rows,
                exact_ns: (e * *calls as f64 + r * *rows as f64) as u64,
            })
            .collect();
        let (fitted_e, fitted_r) = split_publication_cost(&cells).unwrap();
        assert!((fitted_e - e).abs() < 1.0, "E was {fitted_e}");
        assert!((fitted_r - r).abs() < 1.0, "R was {fitted_r}");
    }

    /// A negative price is a fit reading noise, not a cheaper term: the
    /// non-negativity constraint pins it at zero and re-solves.
    #[test]
    fn the_publication_split_never_returns_a_negative_price() {
        // `exact_ns` falls as rows rise, which an unconstrained fit answers
        // with a negative `R`.
        let cells = [
            PublicationCell {
                calls: 10,
                repair_rows: 0,
                exact_ns: 1_000_000,
            },
            PublicationCell {
                calls: 10,
                repair_rows: 50,
                exact_ns: 400_000,
            },
            PublicationCell {
                calls: 10,
                repair_rows: 100,
                exact_ns: 100_000,
            },
        ];
        let (e, r) = split_publication_cost(&cells).unwrap();
        assert!(e >= 0.0 && r >= 0.0, "E={e} R={r}");
        assert_eq!(r, 0.0);
    }

    /// Degenerate designs are refused rather than split arbitrarily.
    #[test]
    fn the_publication_split_refuses_a_degenerate_design() {
        let no_rows = [
            PublicationCell {
                calls: 4,
                repair_rows: 0,
                exact_ns: 1_000,
            },
            PublicationCell {
                calls: 9,
                repair_rows: 0,
                exact_ns: 2_250,
            },
        ];
        assert!(split_publication_cost(&no_rows)
            .unwrap_err()
            .contains("repair row"));

        // Rows always exactly twice the calls: the two columns are the same
        // column, and no split of the total between them is identifiable.
        let collinear = [
            PublicationCell {
                calls: 3,
                repair_rows: 6,
                exact_ns: 900,
            },
            PublicationCell {
                calls: 5,
                repair_rows: 10,
                exact_ns: 1_500,
            },
        ];
        assert!(split_publication_cost(&collinear)
            .unwrap_err()
            .contains("collinear"));

        assert!(split_publication_cost(&[]).is_err());
    }

    // -------------------------------------------------------- the harness --

    fn row(
        fixture: &str,
        iterations: u64,
        samples: u64,
        calls: u64,
        rows: u64,
        moves: u64,
    ) -> BiteProfileRow {
        // A synthetic bite priced at 100 ns per sample evaluation, 30 000 ns of
        // per-batch overhead, 250 000 ns per call, 4 000 ns per repair row and
        // 900 000 ns per disruption move.
        let sweep = samples * 100;
        let overhead = iterations * 30_000;
        let exact = calls * 250_000 + rows * 4_000;
        BiteProfileRow {
            fixture: fixture.to_owned(),
            iterations,
            barrier_to_barrier_ns: sweep + overhead + exact,
            prep_ns: overhead / 3,
            dispatch_ns: overhead / 3,
            sweep_critical_ns: sweep,
            // Eight workers' CPU, which is deliberately *not* what the base
            // unit is read from.
            sweep_total_ns: sweep * 8,
            merge_gls_ns: overhead - 2 * (overhead / 3),
            exact_ns: exact,
            band_fold_ns: 0,
            snapshot_ns: 0,
            residual_ns: 0,
            sample_evaluations: samples,
            exact_calls: calls,
            repair_rows: rows,
            disruption_moves: moves,
        }
    }

    /// The disruption nanoseconds the synthetic cell hides outside its
    /// barriers: 900 000 ns per move.
    fn synthetic_cell(fixture: &str) -> (Vec<BiteProfileRow>, CellWall) {
        let rows = vec![
            row(fixture, 40, 400_000, 3, 0, 0),
            row(fixture, 55, 600_000, 2, 17, 0),
            row(fixture, 90, 900_000, 5, 40, 6),
        ];
        let barriers: u64 = rows.iter().map(|row| row.barrier_to_barrier_ns).sum();
        let moves: u64 = rows.iter().map(|row| row.disruption_moves).sum();
        (
            rows,
            CellWall {
                fixture: fixture.to_owned(),
                search_ns: barriers + moves * 900_000,
            },
        )
    }

    /// The harness recovers the prices the synthetic cell was built from, and
    /// the calibration built on top of it is the conservative rounding of
    /// exactly those.
    fn three_synthetic_cells() -> (Vec<BiteProfileRow>, Vec<CellWall>) {
        let mut rows = Vec::new();
        let mut walls = Vec::new();
        for fixture in FIXTURES {
            let (cell, wall) = synthetic_cell(fixture);
            rows.extend(cell);
            walls.push(wall);
        }
        (rows, walls)
    }

    #[test]
    fn the_harness_recovers_the_prices_it_was_built_from() {
        let (rows, walls) = three_synthetic_cells();
        let report = timings_from_rows(&rows, &walls).unwrap();
        assert!(report.skipped.is_empty());
        let coefficients = calibrate(&report.timings, &FIXTURES)
            .unwrap()
            .currency
            .coefficients
            .unwrap();
        assert_eq!(coefficients.b_master_batch, 300);
        assert_eq!(coefficients.e_publication_call, 2_500);
        assert_eq!(coefficients.r_repair_row, 40);
        assert_eq!(coefficients.d_disruption_move, 9_000);
    }

    /// `D` is labelled as a bound in the record it produces, a cell without a
    /// search wall cannot produce it at all, and a cell whose separations
    /// account for the whole wall is refused rather than pricing the term
    /// free.
    #[test]
    fn the_disruption_price_is_named_as_a_bound_and_needs_a_wall() {
        let (rows, wall) = synthetic_cell("mixed-61");
        let walls = vec![wall.clone()];
        let timings = timings_from_rows(&rows, &walls).unwrap().timings;
        let disruption = timings
            .iter()
            .find(|row| row.term == Term::DisruptionMove)
            .unwrap();
        assert!(disruption.derivation.contains("UPPER BOUND"));
        assert_eq!(disruption.nanos / disruption.count, 900_000);

        assert!(timings_from_rows(&rows, &[]).unwrap_err().contains("wall"));

        let barriers: u64 = rows.iter().map(|row| row.barrier_to_barrier_ns).sum();
        let tight = vec![CellWall {
            fixture: "mixed-61".to_owned(),
            search_ns: barriers,
        }];
        let report = timings_from_rows(&rows, &tight).unwrap();
        assert!(report
            .timings
            .iter()
            .all(|row| row.term != Term::DisruptionMove));
        assert!(report
            .skipped
            .iter()
            .any(|row| row.term == Term::DisruptionMove && row.reason.contains("priced free")));
    }

    /// A cell built without `ics-profile` has no nanoseconds, and the harness
    /// says which flag is missing instead of dividing by zero.
    #[test]
    fn the_harness_refuses_a_cell_with_no_timings() {
        let rows = vec![BiteProfileRow {
            fixture: "mixed-61".to_owned(),
            iterations: 10,
            sample_evaluations: 1_000,
            ..BiteProfileRow::default()
        }];
        let walls = vec![CellWall {
            fixture: "mixed-61".to_owned(),
            search_ns: 1,
        }];
        assert!(timings_from_rows(&rows, &walls)
            .unwrap_err()
            .contains("ics-profile"));
    }

    // -------------------------------------------------- the reject check --

    fn cell(fixture: &str, terms: WorkTerms, seconds: f64) -> FixtureCell {
        FixtureCell {
            fixture: fixture.to_owned(),
            terms,
            seconds,
        }
    }

    /// A currency whose prices really are the machine's transfers: every
    /// leave-one-out prediction is exact and the check accepts.
    #[test]
    fn a_transferring_currency_is_accepted() {
        let currency = Currency::u1(coefficients(300, 2_500, 40, 9_000));
        // Every cell is priced at exactly 1e7 units per second.
        let cells: Vec<FixtureCell> = [
            ("mixed-61", terms(4_000_000, 500, 20, 60, 12)),
            ("shapes-17", terms(1_500_000, 200, 9, 5, 0)),
            ("triangle-20", terms(900_000, 90, 4, 0, 3)),
        ]
        .iter()
        .map(|(fixture, terms)| {
            let units = currency.units(terms);
            cell(fixture, *terms, units as f64 / 1e7)
        })
        .collect();
        let check = transfer_check(&currency, &cells).unwrap();
        assert!(check.accepted);
        assert!(check.worst_relative_error < 1e-9);
        assert_eq!(check.predictions.len(), 6);
        assert!(check.rejected_by.is_none());
    }

    /// **The spec's clause.** One fixture that really runs 25 % slower per unit
    /// than the currency says rejects the currency, and the record names the
    /// pair that did it.
    #[test]
    fn a_fixture_off_by_more_than_ten_percent_rejects_the_currency() {
        let currency = Currency::u1(coefficients(300, 2_500, 40, 9_000));
        let mixed = terms(4_000_000, 500, 20, 60, 12);
        let shapes = terms(1_500_000, 200, 9, 5, 0);
        let cells = vec![
            cell("mixed-61", mixed, currency.units(&mixed) as f64 / 1e7),
            // 25 % slower per unit than mixed-61.
            cell("shapes-17", shapes, currency.units(&shapes) as f64 / 0.8e7),
        ];
        let check = transfer_check(&currency, &cells).unwrap();
        assert!(!check.accepted);
        assert!(check.worst_relative_error > WALL_PREDICTION_TOLERANCE);
        assert!(check.rejected_by.is_some());
    }

    /// The threshold is `>10 %`, so 10.000 % passes and a hair more does not.
    ///
    /// The seconds are chosen so the worst of the two leave-one-out errors is
    /// **exactly** the double `0.1`: two cells of equal units at 10.0 s and
    /// 11.0 s make the harder direction `1.0 / 10.0`. Relative error is not
    /// symmetric - `|p - o| / o` depends on which cell is the observation -
    /// and the check reports the worse of the pair on purpose.
    #[test]
    fn the_tolerance_boundary_is_exactly_ten_percent() {
        let currency = Currency::U0;
        let base = terms(1_000_000, 0, 0, 0, 0);
        let check = transfer_check(
            &currency,
            &[cell("mixed-61", base, 10.0), cell("shapes-17", base, 11.0)],
        )
        .unwrap();
        assert_eq!(check.worst_relative_error, 0.1);
        assert!(check.accepted, "10.000 % is not >10 %");

        let check = transfer_check(
            &currency,
            &[cell("mixed-61", base, 10.0), cell("shapes-17", base, 11.5)],
        )
        .unwrap();
        assert_eq!(check.worst_relative_error, 0.15);
        assert!(!check.accepted);
    }

    /// One cell cannot transfer to anything, and a cell with no wall or no
    /// units has no rate. All three are refusals rather than a `true`.
    #[test]
    fn the_transfer_check_refuses_what_it_cannot_judge() {
        let currency = Currency::U0;
        let base = terms(1_000, 0, 0, 0, 0);
        assert!(transfer_check(&currency, &[cell("mixed-61", base, 1.0)]).is_err());
        assert!(transfer_check(
            &currency,
            &[cell("mixed-61", base, 1.0), cell("shapes-17", base, 0.0)]
        )
        .is_err());
        assert!(transfer_check(
            &currency,
            &[
                cell("mixed-61", base, 1.0),
                cell("shapes-17", terms(0, 0, 0, 0, 0), 1.0)
            ]
        )
        .is_err());
    }

    /// The whole pipeline, end to end: rows in, coefficients out, verdict out,
    /// and the coefficients round-trip through serde unchanged so the plan
    /// file carries the same numbers the check accepted.
    #[test]
    fn the_pipeline_round_trips_through_serde() {
        let (rows, walls) = three_synthetic_cells();
        let calibration = calibrate(
            &timings_from_rows(&rows, &walls).unwrap().timings,
            &FIXTURES,
        )
        .unwrap();
        let text = serde_json::to_string(&calibration).unwrap();
        let parsed: Calibration = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, calibration);
        assert!(text.contains("U1-weighted-vector"));
        assert!(text.contains("conservative-ceil"));
    }
}
