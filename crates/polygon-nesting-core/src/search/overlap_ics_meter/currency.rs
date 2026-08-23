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
    let rows: Vec<(u64, u64, u64)> = cells
        .iter()
        .map(|cell| (cell.calls, cell.repair_rows, cell.exact_ns))
        .collect();
    two_term_nnls(
        &rows,
        "no publication cell has a call or a repair row",
        "no repair row was ever observed; `R` cannot be priced from these cells",
        "no publication call was ever observed; `E` cannot be priced",
        "calls and repair rows are collinear across the cells; the split is not identifiable",
        "the publication split is not finite",
    )
}

/// **The same two-parameter non-negative least squares, over the residual the
/// separations did not claim.**
///
/// `U'` prices two different things out of one unaccounted region: the per-bite
/// work of a bite that published (`P` - the cut, the pose install, the
/// publication commit, the row rebuild) and the disruption (`D`). They share a
/// region for exactly the reason `E` and `R` do - nobody put a timer between
/// them - and they are separable for exactly the same reason: the campaign's
/// fixtures spend them in different proportions. triangle-20 publishes 34 bites
/// and disrupts nothing, shapes-17 disrupts and publishes nothing, mixed-61
/// spends both. That is a design matrix, not a coincidence, and it is printed
/// beside the fit so a reader can see the support each price has.
///
/// One row per **fixture**, not per bite: `outside_ns` is a cell-level residual
/// (the driver's search wall minus the sum of that cell's barriers), and there
/// is no per-bite version of it to fit. Three rows, two unknowns.
pub fn split_residual_cost(rows: &[(u64, u64, u64)]) -> Result<(f64, f64), String> {
    two_term_nnls(
        rows,
        "no cell published a bite or moved a piece",
        "no disruption move was ever observed; `D` cannot be priced from these cells",
        "no published bite was ever observed; `P` cannot be priced",
        "published bites and disruption moves are collinear across the cells; the split is \
         not identifiable",
        "the residual split is not finite",
    )
}

/// The arithmetic both splits share: `t ≈ a·x + b·y`, non-negative, solved
/// unconstrained and re-solved with one price pinned at zero if either comes
/// out negative. The messages are the callers', because the terms have names
/// and an error that named the wrong one would be worse than a duplicated
/// twenty lines.
fn two_term_nnls(
    rows: &[(u64, u64, u64)],
    empty: &str,
    no_y: &str,
    no_x: &str,
    collinear: &str,
    not_finite: &str,
) -> Result<(f64, f64), String> {
    let mut sum_xx = 0.0f64;
    let mut sum_xy = 0.0f64;
    let mut sum_yy = 0.0f64;
    let mut sum_xt = 0.0f64;
    let mut sum_yt = 0.0f64;
    let mut any = false;
    for (x, y, t) in rows {
        if *x == 0 && *y == 0 {
            continue;
        }
        any = true;
        let x = *x as f64;
        let y = *y as f64;
        let t = *t as f64;
        sum_xx += x * x;
        sum_xy += x * y;
        sum_yy += y * y;
        sum_xt += x * t;
        sum_yt += y * t;
    }
    if !any {
        return Err(empty.to_owned());
    }
    if sum_yy <= 0.0 {
        return Err(no_y.to_owned());
    }
    if sum_xx <= 0.0 {
        return Err(no_x.to_owned());
    }
    let determinant = sum_xx * sum_yy - sum_xy * sum_xy;
    if !determinant.is_finite() || determinant.abs() <= 1e-9 * sum_xx * sum_yy {
        return Err(collinear.to_owned());
    }
    let a = (sum_yy * sum_xt - sum_xy * sum_yt) / determinant;
    let b = (sum_xx * sum_yt - sum_xy * sum_xt) / determinant;
    let (a, b) = if a < 0.0 {
        (0.0, sum_yt / sum_yy)
    } else if b < 0.0 {
        (sum_xt / sum_xx, 0.0)
    } else {
        (a, b)
    };
    if !a.is_finite() || !b.is_finite() {
        return Err(not_finite.to_owned());
    }
    Ok((a, b))
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
    /// **`U'`'s per-bite term, and the one counter on this row that is not a
    /// `profile` field.** `1` when this bite published and `0` when it did not,
    /// read from the trajectory record's own `published` flag - the same field
    /// every committed cell document has carried in every build since the
    /// campaign began, at no clock cost and inside the two-process bit
    /// comparison. Additive: a document written before `U'` deserialises with
    /// `0`, and a currency that prices a term at zero occurrences refuses
    /// rather than prices it free.
    #[serde(default)]
    pub published_bites: u64,
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

// ============================================================================
//  `U'` - the amended currency (docs/currency-amendment.md, three signatures)
// ============================================================================
//
// > `U' = sample_evaluations + B·master_batches + E·exact_checkpoint_calls
// > + P·published_bites + D·disruption_moves`
// > - `R` is DROPPED absolutely … restorable only in a FUTURE funding under a
// >   pre-written rule.
// > - Same derivation (timing-only, three fixtures, conservative rounding),
// >   same >10% reject rule verbatim, still a stop.
//
// Everything below is **additive**. `WorkTerms`, `Currency`, `Coefficients`,
// `calibrate` and `transfer_check` above are the signed `U`, and the amendment
// changes the currency rather than editing it: the rejected `U` has to stay
// exactly the thing that was rejected, or the three committed runs that
// rejected it stop being reproducible. `CurrencyVersion` in
// `search/overlap_ics/icscal.rs` is **not** extended either - a `U'` plan is
// never written, because a currency is not writable until it passes the rule
// that this section exists to apply to it.
//
// # The three changes, and what each one costs
//
// * **`R` is absent.** `E` is therefore a *direct* reading - `exactNs` over
//   `exactCheckpointCalls` - on every fixture that reached exact geometry, and
//   no least-squares split runs inside the timed publication region at all.
//   The 16 µm repair's wall does not disappear: it is inside `exactNs`, so it
//   is now charged to `E`. That is what dropping a term means and it is said
//   here rather than discovered later.
// * **`E` is named for what it counts.** The audit's F4 split gave the record
//   `exactCheckpointCalls`; `PhaseProfile::exact_calls` is that counter, and
//   `U`'s `actual_publication_attempt_calls` was the same number under an
//   older name.
// * **`P` is new.** It prices the per-bite work the meter named when `U` was
//   rejected - cut, pose install, publication commit, row rebuild - out of the
//   one region no timer covers: the search wall the barriers did not claim.
//   `D` comes out of the same region, which is why the two are split rather
//   than each taking the whole residual as `U`'s `D` did.

/// The five counted terms of `U'`. `repair_rows` is **gone**, not zeroed: a
/// currency that carried a term priced at zero would be pricing the repair
/// free, and the amendment drops the term rather than its price.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkTermsPrime {
    /// The base unit. Implicit coefficient 1, never calibrated.
    pub sample_evaluations: u64,
    /// Master batches, i.e. eight-worker tournaments. `B`.
    pub master_batches: u64,
    /// The audit's F4 counter: times the exact authorities were actually
    /// asked, not band entries. `E`.
    pub exact_checkpoint_calls: u64,
    /// Bites that published. `P`. **Rider (i)**: an instrumented, deterministic
    /// counter, proven bit-identical across two processes before this
    /// coefficient is fitted.
    pub published_bites: u64,
    /// Pieces an Algorithm-12 disruption moved, followers included. `D`.
    pub disruption_moves: u64,
}

impl WorkTermsPrime {
    pub fn add(&mut self, other: &Self) {
        self.sample_evaluations = self
            .sample_evaluations
            .saturating_add(other.sample_evaluations);
        self.master_batches = self.master_batches.saturating_add(other.master_batches);
        self.exact_checkpoint_calls = self
            .exact_checkpoint_calls
            .saturating_add(other.exact_checkpoint_calls);
        self.published_bites = self.published_bites.saturating_add(other.published_bites);
        self.disruption_moves = self.disruption_moves.saturating_add(other.disruption_moves);
    }
}

/// One of `U'`'s terms, as a timing addresses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TermPrime {
    SampleEvaluation,
    MasterBatch,
    ExactCheckpointCall,
    PublishedBite,
    DisruptionMove,
    /// `E` and `P` fitted as **one** term, when rider (ii) fires.
    CombinedCheckpointAndBite,
}

impl TermPrime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SampleEvaluation => "sample-evaluation",
            Self::MasterBatch => "master-batch",
            Self::ExactCheckpointCall => "exact-checkpoint-call",
            Self::PublishedBite => "published-bite",
            Self::DisruptionMove => "disruption-move",
            Self::CombinedCheckpointAndBite => "combined-checkpoint-and-bite",
        }
    }
}

/// `B`, `E`, `P`, `D`, and the unrounded prices they were ceil'd from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoefficientsPrime {
    pub b_master_batch: u64,
    pub e_exact_checkpoint_call: u64,
    pub p_published_bite: u64,
    pub d_disruption_move: u64,
    pub measured: MeasuredPricesPrime,
    pub rounding: Rounding,
    /// **Rider (ii).** `true` when the `E` and `P` design vectors were collinear
    /// within rounding and one combined price was fitted and written into both
    /// coefficients. `false` when they were separable and two were.
    pub combined_e_and_p: bool,
}

/// The unrounded exchange rates of `U'`, in sample-evaluation equivalents.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasuredPricesPrime {
    pub b_master_batch: f64,
    pub e_exact_checkpoint_call: f64,
    pub p_published_bite: f64,
    pub d_disruption_move: f64,
    pub base_ns_per_sample_evaluation: f64,
}

/// `U'`. Version string is a constant of this module and deliberately **not** a
/// [`CurrencyVersion`]: that enum keys `icscal` files, and no `icscal` file may
/// be written in a currency that has not passed the reject rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyPrime {
    pub version: String,
    pub coefficients: Option<CoefficientsPrime>,
}

/// The name `U'` is reported under, in every document.
pub const U_PRIME_VERSION: &str = "U2-per-bite-vector";

impl CurrencyPrime {
    pub fn new(coefficients: CoefficientsPrime) -> Self {
        Self {
            version: U_PRIME_VERSION.to_owned(),
            coefficients: Some(coefficients),
        }
    }

    /// **`U'`.** Saturating, for the same reason [`Currency::units`] is.
    pub fn units(&self, terms: &WorkTermsPrime) -> u64 {
        let Some(coefficients) = self.coefficients.as_ref() else {
            return terms.sample_evaluations;
        };
        let mut units = terms.sample_evaluations;
        for (count, price) in [
            (terms.master_batches, coefficients.b_master_batch),
            (
                terms.exact_checkpoint_calls,
                coefficients.e_exact_checkpoint_call,
            ),
            (terms.published_bites, coefficients.p_published_bite),
            (terms.disruption_moves, coefficients.d_disruption_move),
        ] {
            units = units.saturating_add(count.saturating_mul(price));
        }
        units
    }

    pub fn summary(&self) -> String {
        match self.coefficients.as_ref() {
            None => format!("{} U'=sampleEvaluations", self.version),
            Some(c) => format!(
                "{} B={} E={} P={} D={}{}",
                self.version,
                c.b_master_batch,
                c.e_exact_checkpoint_call,
                c.p_published_bite,
                c.d_disruption_move,
                if c.combined_e_and_p {
                    " (E,P fitted as ONE combined term: rider (ii))"
                } else {
                    ""
                }
            ),
        }
    }
}

/// **Rider (ii), as a computation rather than a judgement.**
///
/// > since the corrected `E`-counter and bites plausibly co-move (triangle-20
/// > reads 34/34), the two vectors must be reported side by side, and if they
/// > are proportional within rounding, fit **one** term.
///
/// The criterion is written here, once, and it is pre-committed in
/// `gate2/README.md` before any number was measured: the vectors are
/// **collinear** iff the per-fixture ratio `E_f / P_f`, over the fixtures where
/// both are non-zero, has `max/min <= 1.05` **and** the cosine of the angle
/// between the whole vectors is `>= 0.9995`. Two measures rather than one,
/// because either alone has a shape it cannot see: a cosine is insensitive to a
/// fixture with a tiny norm, and a ratio spread says nothing about a fixture
/// where one of the two is zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollinearityReport {
    pub fixtures: Vec<String>,
    /// `E`'s design vector, fixture by fixture, in `fixtures` order.
    pub exact_checkpoint_calls: Vec<u64>,
    /// `P`'s design vector, fixture by fixture, in the same order.
    pub published_bites: Vec<u64>,
    /// `E_f / P_f` wherever both are non-zero, with the fixture named.
    pub ratios: Vec<(String, f64)>,
    pub ratio_max_over_min: Option<f64>,
    pub cosine: f64,
    pub ratio_bar: f64,
    pub cosine_bar: f64,
    /// The verdict. `true` fits **one** combined term.
    pub collinear: bool,
}

/// The pre-committed bars of [`CollinearityReport`]. Constants, so the document
/// and the decision cannot disagree.
pub const COLLINEARITY_RATIO_BAR: f64 = 1.05;
pub const COLLINEARITY_COSINE_BAR: f64 = 0.9995;

pub fn collinearity(cells: &[FixtureCellPrime]) -> CollinearityReport {
    let fixtures: Vec<String> = cells.iter().map(|row| row.fixture.clone()).collect();
    let e: Vec<u64> = cells
        .iter()
        .map(|row| row.terms.exact_checkpoint_calls)
        .collect();
    let p: Vec<u64> = cells.iter().map(|row| row.terms.published_bites).collect();
    let mut ratios = Vec::new();
    for (index, name) in fixtures.iter().enumerate() {
        if e[index] > 0 && p[index] > 0 {
            ratios.push((name.clone(), e[index] as f64 / p[index] as f64));
        }
    }
    let spread = if ratios.is_empty() {
        None
    } else {
        let values: Vec<f64> = ratios.iter().map(|row| row.1).collect();
        let low = values.iter().copied().fold(f64::INFINITY, f64::min);
        let high = values.iter().copied().fold(0.0f64, f64::max);
        if low > 0.0 {
            Some(high / low)
        } else {
            None
        }
    };
    let dot: f64 = e
        .iter()
        .zip(&p)
        .map(|(a, b)| *a as f64 * *b as f64)
        .sum::<f64>();
    let norm_e = e.iter().map(|a| (*a as f64).powi(2)).sum::<f64>().sqrt();
    let norm_p = p.iter().map(|a| (*a as f64).powi(2)).sum::<f64>().sqrt();
    let cosine = if norm_e > 0.0 && norm_p > 0.0 {
        dot / (norm_e * norm_p)
    } else {
        0.0
    };
    let collinear = spread.map(|s| s <= COLLINEARITY_RATIO_BAR).unwrap_or(false)
        && cosine >= COLLINEARITY_COSINE_BAR;
    CollinearityReport {
        fixtures,
        exact_checkpoint_calls: e,
        published_bites: p,
        ratios,
        ratio_max_over_min: spread,
        cosine,
        ratio_bar: COLLINEARITY_RATIO_BAR,
        cosine_bar: COLLINEARITY_COSINE_BAR,
        collinear,
    }
}

/// One fixture's whole `U'` cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureCellPrime {
    pub fixture: String,
    pub terms: WorkTermsPrime,
    pub seconds: f64,
}

/// One `U'` term's pooled price, with the spread the pooling hides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermPricePrime {
    pub term: TermPrime,
    pub fixtures: Vec<String>,
    pub pooled_nanos_each: f64,
    pub cheapest_nanos_each: f64,
    pub dearest_nanos_each: f64,
    pub sample_evaluation_equivalents: f64,
    pub rounded: u64,
    pub derivation: String,
}

/// The residual split's own design matrix, printed so the support of `P` and
/// `D` is a table rather than a claim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualSplitRow {
    pub fixture: String,
    pub published_bites: u64,
    pub disruption_moves: u64,
    /// The driver's search wall minus that cell's summed barrier-to-barrier.
    pub outside_ns: u64,
    /// What the fitted prices say this fixture's residual should have been.
    pub fitted_ns: f64,
    /// `outside_ns - fitted_ns`. A large one is where the fit is lying.
    pub residual_ns: f64,
    /// `outside_ns / published_bites` where the fixture disrupted nothing, and
    /// `outside_ns / disruption_moves` where it published nothing: the direct
    /// single-term readings the two-term fit is checked against.
    pub direct_single_term_ns: Option<f64>,
}

/// What [`calibrate_prime`] produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationPrime {
    pub currency: CurrencyPrime,
    pub fixtures: Vec<String>,
    pub selected: Vec<TermPricePrime>,
    pub collinearity: CollinearityReport,
    pub residual_split: Vec<ResidualSplitRow>,
    /// Terms a fixture could not price, and why. A skip is not a zero.
    pub skipped: Vec<SkippedTiming>,
    /// The amendment's own sentence about what dropping `R` costs.
    pub notes: Vec<String>,
}

/// One fixture's summed profile row, as [`calibrate_prime`] needs it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureTimingInput {
    pub fixture: String,
    pub sweep_critical_ns: u64,
    pub batch_overhead_ns: u64,
    pub exact_ns: u64,
    pub barrier_to_barrier_ns: u64,
    pub search_ns: u64,
    pub sample_evaluations: u64,
    pub iterations: u64,
    pub exact_checkpoint_calls: u64,
    pub published_bites: u64,
    pub disruption_moves: u64,
}

impl FixtureTimingInput {
    /// The wall the barriers did not claim: cut, pose install, publication
    /// commit, row rebuild, pool restore and the disruption itself.
    pub fn outside_ns(&self) -> u64 {
        self.search_ns.saturating_sub(self.barrier_to_barrier_ns)
    }
}

/// **`U'`'s derivation, in one function - the same shape as [`calibrate`].**
///
/// * base unit: pooled `sweepCriticalNs` over pooled `sampleEvaluations`;
/// * `B`: pooled batch overhead over pooled iterations;
/// * `E`: pooled `exactNs` over pooled `exactCheckpointCalls`, **direct**,
///   because with `R` absent nothing else is charged to that region;
/// * `P` and `D`: the two-term non-negative least squares of the unaccounted
///   residual on `(published_bites, disruption_moves)`, one row per fixture;
/// * quotients over the base, rounded **up** ([`Rounding::ConservativeCeil`]).
///
/// Rider (ii) is applied before the two prices are written: if `E` and `P` are
/// collinear within rounding, **one** price is fitted from the two regions
/// pooled and written into both coefficients, and `combined_e_and_p` says so.
///
/// Refuses rather than guesses, exactly as [`calibrate`] does: a term with no
/// occurrence anywhere is an `Err`, not a zero.
pub fn calibrate_prime(
    inputs: &[FixtureTimingInput],
    required_fixtures: &[&str],
) -> Result<CalibrationPrime, String> {
    let mut fixtures: Vec<String> = inputs.iter().map(|row| row.fixture.clone()).collect();
    fixtures.sort();
    fixtures.dedup();
    for required in required_fixtures {
        if !fixtures.iter().any(|seen| seen == required) {
            return Err(format!(
                "the amendment calibrates on all three fixtures; `{required}` has no timing"
            ));
        }
    }
    let mut skipped: Vec<SkippedTiming> = Vec::new();
    let total = |pick: fn(&FixtureTimingInput) -> u64| -> u64 {
        inputs.iter().fold(0u64, |acc, row| acc + pick(row))
    };

    // ---- the base unit, and the two direct prices ----
    let base_count = total(|row| row.sample_evaluations);
    let base_nanos = total(|row| row.sweep_critical_ns);
    if base_count == 0 || base_nanos == 0 {
        return Err("no sample-evaluation timing: `U'` has no base unit".to_owned());
    }

    let per_fixture = |pick_ns: fn(&FixtureTimingInput) -> u64,
                       pick_count: fn(&FixtureTimingInput) -> u64|
     -> (f64, f64, f64, Vec<String>) {
        let mut cheapest = f64::INFINITY;
        let mut dearest = 0.0f64;
        let mut names = Vec::new();
        let mut nanos = 0u128;
        let mut count = 0u128;
        for row in inputs {
            if pick_count(row) == 0 {
                continue;
            }
            let each = pick_ns(row) as f64 / pick_count(row) as f64;
            cheapest = cheapest.min(each);
            dearest = dearest.max(each);
            names.push(row.fixture.clone());
            nanos += pick_ns(row) as u128;
            count += pick_count(row) as u128;
        }
        names.sort();
        let pooled = if count == 0 {
            f64::NAN
        } else {
            nanos as f64 / count as f64
        };
        (pooled, cheapest, dearest, names)
    };

    let (base_pooled, base_cheapest, base_dearest, base_fixtures) = per_fixture(
        |row| row.sweep_critical_ns,
        |row| row.sample_evaluations,
    );
    // **One base, used for both the report and the arithmetic.** An earlier
    // draft computed the divisor a second way (totals over totals, including
    // any fixture that swept without evaluating) and printed this one. The two
    // agree on every cell the campaign has, which is exactly why the
    // duplication was worth removing rather than worth keeping: a divisor that
    // is not the number beside it is a document that cannot be checked by hand.
    let base_ns = base_pooled;
    if !base_ns.is_finite() || base_ns <= 0.0 {
        return Err(format!(
            "the base unit is {base_ns} ns, which prices nothing"
        ));
    }
    let mut selected = vec![TermPricePrime {
        term: TermPrime::SampleEvaluation,
        fixtures: base_fixtures,
        pooled_nanos_each: base_pooled,
        cheapest_nanos_each: base_cheapest,
        dearest_nanos_each: base_dearest,
        sample_evaluation_equivalents: 1.0,
        rounded: 1,
        derivation: "direct: sweepCriticalNs (the sweeps' wall, never sweepTotalNs) over the \
                     all-workers sampleEvaluations"
            .to_owned(),
    }];

    let (b_ns, b_cheap, b_dear, b_fixtures) =
        per_fixture(|row| row.batch_overhead_ns, |row| row.iterations);
    if !b_ns.is_finite() || b_ns <= 0.0 {
        return Err("`B` has no timing; `U'` may not price a term nobody measured".to_owned());
    }
    selected.push(TermPricePrime {
        term: TermPrime::MasterBatch,
        fixtures: b_fixtures,
        pooled_nanos_each: b_ns,
        cheapest_nanos_each: b_cheap,
        dearest_nanos_each: b_dear,
        sample_evaluation_equivalents: b_ns / base_ns,
        rounded: Rounding::ConservativeCeil.apply(b_ns / base_ns),
        derivation: "direct: prep + dispatch + mergeGls + bandFold + snapshot + residual, over \
                     master iterations"
            .to_owned(),
    });

    let (e_ns, e_cheap, e_dear, e_fixtures) =
        per_fixture(|row| row.exact_ns, |row| row.exact_checkpoint_calls);
    for row in inputs {
        if row.exact_checkpoint_calls == 0 {
            skipped.push(SkippedTiming {
                // `SkippedTiming` is `U`'s type and its enum has no
                // `ExactCheckpointCall` variant; `PublicationAttemptCall` is
                // the same counter under the older name, and the reason string
                // says which so the document is not read as a claim about a
                // term `U'` does not have.
                fixture: row.fixture.clone(),
                term: Term::PublicationAttemptCall,
                reason: "E (exact_checkpoint_calls): the cell never reached exact geometry, \
                         so there is no call to price"
                    .to_owned(),
            });
        }
    }
    if !e_ns.is_finite() || e_ns <= 0.0 {
        return Err(
            "`E` has no timing on any fixture; `U'` may not price a term nobody measured"
                .to_owned(),
        );
    }

    // ---- `P` and `D`, out of the one region no timer covers ----
    let rows: Vec<(u64, u64, u64)> = inputs
        .iter()
        .map(|row| (row.published_bites, row.disruption_moves, row.outside_ns()))
        .collect();
    let (p_ns, d_ns) = split_residual_cost(&rows)?;
    let residual_split: Vec<ResidualSplitRow> = inputs
        .iter()
        .map(|row| {
            let fitted = p_ns * row.published_bites as f64 + d_ns * row.disruption_moves as f64;
            let direct = if row.disruption_moves == 0 && row.published_bites > 0 {
                Some(row.outside_ns() as f64 / row.published_bites as f64)
            } else if row.published_bites == 0 && row.disruption_moves > 0 {
                Some(row.outside_ns() as f64 / row.disruption_moves as f64)
            } else {
                None
            };
            ResidualSplitRow {
                fixture: row.fixture.clone(),
                published_bites: row.published_bites,
                disruption_moves: row.disruption_moves,
                outside_ns: row.outside_ns(),
                fitted_ns: fitted,
                residual_ns: row.outside_ns() as f64 - fitted,
                direct_single_term_ns: direct,
            }
        })
        .collect();
    if p_ns <= 0.0 {
        return Err(
            "`P` priced at or below zero by the residual split; `U'` will not carry a term it \
             cannot see"
                .to_owned(),
        );
    }
    if d_ns <= 0.0 {
        return Err(
            "`D` priced at or below zero by the residual split; `U'` will not carry a term it \
             cannot see"
                .to_owned(),
        );
    }

    // ---- rider (ii), decided before the prices are written ----
    let cells: Vec<FixtureCellPrime> = inputs
        .iter()
        .map(|row| FixtureCellPrime {
            fixture: row.fixture.clone(),
            terms: WorkTermsPrime {
                sample_evaluations: row.sample_evaluations,
                master_batches: row.iterations,
                exact_checkpoint_calls: row.exact_checkpoint_calls,
                published_bites: row.published_bites,
                disruption_moves: row.disruption_moves,
            },
            seconds: row.search_ns as f64 / 1e9,
        })
        .collect();
    let collinear = collinearity(&cells);

    let mut notes = vec![
        "R is ABSENT (docs/currency-amendment.md). The 16 um repair's wall is inside exactNs \
         and is therefore charged to E; dropping a term does not delete its cost."
            .to_owned(),
        "P and D are split out of the search wall the barriers did not claim - the only region \
         that contains the cut, the pose install, the publication commit, the row rebuild and \
         the disruption. No timer exists around any of them, so this is a two-term fit and not \
         two direct readings; `residualSplit` prints the design matrix and the per-fixture \
         miss."
            .to_owned(),
    ];

    let (e_equivalents, p_equivalents) = if collinear.collinear {
        // One region, one price: the two timed regions pooled over the two
        // counts pooled. Written into both coefficients so `units` need not
        // know which branch ran.
        let nanos = total(|row| row.exact_ns) as f64
            + inputs
                .iter()
                .map(|row| p_ns * row.published_bites as f64)
                .sum::<f64>();
        let count = total(|row| row.exact_checkpoint_calls) + total(|row| row.published_bites);
        if count == 0 {
            return Err("the combined E,P term has no occurrences".to_owned());
        }
        let combined = nanos / count as f64 / base_ns;
        notes.push(format!(
            "RIDER (ii) FIRED: the E and P design vectors are collinear within rounding \
             (ratio spread {:?}, cosine {:.6}), so ONE combined price of {:.3} \
             sample-evaluation equivalents was fitted and written into both coefficients.",
            collinear.ratio_max_over_min, collinear.cosine, combined
        ));
        selected.push(TermPricePrime {
            term: TermPrime::CombinedCheckpointAndBite,
            fixtures: fixtures.clone(),
            pooled_nanos_each: combined * base_ns,
            cheapest_nanos_each: combined * base_ns,
            dearest_nanos_each: combined * base_ns,
            sample_evaluation_equivalents: combined,
            rounded: Rounding::ConservativeCeil.apply(combined),
            derivation: "rider (ii): exactNs + the residual split's published-bite share, over \
                         calls + published bites, as ONE term"
                .to_owned(),
        });
        (combined, combined)
    } else {
        notes.push(format!(
            "RIDER (ii) DID NOT FIRE: the E and P design vectors are separable (ratio spread \
             {:?} against a {} bar, cosine {:.6} against a {} bar), so two prices were fitted.",
            collinear.ratio_max_over_min,
            COLLINEARITY_RATIO_BAR,
            collinear.cosine,
            COLLINEARITY_COSINE_BAR
        ));
        let e_equivalents = e_ns / base_ns;
        let p_equivalents = p_ns / base_ns;
        selected.push(TermPricePrime {
            term: TermPrime::ExactCheckpointCall,
            fixtures: e_fixtures.clone(),
            pooled_nanos_each: e_ns,
            cheapest_nanos_each: e_cheap,
            dearest_nanos_each: e_dear,
            sample_evaluation_equivalents: e_equivalents,
            rounded: Rounding::ConservativeCeil.apply(e_equivalents),
            derivation: "direct: exactNs over exactCheckpointCalls. With R absent this region \
                         has one term in it, so no split runs and the repair's wall is charged \
                         to E."
                .to_owned(),
        });
        let p_direct: Vec<f64> = residual_split
            .iter()
            .filter(|row| row.published_bites > 0)
            .filter_map(|row| row.direct_single_term_ns)
            .collect();
        selected.push(TermPricePrime {
            term: TermPrime::PublishedBite,
            fixtures: residual_split
                .iter()
                .filter(|row| row.published_bites > 0)
                .map(|row| row.fixture.clone())
                .collect(),
            pooled_nanos_each: p_ns,
            cheapest_nanos_each: p_direct.iter().copied().fold(p_ns, f64::min),
            dearest_nanos_each: p_direct.iter().copied().fold(p_ns, f64::max),
            sample_evaluation_equivalents: p_equivalents,
            rounded: Rounding::ConservativeCeil.apply(p_equivalents),
            derivation: "fitted: non-negative least squares of the unclaimed search wall on \
                         (publishedBites, disruptionMoves), one row per fixture"
                .to_owned(),
        });
        (e_equivalents, p_equivalents)
    };

    let d_equivalents = d_ns / base_ns;
    let d_direct: Vec<f64> = residual_split
        .iter()
        .filter(|row| row.disruption_moves > 0)
        .filter_map(|row| row.direct_single_term_ns)
        .collect();
    selected.push(TermPricePrime {
        term: TermPrime::DisruptionMove,
        fixtures: residual_split
            .iter()
            .filter(|row| row.disruption_moves > 0)
            .map(|row| row.fixture.clone())
            .collect(),
        pooled_nanos_each: d_ns,
        cheapest_nanos_each: d_direct.iter().copied().fold(d_ns, f64::min),
        dearest_nanos_each: d_direct.iter().copied().fold(d_ns, f64::max),
        sample_evaluation_equivalents: d_equivalents,
        rounded: Rounding::ConservativeCeil.apply(d_equivalents),
        derivation: "fitted: the same non-negative least squares. U priced D as the WHOLE \
                     unclaimed residual and called it an upper bound; U' takes the per-bite \
                     work out of it first, so this D is smaller and tighter."
            .to_owned(),
    });

    let coefficients = CoefficientsPrime {
        b_master_batch: Rounding::ConservativeCeil.apply(b_ns / base_ns),
        e_exact_checkpoint_call: Rounding::ConservativeCeil.apply(e_equivalents),
        p_published_bite: Rounding::ConservativeCeil.apply(p_equivalents),
        d_disruption_move: Rounding::ConservativeCeil.apply(d_equivalents),
        measured: MeasuredPricesPrime {
            b_master_batch: b_ns / base_ns,
            e_exact_checkpoint_call: e_equivalents,
            p_published_bite: p_equivalents,
            d_disruption_move: d_equivalents,
            base_ns_per_sample_evaluation: base_ns,
        },
        rounding: Rounding::ConservativeCeil,
        combined_e_and_p: collinear.collinear,
    };
    Ok(CalibrationPrime {
        currency: CurrencyPrime::new(coefficients),
        fixtures,
        selected,
        collinearity: collinear,
        residual_split,
        skipped,
        notes,
    })
}

/// The `U'` transfer check. **The reject rule is the same sentence, verbatim**,
/// against the same [`WALL_PREDICTION_TOLERANCE`]: leave one fixture out, price
/// a rate on it, predict every other fixture's wall, and reject on any pair
/// over 10 %.
pub fn transfer_check_prime(
    currency: &CurrencyPrime,
    cells: &[FixtureCellPrime],
) -> Result<TransferCheck, String> {
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
        // `TransferCheck` is the signed `U`'s type and carries a `Currency`.
        // `U'` reports its own currency beside the check rather than inside it,
        // so that the frozen document shape of the rejected `U` does not move.
        currency: Currency::U0,
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
            // `U`'s harness never reads this field; it is `U'`'s term, and a
            // synthetic `U` row leaves it at the value a pre-`U'` document
            // deserialises to.
            published_bites: 0,
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

    // ------------------------------------------------------------- `U'` --
    //
    // The amended currency's own vectors. `ics_meter` prints what these
    // functions return, so a driver that agreed with a copy of the rule would
    // still have to disagree with these.

    #[allow(clippy::too_many_arguments)]
    fn prime_input(
        fixture: &str,
        sweep: u64,
        overhead: u64,
        exact: u64,
        barriers: u64,
        search: u64,
        samples: u64,
        iterations: u64,
        calls: u64,
        published: u64,
        moves: u64,
    ) -> FixtureTimingInput {
        FixtureTimingInput {
            fixture: fixture.to_owned(),
            sweep_critical_ns: sweep,
            batch_overhead_ns: overhead,
            exact_ns: exact,
            barrier_to_barrier_ns: barriers,
            search_ns: search,
            sample_evaluations: samples,
            iterations,
            exact_checkpoint_calls: calls,
            published_bites: published,
            disruption_moves: moves,
        }
    }

    /// `U'` prices the four terms over the base, and the ceil is the only
    /// rounding. One sample evaluation is 100 ns by construction here, so
    /// every coefficient below is readable by hand.
    #[test]
    fn u_prime_prices_every_term_over_the_base() {
        // base: 1_000_000 ns of sweep over 10_000 evaluations = 100 ns each.
        // B:       50_000 ns of overhead over 100 iterations  =   500 ns ->   5
        // E:       30_000 ns of exact over 10 calls           = 3_000 ns ->  30
        let inputs = [
            // `alpha` spends both residual terms and can separate neither
            // alone: 60 000 ns of residual is 4 bites and 2 moves at the same
            // 10 000 ns each that the other two fixtures read directly.
            prime_input(
                "alpha", 1_000_000, 50_000, 30_000, 800_000, 860_000, 10_000, 100, 10, 4, 2,
            ),
            // `beta` publishes nothing and disrupts: it prices `D` alone.
            prime_input(
                "beta", 1_000_000, 50_000, 0, 500_000, 560_000, 10_000, 100, 0, 0, 6,
            ),
            // `gamma` disrupts nothing and publishes: it prices `P` alone. Its
            // 5 calls against alpha's 10, on the same 4 published bites, are
            // what keeps rider (ii) from firing on this vector.
            prime_input(
                "gamma", 1_000_000, 50_000, 15_000, 500_000, 540_000, 10_000, 100, 5, 4, 0,
            ),
        ];
        let calibration = calibrate_prime(&inputs, &["alpha", "beta", "gamma"]).unwrap();
        let coefficients = calibration.currency.coefficients.unwrap();
        assert_eq!(coefficients.b_master_batch, 5);
        assert_eq!(coefficients.e_exact_checkpoint_call, 30);
        // gamma: 40_000 ns over 4 published bites = 10_000 ns -> 100 equivalents
        assert_eq!(coefficients.p_published_bite, 100);
        // beta: 60_000 ns over 6 moves = 10_000 ns -> 100 equivalents
        assert_eq!(coefficients.d_disruption_move, 100);
        assert!(!coefficients.combined_e_and_p);
        assert_eq!(coefficients.rounding, Rounding::ConservativeCeil);
        // The design matrix is printed, not claimed.
        assert_eq!(calibration.residual_split.len(), 3);
        let alpha = &calibration.residual_split[0];
        assert_eq!(alpha.outside_ns, 60_000);
        assert_eq!(alpha.published_bites, 4);
        assert_eq!(alpha.disruption_moves, 2);
        assert!(alpha.direct_single_term_ns.is_none());
        // An exactly-consistent design leaves nothing over, and the document
        // says so rather than making the reader take the fit on trust.
        assert!(alpha.residual_ns.abs() < 1e-6, "{}", alpha.residual_ns);
    }

    /// **Rider (ii) end to end.** When the two vectors *are* proportional the
    /// calibration fits ONE price and writes it into both coefficients, so no
    /// document can carry two collinear prices with units on them.
    #[test]
    fn u_prime_fits_one_term_when_the_vectors_co_move() {
        let inputs = [
            prime_input(
                "alpha", 1_000_000, 50_000, 30_000, 800_000, 900_000, 10_000, 100, 10, 4, 2,
            ),
            prime_input(
                "beta", 1_000_000, 50_000, 0, 500_000, 560_000, 10_000, 100, 0, 0, 6,
            ),
            // 10 calls to 4 published bites on both: exactly proportional.
            prime_input(
                "gamma", 1_000_000, 50_000, 30_000, 500_000, 540_000, 10_000, 100, 10, 4, 0,
            ),
        ];
        let calibration = calibrate_prime(&inputs, &["alpha", "beta", "gamma"]).unwrap();
        assert!(calibration.collinearity.collinear, "{:?}", calibration.collinearity);
        let coefficients = calibration.currency.coefficients.unwrap();
        assert!(coefficients.combined_e_and_p);
        assert_eq!(
            coefficients.e_exact_checkpoint_call, coefficients.p_published_bite,
            "one price, written into both"
        );
        assert!(calibration
            .selected
            .iter()
            .any(|row| row.term == TermPrime::CombinedCheckpointAndBite));
        assert!(!calibration
            .selected
            .iter()
            .any(|row| row.term == TermPrime::ExactCheckpointCall));
        assert!(calibration
            .notes
            .iter()
            .any(|note| note.contains("RIDER (ii) FIRED")));
    }

    /// `units` is the amended formula and nothing else, and it is
    /// `sample_evaluations` when there are no coefficients.
    #[test]
    fn u_prime_units_are_the_amended_formula() {
        let terms = WorkTermsPrime {
            sample_evaluations: 1_000,
            master_batches: 7,
            exact_checkpoint_calls: 3,
            published_bites: 2,
            disruption_moves: 5,
        };
        let currency = CurrencyPrime::new(CoefficientsPrime {
            b_master_batch: 10,
            e_exact_checkpoint_call: 100,
            p_published_bite: 1_000,
            d_disruption_move: 10_000,
            measured: MeasuredPricesPrime {
                b_master_batch: 10.0,
                e_exact_checkpoint_call: 100.0,
                p_published_bite: 1_000.0,
                d_disruption_move: 10_000.0,
                base_ns_per_sample_evaluation: 1.0,
            },
            rounding: Rounding::ConservativeCeil,
            combined_e_and_p: false,
        });
        assert_eq!(
            currency.units(&terms),
            1_000 + 7 * 10 + 3 * 100 + 2 * 1_000 + 5 * 10_000
        );
        let bare = CurrencyPrime {
            version: U_PRIME_VERSION.to_owned(),
            coefficients: None,
        };
        assert_eq!(bare.units(&terms), 1_000);
    }

    /// **`R` is absent, and absent is not zero.** `WorkTermsPrime` has no
    /// repair field at all, so no arrangement of repair rows can change a `U'`
    /// reading - which is the whole of "R is DROPPED absolutely".
    #[test]
    fn u_prime_has_no_repair_term_to_price() {
        let text = serde_json::to_string(&WorkTermsPrime::default()).unwrap();
        assert!(!text.contains("repair"), "{text}");
    }

    /// **Rider (ii)**, both branches, at the pre-committed bars.
    #[test]
    fn rider_two_fires_only_on_proportional_vectors() {
        let cell = |fixture: &str, calls: u64, published: u64| FixtureCellPrime {
            fixture: fixture.to_owned(),
            terms: WorkTermsPrime {
                sample_evaluations: 1,
                master_batches: 0,
                exact_checkpoint_calls: calls,
                published_bites: published,
                disruption_moves: 0,
            },
            seconds: 1.0,
        };
        // Exactly proportional: 2x on both fixtures.
        let proportional = [cell("a", 50, 25), cell("b", 34, 17)];
        let report = collinearity(&proportional);
        assert!(report.collinear, "{report:?}");
        assert!((report.cosine - 1.0).abs() < 1e-12);
        assert!((report.ratio_max_over_min.unwrap() - 1.0).abs() < 1e-12);
        // The campaign's own shape: 50/24 against 34/34.
        let campaign = [cell("mixed-61", 50, 24), cell("triangle-20", 34, 34)];
        let report = collinearity(&campaign);
        assert!(!report.collinear, "{report:?}");
        assert!(report.ratio_max_over_min.unwrap() > COLLINEARITY_RATIO_BAR);
        assert!(report.cosine < COLLINEARITY_COSINE_BAR);
        assert_eq!(report.exact_checkpoint_calls, vec![50, 34]);
        assert_eq!(report.published_bites, vec![24, 34]);
    }

    /// The residual split separates the two terms when the fixtures spend them
    /// in different proportions, and refuses when they do not.
    #[test]
    fn the_residual_split_is_identifiable_or_it_refuses() {
        // 1_000 ns per published bite, 10_000 ns per move.
        let rows = [(4u64, 2u64, 24_000u64), (0, 6, 60_000), (4, 0, 4_000)];
        let (p, d) = split_residual_cost(&rows).unwrap();
        assert!((p - 1_000.0).abs() < 1e-6, "{p}");
        assert!((d - 10_000.0).abs() < 1e-6, "{d}");
        // Perfectly co-moving counts: not identifiable, and it says so.
        let collinear = [(2u64, 4u64, 10_000u64), (4, 8, 20_000), (6, 12, 30_000)];
        assert!(split_residual_cost(&collinear)
            .unwrap_err()
            .contains("collinear"));
    }

    /// A negative price is a fit reading noise, and the non-negativity
    /// constraint pins it rather than shipping `P = -3`.
    #[test]
    fn the_residual_split_never_ships_a_negative_price() {
        let rows = [(1u64, 1u64, 100u64), (0, 1, 10_000), (1, 0, 5)];
        let (p, d) = split_residual_cost(&rows).unwrap();
        assert!(p >= 0.0 && d >= 0.0, "p={p} d={d}");
    }

    /// **The reject rule is the same sentence.** A currency that transfers
    /// inside 10 % is accepted; one that does not is rejected and names the
    /// pair, and the `U'` check reads the same tolerance constant as `U`'s.
    #[test]
    fn u_prime_reject_rule_is_the_same_ten_percent() {
        let currency = CurrencyPrime {
            version: U_PRIME_VERSION.to_owned(),
            coefficients: None,
        };
        let cell = |fixture: &str, samples: u64, seconds: f64| FixtureCellPrime {
            fixture: fixture.to_owned(),
            terms: WorkTermsPrime {
                sample_evaluations: samples,
                ..WorkTermsPrime::default()
            },
            seconds,
        };
        // Same rate on both: transfers exactly.
        let check =
            transfer_check_prime(&currency, &[cell("a", 1_000, 1.0), cell("b", 2_000, 2.0)])
                .unwrap();
        assert!(check.accepted, "{check:?}");
        assert_eq!(check.tolerance, WALL_PREDICTION_TOLERANCE);
        // 20 % off: rejected, and the pair is named.
        let check =
            transfer_check_prime(&currency, &[cell("a", 1_000, 1.0), cell("b", 2_400, 2.0)])
                .unwrap();
        assert!(!check.accepted);
        assert!(check.rejected_by.is_some());
        assert!(check.worst_relative_error > 0.10);
    }

    /// A term nobody counted is a refusal, not a free price - and "on all
    /// three fixtures" is a refusal too.
    #[test]
    fn u_prime_refuses_a_term_with_no_occurrence() {
        let none_published = [
            prime_input(
                "alpha", 1_000_000, 50_000, 30_000, 800_000, 900_000, 10_000, 100, 10, 0, 2,
            ),
            prime_input(
                "beta", 1_000_000, 50_000, 0, 500_000, 560_000, 10_000, 100, 0, 0, 6,
            ),
            prime_input(
                "gamma", 1_000_000, 50_000, 30_000, 500_000, 540_000, 10_000, 100, 10, 0, 3,
            ),
        ];
        assert!(calibrate_prime(&none_published, &["alpha", "beta", "gamma"]).is_err());
        let two = [
            prime_input(
                "alpha", 1_000_000, 50_000, 30_000, 800_000, 900_000, 10_000, 100, 10, 4, 2,
            ),
            prime_input(
                "beta", 1_000_000, 50_000, 0, 500_000, 560_000, 10_000, 100, 0, 0, 6,
            ),
        ];
        assert!(calibrate_prime(&two, &["alpha", "beta", "gamma"])
            .unwrap_err()
            .contains("gamma"));
    }
}
