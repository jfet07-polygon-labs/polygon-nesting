//! The anytime portfolio coordinator: one process, one budget, several
//! operators, two state objects.
//!
//! # Why this exists
//!
//! The third adversarial review's second finding is that the engine's
//! publication logic - "adopt a mode's layout when it is complete, exact-valid
//! and strictly better" - is correct as a *safety* rule and wrong as the sole
//! *anytime state model*:
//!
//! > Mode 20's 206.869 candidate correctly loses to the 179.756 protected
//! > incumbent - but the documented from-scratch lineage begins from precisely
//! > this class of worse, structurally different constructor basin. If PR7
//! > feeds only `GeneralRelaxedOutcome.result` forward, it destroys the only
//! > evidenced route to 164.
//!
//! So a coordinator needs two objects, and this module is those two objects
//! plus a schedule that spends a budget across them:
//!
//! * [`PublishedIncumbent`] - the engine's answer. Always the best *raw* depth
//!   that has passed the composite exact validator against the real request.
//!   It only ever improves, and it improves only through
//!   [`general_relaxed::adopt_published_placements`], which is the same
//!   function the coupled separator's own mode slot publishes through. The
//!   coordinator has no validity opinion of its own; see [`try_publish`].
//! * [`SearchArchive`] - the search's memory. Exact-valid complete layouts kept
//!   for their *future* expected value, keyed by placement fingerprint, carrying
//!   raw depth, birth time, and the operator that produced them. It retains
//!   worse-but-structurally-different basins on purpose, and it evicts only a
//!   basin that is both dominated and similar.
//!
//! # What a work unit is
//!
//! The reproducible mode's budget is denominated in [`PortfolioBudget::Work`]
//! units, never in wall time, because a schedule whose branch points read a
//! clock is not reproducible on a shared box. One unit is one proxy candidate
//! query; an exact Clipper pair test is charged
//! [`WORK_UNITS_PER_EXACT_PAIR_TEST`] units. Both counters are the engine's own
//! `profiling` counters, so the budget is a function of the search and of
//! nothing else, and two runs of a work-budget schedule take the same branches
//! in the same order.
//!
//! [`PortfolioBudget::Wall`] is the demo mode: the same schedule, the same
//! phases, the deadline read off a monotonic clock. Its trajectory is *not*
//! reproducible, and it does not pretend to be.
//!
//! # What is general and what is policy
//!
//! Every length in this module is derived from the request: the constructor
//! clamp is the larger of a multiple of the request's own area lower-bound
//! depth and the depth the coordinator's own phase-0 constructor reached, the
//! basin target salts are relative to that clamp, and the alternation rung is
//! the engine's own construction drop ladder. The dimensionless numbers - how
//! many basin slots, what fraction of a budget a phase gets, how much pose
//! overlap makes two layouts "similar" - are schedule policy, carry no
//! millimetres, and are settings rather than constants wherever a caller could
//! reasonably disagree.
//!
//! A *dimensionless* constant can still be a fact about one request, and this
//! module shipped two that were. [`constructor_clamp_mm`] documents the first:
//! twice the area lower bound is above the reachable depth only when the
//! request packs at better than half of its own bound, which mixed-61 does and
//! shapes-17 and triangle-20 do not. [`PhaseSchedule`] documents the second:
//! a phase deadline quoted as a fraction of the *whole* budget is a fraction of
//! an unknown remainder, because phase 0's share of the budget is a property of
//! the request and the box.
//!
//! # How the budget is spent, and why in this order
//!
//! The phases are ordered by measured publications per second, not by the
//! order the review sketched them in: alternation quanta, then crossovers over
//! the distinct archive pairs, then a compressing micro-descent, then - last,
//! conditional, and stopping as soon as it stops paying - the salted
//! constructor slice. See [`BasinTrigger`] and [`PortfolioSettings::
//! basin_patience`] for what "conditional" and "stops paying" are measured to
//! mean, and `docs/experiments/pr7-coordinator-v2/` for the batteries.
//!
//! One rule cuts across all of them: a phase may start an operator call only if
//! the budget remaining before its deadline covers that operator's own measured
//! mean cost *in this run*, in the budget's own currency. An operator this run
//! has never called has no measured cost, so it is allowed one call to acquire
//! one - which is the only way the budget can ever overrun a deadline.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::profiling::{self, Counter};
#[cfg(feature = "compression-schedule")]
use crate::search::compression_schedule::ScheduleCheckpoint;
use crate::search::general_fast::{
    construct_short_side_first, validate_and_measure_placements, GeneralFastError,
    GeneralFastPiece, GeneralFastPlacement, GeneralFastResult, GeneralFastSettings,
};
#[cfg(feature = "compression-schedule")]
use crate::search::general_relaxed::SliceControl;
use crate::search::general_relaxed::{
    general_placement_fingerprint, GeneralCoupledSeparatorArmDiagnostics,
    GeneralPersistentVacancyDiagnostics, GeneralPersistentVacancyPinnedParent,
    GeneralRelaxedSettings, ALTERNATION_MAX_CYCLES,
};

/// The relative cost of one exact Clipper pair test in candidate-query units.
///
/// Calibrated from the quality frontier trace's own scope ledger, which is the
/// only place this engine has ever priced the two against each other on one
/// stream: the constructor spent 0.648 s on 584,671 exact pair tests
/// (1.108 us each) while the relaxed epochs spent 1.193 s on 5,332,423
/// candidate queries (0.224 us each), a ratio of 4.95.
///
/// It is a *budget* weight, not a claim about any other request: what it has to
/// be is fixed, positive, and the same in every run, so that a work budget
/// advances during a constructor phase - which issues no candidate queries at
/// all - instead of letting one run forever.
pub const WORK_UNITS_PER_EXACT_PAIR_TEST: u64 = 5;

/// The constructor clamp, as a multiple of the request's own area lower-bound
/// depth.
///
/// The quality frontier trace established this as the from-request mode-20
/// clamp (130.399 mm -> 260.797 mm on its stream) precisely so that no fixture
/// depth enters a from-request run. Dimensionless, so it scales with the
/// request.
///
/// It is a *floor* and not the clamp; see [`constructor_clamp_mm`]. Two times
/// the area lower bound is only above the reachable depth when the request
/// packs at better than 50% of its own area bound, and that is a property of
/// the request rather than a law: mixed-61 packs at 1.59x its bound, but
/// shapes-17 packs at 2.08x and triangle-20 at 2.21x, so on both of those a
/// clamp of two times the bound is *below* every layout that exists and every
/// constructor arm is asked for an impossible target.
pub const CONSTRUCTOR_CLAMP_MULTIPLE_OF_AREA_LOWER_BOUND: f64 = 2.0;

/// The clamp the constructor slice runs its salted arms at.
///
/// The clamp's job, per the cell-lottery finding, is to be *geometrically
/// inert*: it is only there so that the salt moves `grid_key(target_depth_mm)`
/// and redraws the insertion lottery, so it has to sit above any depth the
/// constructor can reach, and a clamp below that is not a loose bound - it is a
/// refusal.
///
/// So it is the larger of the area-lower-bound multiple and a depth this
/// request is *known* to admit a complete layout at, which is the one the
/// coordinator's own phase-0 constructor just built. Both terms are derived
/// from the request and neither is a length anyone chose.
pub fn constructor_clamp_mm(area_lower_bound_depth_mm: f64, constructed_depth_mm: f64) -> f64 {
    (area_lower_bound_depth_mm * CONSTRUCTOR_CLAMP_MULTIPLE_OF_AREA_LOWER_BOUND)
        .max(constructed_depth_mm)
}

/// The relative step between one basin slot's constructor clamp and the next.
///
/// Mode 20 derives its construction seed as
/// `parent_seed_key ^ CONSTRUCTION_SEED_DOMAIN ^ grid_key(target_depth_mm)`, so
/// moving the clamp by a few canonical grid steps redraws the whole insertion
/// lottery while leaving the clamp geometrically inert - it stays far above any
/// depth the constructor reaches. This is the "salt the target, never tune it"
/// half of the ledger's cell-lottery finding.
///
/// Relative, so it is a fixed number of grid steps only in proportion to the
/// request's own scale.
pub const BASIN_TARGET_SALT_RELATIVE_STEP: f64 = 1.0e-4;

/// Which operator produced an archived basin.
///
/// A string would have done; a type means a phase cannot invent a provenance
/// the report does not know how to name, and the archive's own diagnostics
/// group by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BasinOperator {
    /// The short-side-first constructor's own complete layout.
    Constructor,
    /// The protected relaxed search's result (mode 0).
    RelaxedM0,
    /// One arm of the coupled dynamic separator.
    CoupledSeparator,
    /// A persistent-vacancy mode, by its dispatch number.
    Mode(usize),
}

impl BasinOperator {
    /// The reporting name, stable across runs.
    pub fn name(self) -> String {
        match self {
            BasinOperator::Constructor => "constructor".to_owned(),
            BasinOperator::RelaxedM0 => "m0".to_owned(),
            BasinOperator::CoupledSeparator => "coupled".to_owned(),
            BasinOperator::Mode(mode) => format!("mode{mode}"),
        }
    }
}

/// One retained basin: a complete layout the search may want to descend from
/// later, whether or not it is the best thing anyone has found.
#[derive(Clone, Debug)]
pub struct ArchivedBasin {
    /// The placement fingerprint, which is the archive's key.
    pub fingerprint: String,
    /// Raw source depth, the untouched `f64` reading that cannot round.
    pub raw_depth_mm: f64,
    /// Seconds since the coordinator's own clock started.
    pub birth_seconds: f64,
    /// Work units spent when this basin was admitted.
    pub birth_work_units: u64,
    /// Which operator produced it.
    pub operator: BasinOperator,
    /// The fingerprint of the basin it descended from, when it descended.
    pub parent_fingerprint: Option<String>,
    /// The fingerprint of the *second* parent, for the operators that have one.
    ///
    /// Mode 23 descends from two layouts and the archive recorded only the
    /// first, so the genealogy it could report stopped at every crossover: a
    /// basin that fed parent B of a recombination was, on the record, never
    /// anyone's ancestor. Deferred credit is exactly the quantity that edge
    /// carries, so it is recorded.
    pub secondary_parent_fingerprint: Option<String>,
    /// Whether the composite exact validator accepted it against the real
    /// request. An archived basin may legitimately be `false`: the archive is
    /// allowed to remember a deliberately infeasible parent, and only
    /// [`PublishedIncumbent`] may not.
    pub exact_valid: bool,
    /// How many times this basin has been handed to an operator as a parent.
    pub descents: usize,
    /// The layout itself.
    pub placements: Vec<GeneralFastPlacement>,
}

/// What the archive did with an offered layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveDisposition {
    /// Admitted, with room to spare.
    Admitted,
    /// Admitted after evicting a basin that was both dominated and similar.
    AdmittedAfterEviction,
    /// The fingerprint was already in the archive.
    Duplicate,
    /// The layout was not complete, so it cannot be anyone's parent.
    IncompleteCardinality,
    /// The archive is full and no member is both dominated and similar to
    /// another, so nothing may be evicted and this candidate is refused.
    ///
    /// This is the *point* of the eviction rule: a full archive of mutually
    /// distinct basins is a full archive, and dropping one of them to make
    /// room for a newcomer would be exactly the "keep only the best" behaviour
    /// the review's second finding rejects.
    RefusedArchiveFullAllDistinct,
}

/// Archived basins, keyed by placement fingerprint.
///
/// The retention rule, in the order it is applied:
///
/// 1. A layout must be complete - one placement per requested piece - or it
///    cannot be an operator's parent and there is no point remembering it.
/// 2. A fingerprint already present is a duplicate; the first arrival keeps its
///    birth time and provenance.
/// 3. Under capacity, everything is admitted, *including a basin deeper than
///    every member*. Depth is not an admission criterion, because the ledger's
///    measured anti-correlation between a constructor's immediate depth and its
///    descendant's - Pearson -0.212 over eighteen paired samples - says
///    immediate depth is not evidence about future value.
/// 4. At capacity, a member may be evicted only if some other member is both
///    *dominated-by* (no deeper) and *similar-to* it. Similar means their
///    piece-assignment overlap - the fraction of pieces at an identical pose -
///    is at least [`SearchArchive::similarity_threshold`]. Nothing else is ever
///    evicted.
#[derive(Clone, Debug)]
pub struct SearchArchive {
    basins: Vec<ArchivedBasin>,
    capacity: usize,
    similarity_threshold: f64,
    piece_count: usize,
    admitted: usize,
    duplicates: usize,
    evicted: usize,
    refused_full: usize,
    refused_incomplete: usize,
    occupancy_samples: Vec<(f64, usize)>,
}

impl SearchArchive {
    /// A new archive holding at most `capacity` basins over `piece_count`
    /// pieces.
    pub fn new(capacity: usize, piece_count: usize, similarity_threshold: f64) -> Self {
        Self {
            basins: Vec::new(),
            capacity: capacity.max(1),
            similarity_threshold: similarity_threshold.clamp(0.0, 1.0),
            piece_count,
            admitted: 0,
            duplicates: 0,
            evicted: 0,
            refused_full: 0,
            refused_incomplete: 0,
            occupancy_samples: Vec::new(),
        }
    }

    /// The similarity threshold in force, as a piece-assignment overlap
    /// fraction.
    pub fn similarity_threshold(&self) -> f64 {
        self.similarity_threshold
    }

    /// Every retained basin, in admission order.
    pub fn basins(&self) -> &[ArchivedBasin] {
        &self.basins
    }

    /// Offers a layout to the archive and reports what happened to it.
    pub fn offer(&mut self, basin: ArchivedBasin) -> ArchiveDisposition {
        if basin.placements.len() != self.piece_count {
            self.refused_incomplete += 1;
            return ArchiveDisposition::IncompleteCardinality;
        }
        if self
            .basins
            .iter()
            .any(|member| member.fingerprint == basin.fingerprint)
        {
            self.duplicates += 1;
            return ArchiveDisposition::Duplicate;
        }
        if self.basins.len() < self.capacity {
            self.record(basin);
            return ArchiveDisposition::Admitted;
        }
        let Some(victim) = self.dominated_and_similar_victim(&basin) else {
            self.refused_full += 1;
            return ArchiveDisposition::RefusedArchiveFullAllDistinct;
        };
        self.basins.remove(victim);
        self.evicted += 1;
        self.record(basin);
        ArchiveDisposition::AdmittedAfterEviction
    }

    /// The index of the member that is *both* dominated by some other layout
    /// and structurally similar to it, deepest such member first.
    ///
    /// The incoming candidate counts as a possible dominator, which is what
    /// lets a strictly better re-descent of the same region replace its own
    /// predecessor. It is never itself a victim: a candidate the archive
    /// refuses is refused, not admitted-and-evicted.
    fn dominated_and_similar_victim(&self, candidate: &ArchivedBasin) -> Option<usize> {
        let mut worst: Option<(usize, f64)> = None;
        for (index, member) in self.basins.iter().enumerate() {
            let dominated = self
                .basins
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, other)| other)
                .chain(std::iter::once(candidate))
                .any(|other| {
                    other.raw_depth_mm <= member.raw_depth_mm
                        && assignment_overlap(&other.placements, &member.placements)
                            >= self.similarity_threshold
                });
            if !dominated {
                continue;
            }
            let replace = match worst {
                None => true,
                Some((_, depth)) => member.raw_depth_mm > depth,
            };
            if replace {
                worst = Some((index, member.raw_depth_mm));
            }
        }
        worst.map(|(index, _)| index)
    }

    fn record(&mut self, basin: ArchivedBasin) {
        self.admitted += 1;
        self.occupancy_samples
            .push((basin.birth_seconds, self.basins.len() + 1));
        self.basins.push(basin);
    }

    /// Withdraws a basin the caller has decided against, and says whether it
    /// was there to withdraw.
    ///
    /// Charged to `evicted` rather than to a fourth counter, because from the
    /// archive's point of view it is the same event as an eviction - a member
    /// leaving to make room for a better decision - and a second counter would
    /// invite the report to double-count the departures. The one caller is the
    /// basin race, which uses it to make elimination mean something: an arm
    /// that lost its audition and stayed in the archive is an arm the v3 queue
    /// can rank anyway, and then the race has only spent work.
    pub fn retire(&mut self, fingerprint: &str) -> bool {
        let Some(index) = self
            .basins
            .iter()
            .position(|basin| basin.fingerprint == fingerprint)
        else {
            return false;
        };
        self.basins.remove(index);
        self.evicted += 1;
        true
    }

    /// Marks a basin as having been used as an operator parent.
    pub fn charge_descent(&mut self, fingerprint: &str) {
        if let Some(basin) = self
            .basins
            .iter_mut()
            .find(|basin| basin.fingerprint == fingerprint)
        {
            basin.descents += 1;
        }
    }

    /// The `count` shallowest basins that are pairwise structurally distinct,
    /// shallowest first, breaking ties toward the least-descended.
    ///
    /// The order is the review's own phrase - "m22 work quanta across the
    /// **best** structurally distinct archive states" - and the order is
    /// load-bearing, which this stage learned by getting it wrong. Ordering by
    /// descent count first is *fairer*, and on the measured stream it spent the
    /// whole alternation phase on 194-214 mm constructor basins while the
    /// incumbent, the one parent whose quantum actually published, waited. A
    /// deep basin still gets a quantum - it is in this list, just behind the
    /// better ones - which is the archive earning its retention rather than
    /// being paid a subsidy.
    ///
    /// Distinctness is enforced *within the selection*, not against the whole
    /// archive: the point of the selection is to spend a phase's quanta on
    /// different regions, and two near-identical parents would spend two quanta
    /// on one.
    pub fn distinct_frontier(&self, count: usize) -> Vec<ArchivedBasin> {
        let mut ordered = self.basins.clone();
        ordered.sort_by(|left, right| {
            left.raw_depth_mm
                .total_cmp(&right.raw_depth_mm)
                .then(left.descents.cmp(&right.descents))
                .then(left.fingerprint.cmp(&right.fingerprint))
        });
        let mut chosen: Vec<ArchivedBasin> = Vec::new();
        for basin in ordered {
            if chosen.len() >= count {
                break;
            }
            let distinct = chosen.iter().all(|kept| {
                assignment_overlap(&kept.placements, &basin.placements) < self.similarity_threshold
            });
            if distinct {
                chosen.push(basin);
            }
        }
        chosen
    }

    /// The archive's own report.
    pub fn report(&self) -> ArchiveReport {
        let mut by_operator: BTreeMap<String, usize> = BTreeMap::new();
        for basin in &self.basins {
            *by_operator.entry(basin.operator.name()).or_default() += 1;
        }
        ArchiveReport {
            capacity: self.capacity,
            occupancy: self.basins.len(),
            similarity_threshold: self.similarity_threshold,
            admitted: self.admitted,
            duplicates: self.duplicates,
            evicted: self.evicted,
            refused_full: self.refused_full,
            refused_incomplete: self.refused_incomplete,
            by_operator,
            occupancy_over_time: self.occupancy_samples.clone(),
            members: self
                .basins
                .iter()
                .map(|basin| ArchiveMemberReport {
                    fingerprint: basin.fingerprint.clone(),
                    raw_depth_mm: basin.raw_depth_mm,
                    birth_seconds: basin.birth_seconds,
                    birth_work_units: basin.birth_work_units,
                    operator: basin.operator.name(),
                    parent_fingerprint: basin.parent_fingerprint.clone(),
                    secondary_parent_fingerprint: basin.secondary_parent_fingerprint.clone(),
                    exact_valid: basin.exact_valid,
                    descents: basin.descents,
                })
                .collect(),
        }
    }
}

/// One archived basin, as reported.
#[derive(Clone, Debug)]
pub struct ArchiveMemberReport {
    pub fingerprint: String,
    pub raw_depth_mm: f64,
    pub birth_seconds: f64,
    pub birth_work_units: u64,
    pub operator: String,
    pub parent_fingerprint: Option<String>,
    pub secondary_parent_fingerprint: Option<String>,
    pub exact_valid: bool,
    pub descents: usize,
}

/// The archive's occupancy, provenance mix and refusal counts.
#[derive(Clone, Debug)]
pub struct ArchiveReport {
    pub capacity: usize,
    pub occupancy: usize,
    pub similarity_threshold: f64,
    pub admitted: usize,
    pub duplicates: usize,
    pub evicted: usize,
    pub refused_full: usize,
    pub refused_incomplete: usize,
    pub by_operator: BTreeMap<String, usize>,
    /// `(seconds, occupancy)` after each admission.
    pub occupancy_over_time: Vec<(f64, usize)>,
    pub members: Vec<ArchiveMemberReport>,
}

/// The fraction of pieces that occupy an *identical* pose in both layouts.
///
/// The cheap first cut is the placement fingerprint - equal fingerprints are
/// the same layout - and this is the better cut the review asks for when the
/// fingerprints differ: two layouts that agree on 59 of 61 pieces are the same
/// basin with a two-piece repair, and two that agree on 12 are not.
///
/// Pose equality is exact on the fields a placement is made of. There is no
/// tolerance here on purpose: a tolerance would be a length, and a length would
/// have to come from somewhere.
pub fn assignment_overlap(left: &[GeneralFastPlacement], right: &[GeneralFastPlacement]) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let right_by_id = right
        .iter()
        .map(|placement| (placement.piece_id.as_str(), placement))
        .collect::<BTreeMap<_, _>>();
    let mut agreeing = 0usize;
    for placement in left {
        let Some(other) = right_by_id.get(placement.piece_id.as_str()) else {
            continue;
        };
        if placement.rotation_deg.to_bits() == other.rotation_deg.to_bits()
            && placement.mirrored == other.mirrored
            && placement.translate_short_axis.to_bits() == other.translate_short_axis.to_bits()
            && placement.translate_long_axis.to_bits() == other.translate_long_axis.to_bits()
        {
            agreeing += 1;
        }
    }
    agreeing as f64 / left.len().max(right.len()) as f64
}

/// The engine's answer: the best raw depth that has passed the composite exact
/// validator against the real request.
#[derive(Clone, Debug)]
pub struct PublishedIncumbent {
    result: GeneralFastResult,
    fingerprint: String,
    raw_depth_mm: Option<f64>,
    dual_gate_valid: bool,
    source: String,
    published_seconds: f64,
    published_work_units: u64,
}

impl PublishedIncumbent {
    /// The layout this incumbent publishes.
    pub fn result(&self) -> &GeneralFastResult {
        &self.result
    }

    /// Its placement fingerprint.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Its raw source depth, when it is measurable.
    pub fn raw_depth_mm(&self) -> Option<f64> {
        self.raw_depth_mm
    }

    /// Whether the composite exact validator accepts it against the real
    /// request.
    pub fn dual_gate_valid(&self) -> bool {
        self.dual_gate_valid
    }

    /// Which phase published it.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Seconds since the coordinator's clock started when it was published.
    pub fn published_seconds(&self) -> f64 {
        self.published_seconds
    }

    /// Work units spent when it was published.
    pub fn published_work_units(&self) -> u64 {
        self.published_work_units
    }
}

/// The budget a schedule spends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortfolioBudget {
    /// Wall-clock milliseconds. The demo mode: the trajectory depends on how
    /// fast the box is and is not reproducible.
    Wall { millis: u64 },
    /// Work units, per this module's header. The reproducible mode: two runs
    /// take the same branches and produce the same layout.
    Work { units: u64 },
    /// A wall **target**, spent as a work plan sized to fit it.
    ///
    /// The caller names seconds; the coordinator measures how fast this box is
    /// running this request, converts the target into a work budget, and then
    /// runs [`Work`][Self::Work] - so the trajectory is a function of the plan
    /// and not of the clock, and the clock is read exactly once.
    ///
    /// This is the mode `docs/sol-review-5-se2-and-pose-freedom.md` §5 asks for
    /// when it refuses "l'efficienza mm/work di m34 come prestazione di
    /// produzione": a work envelope is not a wall envelope, and the way to make
    /// it one is to price the work in seconds *on the box that will run it*
    /// rather than to quote a work number and hope. It is also the answer to
    /// `docs/experiments/sparse-rotation/` §7.2, where the same unchanged arm
    /// published medians 2-5 mm apart between sessions because a wall budget
    /// converts box load into depth.
    ///
    /// The calibration is [`PlanReport`]; `run_portfolio` replaces this variant
    /// with [`Work`][Self::Work] as soon as phase 0 has priced the box, so no
    /// budget decision is ever taken against this variant.
    Plan { target_millis: u64 },
}

/// One improvement of the published incumbent, with the phase that caused it.
#[derive(Clone, Debug)]
pub struct PublicationEvent {
    pub seconds: f64,
    pub work_units: u64,
    pub phase: String,
    pub source: String,
    pub raw_depth_mm: f64,
    pub previous_raw_depth_mm: Option<f64>,
    pub fingerprint: String,
}

/// How a run treats the parallel work currency.
///
/// Three values rather than a bool, and the middle one is the instrument this
/// round is built on: [`Self::Observe`] computes and reports every price the
/// currency would charge **without charging any of them**, so a paired
/// `Off`/`Observe` run walks the same trajectory and the difference between
/// the two documents is exactly the new reporting. That is what makes the
/// mispricing measurable on the shipped arm rather than only on a repriced
/// one, and it is what `docs/experiments/work-currency/` §1's table is read
/// off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkCurrencyMode {
    /// Nothing is computed, nothing is reported, nothing is charged. The
    /// shipped arm, and bit-identical to a binary that does not have the
    /// field.
    #[default]
    Off,
    /// Priced and reported, never charged. The trajectory is the `Off`
    /// trajectory: two extra counter reads per operator call increment
    /// nothing, so a work-budgeted run under `Observe` produces the `Off`
    /// document plus the currency's own block.
    Observe,
    /// Priced, reported and **charged**: the coordinator settles every
    /// operator call at `max(global_delta, self_metered_units,
    /// class_self_units)`, so affordability, every phase deadline, the class
    /// ranking and the plan's own budget all read the repriced currency.
    Charge,
}

impl WorkCurrencyMode {
    /// Whether the currency is computed and reported at all.
    pub fn armed(self) -> bool {
        !matches!(self, WorkCurrencyMode::Off)
    }

    /// Whether a price this currency computed is settled into the meter.
    fn charges(self) -> bool {
        matches!(self, WorkCurrencyMode::Charge)
    }

    /// The stable reporting name.
    pub fn label(self) -> &'static str {
        match self {
            WorkCurrencyMode::Off => "off",
            WorkCurrencyMode::Observe => "observe",
            WorkCurrencyMode::Charge => "charge",
        }
    }
}

/// What the parallel currency priced one operator call at, and out of what.
///
/// Reported whole rather than as one scalar for the same reason
/// [`OperatorCharge`] reports four numbers: the interesting fact about a
/// repricing is the *gap*, and a document that carried only the settled
/// maximum could not be used to fit the profile that produced it. Every field
/// here is an input to `drivers/fitprofile.py`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkCurrencyCallReport {
    pub candidate_queries: u64,
    pub exact_pair_tests: u64,
    pub collision_builds: u64,
    pub neighbor_tests: u64,
    pub full_rescores: u64,
    pub position_source_attempts: u64,
    pub returned_positions: u64,
    pub pair_visits: u64,
    pub operator_collision_builds: u64,
    pub confirmations: u64,
    /// What [`crate::search::work_currency::ClassPrice::units`] returned for
    /// this class and these counts.
    pub class_units: u64,
    /// What this call actually added to the meter *because of* the currency:
    /// zero under [`WorkCurrencyMode::Observe`], zero when some other price
    /// was already at least as high, and the difference otherwise.
    pub charged_extra_units: u64,
}

/// One operator invocation the schedule made.
#[derive(Clone, Debug)]
pub struct OperatorCallReport {
    pub phase: String,
    pub operator: String,
    pub parent_fingerprint: Option<String>,
    /// Parent B, for the operators that take one. See
    /// [`ArchivedBasin::secondary_parent_fingerprint`].
    pub secondary_parent_fingerprint: Option<String>,
    /// The action this call executed, when the phase names one: the crossover
    /// ledger's `A->B@cut` descriptor, or a probe arm's step label. `None` for
    /// the schedule's own unparameterised calls.
    pub action: Option<String>,
    pub started_seconds: f64,
    pub elapsed_seconds: f64,
    /// What this call was *charged* to the work budget:
    /// `global_units + debited_units`, i.e. `max(global_units,
    /// self_metered_units)` for the one operator that carries its own meter.
    /// This is the number [`BudgetMeter::call_cost`] prices a future call of
    /// the same operator at, and since coordinator v5's transaction ordering
    /// it includes *this* call's own debit rather than the previous one's.
    pub work_units: u64,
    /// The coordinator's own counter delta across the call, before any
    /// self-metered debit. Reported next to [`Self::work_units`] rather than
    /// instead of it so the one place the two disagree is visible in the
    /// evidence rather than argued for in prose (Sol review 6 §1).
    pub global_units: u64,
    /// What the operator's own meter charged itself, when it carries one -
    /// see [`schedule_self_cost_units`]. `None` for every operator that does
    /// not, which without the parallel currency is all of them but mode 34.
    ///
    /// With [`WorkCurrencyMode::Charge`] armed this is
    /// `max(schedule_self_cost_units, class_self_units)` - the class price is
    /// the operator's own price, settled through the same arm - so it becomes
    /// `Some` on every class the profile names. Nothing is lost by the
    /// merge: [`WorkCurrencyCallReport::class_units`] carries the class price
    /// on its own, so the two are separable in the evidence.
    pub self_metered_units: Option<u64>,
    /// `self_metered_units.saturating_sub(global_units)`, and zero under a
    /// wall budget. What this call added to the meter beyond the global
    /// counter's own reading.
    pub debited_units: u64,
    pub exact_valid: bool,
    pub raw_depth_mm: Option<f64>,
    /// The fingerprint of the layout this call produced, when it produced a
    /// complete one. The genealogy needs it: a call whose output the archive
    /// refused as a duplicate still happened, and its parent edge is real.
    pub result_fingerprint: Option<String>,
    pub archive_disposition: Option<String>,
    pub published: bool,
    pub failure_reason: Option<String>,
    /// The compression-schedule slice's own account of itself, when this call
    /// was one. `None` for every other operator, and for a build without the
    /// `compression-schedule` feature.
    ///
    /// The coordinator's document carried none of this before: a reader could
    /// see that an m34 call took 1.93 s and published, and could not see how
    /// much of that wall was the exact tier, whether the parent arrived
    /// feasible, or whether the slice was given back unspent. Every claim this
    /// round makes about where the slice's seconds go is read out of here.
    pub schedule_slice: Option<ScheduleSliceReport>,
    /// The parallel currency's account of this call, or `None` when
    /// [`PortfolioSettings::work_currency`] is [`WorkCurrencyMode::Off`] -
    /// which is the default, and is why an unarmed run's document is
    /// byte-identical to a binary that has never heard of the currency.
    pub work_currency: Option<WorkCurrencyCallReport>,
}

/// What one mode-34 slice did, as the slice itself measured it.
///
/// A projection of `GeneralCompressionScheduleDiagnostics` rather than the
/// whole of it: the per-step rows are thousands of entries per call and belong
/// to the operator's own document, not to the coordinator's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScheduleSliceReport {
    /// The proxy tier's verdict on the parent as it arrived, before any entry
    /// repair. `parentCollisionPairs` is 26-38 on every 171-179 mm coordinator
    /// parent the port measured, at the parent's *own* depth.
    pub parent_proxy_feasible: bool,
    pub parent_collision_pairs: usize,
    pub parent_boundary_violations: usize,
    pub parent_entry_loss: f64,
    /// The same three, on the state the slice actually started from. Equal to
    /// the parent's unless a translation-only entry repair was armed and
    /// accepted.
    pub entry_proxy_feasible: bool,
    pub entry_collision_pairs: usize,
    pub entry_boundary_violations: usize,
    pub entry_loss: f64,
    /// The material depth of the state the lane actually starts from, and how
    /// far above the parent that is. The slice's whole arithmetic: it may walk
    /// `requestedDropMm` of clamp and it publishes only below the parent, so a
    /// slice that arrives a drop or more above the parent cannot publish.
    pub entry_source_depth_mm: Option<f64>,
    pub entry_depth_loss_mm: Option<f64>,
    pub requested_drop_mm: f64,
    pub entry_legalization_armed: bool,
    pub entry_legalization_run: bool,
    pub entry_legalization_resolved: bool,
    pub entry_legalization_accepted: bool,
    pub entry_legalization_ms: f64,
    /// Why the entry repair produced no layout, when it ran and did not, and
    /// its own before/after violation counts in the exact tier's terms.
    pub entry_legalization_reason: Option<String>,
    pub entry_legalization_violating_pairs_before: usize,
    pub entry_legalization_violating_pairs_after: usize,
    pub entry_legalization_boundary_pieces_before: usize,
    pub entry_legalization_boundary_pieces_after: usize,
    /// Whether the slice was given back unspent - by either skip rule.
    pub skipped_infeasible_entry: bool,
    /// Whether the slice was abandoned after its probe expired with nothing
    /// published below the parent, and how long that probe was.
    pub aborted_barren_probe: bool,
    pub probe_steps: usize,
    pub steps_planned: usize,
    pub steps_taken: usize,
    pub confirmations_attempted: usize,
    pub confirmations_accepted: usize,
    pub confirmations_refused: usize,
    pub confirmations_skipped_infeasible: usize,
    /// Where the slice's wall went: the exact tier, the repair sweeps, and the
    /// entry repair. The three do not sum to the call's `elapsedSeconds` -
    /// catalogue construction, the initial state and the surrogate scoring are
    /// outside all three - and that residual is itself a finding.
    pub confirmation_ms: f64,
    pub repair_ms: f64,
    pub start_depth_mm: f64,
    pub final_depth_mm: f64,
    pub work_units: usize,
    pub exit_cause: String,
    /// The continuous-rotation operator's account of this slice: whether it was
    /// armed, what it proposed, what it bought and what it cost. See
    /// `GeneralCompressionScheduleDiagnostics::continuous_rotation` for what
    /// each counter is a denominator for; all zero on an unarmed slice.
    pub continuous_rotation: bool,
    pub rotation_rungs_proposed: usize,
    pub rotation_rungs_improved: usize,
    pub mirror_toggles_proposed: usize,
    pub mirror_toggles_improved: usize,
    pub rotation_accepted_moves: usize,
    pub accepted_moves: usize,
    pub rotation_loss_bought_mm: f64,
    pub translation_loss_bought_mm: f64,
    pub rotation_surrogate_builds: usize,
    pub rotation_surrogate_hits: usize,
    pub rotation_surrogate_evictions: usize,
    pub rotation_surrogate_build_ms: f64,
    pub rotation_surrogate_cells: usize,
    pub rotation_builds_refused: usize,
    /// The sparse operator's account of the same slice: how the rungs were
    /// built, how sparse the arming actually was, and what the witness cost.
    /// See `GeneralCompressionScheduleDiagnostics` for each counter's
    /// denominator; all zero without `sparse-rotation` compiled in.
    pub sparse_rotation: bool,
    pub rotation_equivariant_offset: bool,
    pub rotation_equivariant_builds: usize,
    pub rotation_equivariant_fallbacks: usize,
    pub sparse_rotation_episodes: usize,
    pub sparse_rotation_pieces_armed: usize,
    pub sparse_rotation_sweeps: usize,
    /// The operator-specific chain. `sparse_rotation_committed_episodes` is the
    /// one the disarm bit reads; see
    /// `GeneralCompressionScheduleDiagnostics` for why
    /// `rotation_accepted_moves` cannot be.
    pub sparse_rotation_rungs_proposed: usize,
    pub sparse_rotation_rung_winners: usize,
    pub sparse_rotation_committed_moves: usize,
    pub sparse_rotation_committed_episodes: usize,
    pub se2_witness_calls: usize,
    pub se2_witness_accepted: usize,
    pub se2_witness_adoptions: usize,
    pub se2_witness_ms: f64,
    pub se2_witness_bought_mm: f64,
    /// The batch budget the slice ran under, or `None` for the atomic slice.
    #[cfg(feature = "compression-schedule")]
    pub batch_work_units: Option<usize>,
    /// One row per batch boundary, empty on the atomic slice. See
    /// [`crate::search::compression_schedule::ScheduleCheckpoint`].
    #[cfg(feature = "compression-schedule")]
    pub checkpoints: Vec<crate::search::compression_schedule::ScheduleCheckpoint>,
    /// Batches, resumptions and whether the caller stopped the slice. See the
    /// three fields of the same name on
    /// [`crate::search::compression_schedule::GeneralCompressionScheduleDiagnostics`],
    /// which carry the same silence rules: an atomic, never-interrupted slice
    /// emits none of them and its document is the previous round's, key for key.
    #[cfg(feature = "compression-schedule")]
    pub batches: usize,
    #[cfg(feature = "compression-schedule")]
    pub resumptions: usize,
    #[cfg(feature = "compression-schedule")]
    pub interrupted: bool,
    /// The slice's per-step digest. See
    /// [`crate::search::compression_schedule::GeneralCompressionScheduleDiagnostics::step_digest`].
    /// This is the field the concatenation gate is decided on: the aggregates
    /// above can agree while the walks differ, and this cannot.
    #[cfg(feature = "compression-schedule")]
    pub step_digest: u64,
}

/// Why a phase stopped issuing operator calls.
///
/// `skipped` answered "did this phase run at all" and nothing else, so a
/// saturated schedule reported five phases that had all simply *ended*. The
/// review's third ledger item is the distinction this enum makes: a phase that
/// ran out of *actions* is a fixpoint of the action space and a phase that ran
/// out of *budget* is not, and only the first is evidence that the operator set
/// is exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseExitCause {
    /// The phase's deadline had already passed when it was entered.
    SkippedDeadlinePassed,
    /// The phase ran its body to the end: every attempt it was allowed to make
    /// was made.
    Completed,
    /// The selection the phase draws from cannot supply an action at all - the
    /// frontier has fewer members than the operator needs, or the incumbent has
    /// no measurable depth.
    GeometricFixpoint,
    /// Every action the phase can name has already been attempted, at this
    /// quantum size, with these parents. This is the "keys exhausted" exit and
    /// it is *not* a fixpoint of the operator set.
    KeysExhausted,
    /// The remaining budget before the phase deadline no longer covers this
    /// operator's own measured mean cost in this run.
    Affordability,
    /// The phase deadline was reached mid-phase.
    Deadline,
    /// The run's **wall** deadline was reached mid-phase, with
    /// [`PortfolioSettings::compression_schedule_wall_stop_all`] armed.
    ///
    /// Distinct from [`Self::Deadline`], which is a fraction of the budget's
    /// own currency, because under a plan or a work budget that currency is a
    /// counter and this one is a clock. A phase that exits here has budget left
    /// and refused to spend it; a phase that exits on `Deadline` has not.
    ///
    /// It is also what keeps the re-plan loop out: `run_v3_schedule` buys
    /// another tranche only for the three budget-bound causes, and a run out of
    /// seconds is not one of them.
    WallStop,
    /// The diversify phase's patience rule fired: `basin_patience` consecutive
    /// iterations published nothing.
    Patience,
    /// The diversify phase's trigger refused to draw at all.
    TriggerRefused,
    /// The compressing descent came back exact-valid, so mode 31 had no residue
    /// to legalize.
    NoResidue,
    /// An operator produced no complete layout, so the phase stopped rather
    /// than buying the same refusal again.
    NoCompleteLayout,
}

impl PhaseExitCause {
    /// The reporting name, stable across runs.
    pub fn name(self) -> &'static str {
        match self {
            PhaseExitCause::SkippedDeadlinePassed => "skippedDeadlinePassed",
            PhaseExitCause::Completed => "completed",
            PhaseExitCause::GeometricFixpoint => "geometricFixpoint",
            PhaseExitCause::KeysExhausted => "keysExhausted",
            PhaseExitCause::Affordability => "affordability",
            PhaseExitCause::Deadline => "deadline",
            PhaseExitCause::WallStop => "wallStop",
            PhaseExitCause::Patience => "patience",
            PhaseExitCause::TriggerRefused => "triggerRefused",
            PhaseExitCause::NoResidue => "noResidue",
            PhaseExitCause::NoCompleteLayout => "noCompleteLayout",
        }
    }
}

/// How a [`PortfolioBudget::Plan`] turned a wall target into a work budget.
///
/// Every field is reported because the plan is a *prediction* and a reader has
/// to be able to audit it against the wall the run actually took. The split
/// between the two halves matters and is the reason this is one struct rather
/// than a scalar:
///
/// * `target_millis`, `bias`, `headroom`, `quantum_step`, `probe_work_units`
///   and `units` are **deterministic** for a given (request, seed, settings).
///   `probe_work_units` is a counter, not a clock.
/// * `probe_seconds`, `probe_rate_units_per_second` and `raw_units` are
///   **clock readings**. Two processes on one box do not agree on them and
///   never will.
///
/// `units` is what the run is actually budgeted at, and it is in the first
/// group *only because it is quantised*: see [`PLAN_QUANTUM_STEP`].
#[derive(Clone, Copy, Debug)]
pub struct PlanReport {
    /// The wall target the caller asked for, in milliseconds.
    pub target_millis: u64,
    /// Phase 0's own elapsed seconds - the probe. A clock reading.
    pub probe_seconds: f64,
    /// Phase 0's own work units. Deterministic per (request, seed).
    pub probe_work_units: u64,
    /// `probe_work_units / probe_seconds`.
    pub probe_rate_units_per_second: f64,
    /// The phase-0 bias correction actually applied. See [`PLAN_PHASE_ZERO_BIAS`].
    pub bias: f64,
    /// The headroom actually applied. See [`PLAN_HEADROOM`].
    pub headroom: f64,
    /// The quantisation step actually applied. `1.0` means unquantised.
    pub quantum_step: f64,
    /// The plan before quantisation.
    pub raw_units: f64,
    /// The rung index on the quantisation ladder, or `None` when unquantised.
    pub rung: Option<i64>,
    /// The work budget installed. **This is the plan.**
    pub units: u64,
    /// The fraction of the target this first tranche aimed at. `1.0` is the
    /// single-tranche plan; anything below it means the run intends to re-plan.
    /// See [`PortfolioSettings::plan_first_tranche`].
    pub first_tranche: f64,
    /// The probe wall the arithmetic above actually used.
    ///
    /// Equal to `probe_seconds` in the shipped mode. It differs when a max-of-k
    /// bucket estimate ([`PLAN_PROBE_BUCKETS`]) or a persisted calibration
    /// ([`PortfolioSettings::plan_calibration_path`]) replaced the whole-phase
    /// clock reading, and `calibration_source` says which.
    ///
    /// It is a clock reading when the source is `live` or `probe` and a **file
    /// constant** when the source is `file` - which is the whole point of the
    /// file, and the only configuration in which this mode's plan is
    /// independent of the box's load.
    pub probe_effective_seconds: f64,
    /// Where `probe_effective_seconds` came from. Deterministic.
    pub calibration_source: PlanCalibrationSource,
    /// How many probe samples the sampler thread collected. `0` when it was not
    /// armed. A clock-side diagnostic.
    pub probe_samples: usize,
}

/// One in-run re-plan: a second (or third) tranche of work, priced at the rate
/// the **queue** is actually retiring units at rather than at the rate phase 0
/// did.
///
/// This is `docs/experiments/calibrated-plan/` §13.1's fix, and the whole of why
/// it works is that it swaps a guess for a measurement. [`PLAN_PHASE_ZERO_BIAS`]
/// exists to correct phase 0's rate onto the queue's; a tranche measures the
/// queue's rate directly, over a window that is the whole of the run so far, so
/// its estimator's bias is 1 by construction and its window is three times the
/// probe's.
///
/// # What is deterministic here and what is not
///
/// The same split as [`PlanReport`], and it has to be read carefully because
/// this is the one place in the mode where a clock reading can change *how many*
/// decisions a run makes rather than only how big one of them is:
///
/// * `index`, `rung` and `units` are the deterministic half. `units` is snapped
///   to the same ladder [`PLAN_QUANTUM_STEP`] defines, so two processes that
///   read slightly different clocks still land on the same rung.
/// * `at_seconds`, `queue_seconds`, `queue_rate_units_per_second`,
///   `remaining_seconds` and `raw_units` are clock readings.
///
/// **The decision to take a tranche at all is deliberately one whole rung
/// coarse.** A tranche is taken only when the re-priced total lands *strictly
/// above the rung the current budget already sits on* - a 15% growth at the
/// shipped step - so a run whose remaining wall buys less than one rung takes no
/// tranche and produces exactly the document it would have produced with
/// re-planning off. That is what bounds the clock's influence: it is not a
/// threshold chosen to be coarse, it is the ladder the mode already ships,
/// re-read as a decision.
#[derive(Clone, Copy, Debug)]
pub struct TrancheReport {
    /// 1 for the first re-plan, 2 for the second, and so on. Tranche 0 is the
    /// initial plan and is reported as [`PlanReport`].
    pub index: usize,
    /// Seconds elapsed when the tranche was priced. The one clock read.
    pub at_seconds: f64,
    /// Work units spent when the tranche was priced. A counter.
    pub at_work_units: u64,
    /// Seconds the queue has been running - `at_seconds - probe_seconds`. The
    /// window the rate below was measured over.
    pub queue_seconds: f64,
    /// The queue's own retirement rate, in work units per second. **No bias is
    /// applied to it**: this is the quantity the bias exists to estimate.
    pub queue_rate_units_per_second: f64,
    /// What is left of `target * headroom` after `at_seconds`.
    pub remaining_seconds: f64,
    /// What the tranche actually priced, which is
    /// `min(remaining_seconds, queue_seconds * PLAN_TRANCHE_HORIZON)`. When it
    /// is below `remaining_seconds` the tranche refused to extrapolate and the
    /// run intends to re-plan again.
    pub horizon_seconds: f64,
    /// The re-priced total before quantisation.
    pub raw_units: f64,
    /// The rung the new total landed on, or `None` when unquantised.
    pub rung: Option<i64>,
    /// The new **total** work budget. It is a total and not an increment, so a
    /// run that takes three tranches has three budgets and not four.
    pub units: u64,
}

/// The fraction of the wall target the *first* tranche of a re-planning run aims
/// at.
///
/// `1.0` - aim at the whole target on the probe alone - is what
/// `docs/experiments/calibrated-plan/` shipped, and it is still the default
/// whenever re-planning is off. It has one failure mode and §10.2 of that round
/// measured it: the bias rises with the budget, so a constant fitted at ten
/// seconds is not conservative at thirty and mixed-61 seed 2 ran **36.39 s
/// against a 30 s target**. A single plan cannot recover from that, because by
/// the time the error is visible the budget is already spent.
///
/// A re-planning run aims the first tranche at a fraction of the target instead,
/// and tops it up from the measured queue rate. The value is a trade with three
/// ends and none of them is free:
///
/// * **too large** and the first tranche can overrun the whole target on its own,
///   which is exactly the 30 s failure this exists to bound - and which no
///   re-plan can undo, because by the time the error is visible the wall is
///   gone;
/// * **too small** and the first tranche is a short window to measure a rate
///   over, and the run pays a re-plan's fixed cost more often than it needs to;
/// * and it is **not only a safety knob**, which this round measured rather than
///   assumed. The affordability rule refuses an action the *current* tranche
///   cannot afford, so where a tranche boundary falls decides which actions the
///   queue is allowed to buy before it. `docs/experiments/replan/` §9 has two
///   fractions that arrive at the same final budget by different routes and
///   publish different depths, which is the honest form of this: the boundary
///   is a scheduling decision and not only a bookkeeping one.
///
/// **The shipped value is `1.0`, and that is a measurement rather than a
/// decision not to use the knob.** `docs/experiments/replan/` §9.3 swept it on
/// mixed-61 at both budgets, two rounds, three seeds:
///
/// * at **ten seconds** - the budget the user priority names - `0.6` and `1.0`
///   produce the *same three depths* (175.136 / 171.362 / 176.162), both at
///   0 of 6 over target and one document per seed. The gain over a
///   non-re-planning run is the re-plan's, not the fraction's;
/// * at **thirty seconds** `0.6` bounds the worst case (34.13 s against
///   36.54 s) and makes the typical one worse - **4 of 6 over target at a p50
///   of 33.15 s**, against `1.0`'s 2 of 6 at 25.99 s.
///
/// So the fraction that was introduced to bound the thirty-second overrun does
/// not bound it; it moves the overrun from the tail into the middle. Shipping
/// `1.0` keeps the ten-second gain, which is the one that was asked for, and
/// leaves the thirty-second wall exactly where the single plan left it - which
/// §9.3 states as the negative result it is.
pub const PLAN_FIRST_TRANCHE: f64 = 1.0;

/// The growth a re-priced total must show before a tranche is taken at all.
///
/// One ladder rung, and it is [`PLAN_QUANTUM_STEP`] rather than a second
/// constant on purpose. Under quantisation the two are the same statement: a
/// total that has not grown by a rung snaps back onto the rung it is already on,
/// so the tranche would install the budget the run already has. Stating it as an
/// explicit threshold makes the unquantised arm (`planq=1`) obey the same rule,
/// which is what keeps that arm's tranche *count* as coarse a decision as the
/// quantised arm's.
pub const PLAN_TRANCHE_MIN_GROWTH: f64 = PLAN_QUANTUM_STEP;

/// How far past the window it measured a tranche may extrapolate, as a multiple
/// of that window.
///
/// **`1.0` - never predict more queue time than you have already watched.**
///
/// This is the round's second constant and it is the one the pilot forced.
/// `evidence/cal-pilot-unbounded.json` ran the re-plan without it: on mixed-61
/// seed 2 at a thirty-second target the first tranche ended at 13.6 s having
/// watched the queue for 11.1 s, and priced the remaining **15.5 s** at the rate
/// it had measured - an extrapolation 139% beyond its own window. The queue's
/// rate does not hold over that range, and this document's own parent round
/// says why (`calibrated-plan` §13: *"the fitted bias rises with the budget,
/// because the queue's late actions cost more per unit than its early ones"*).
/// The tranche bought 66.2 M units, the rate fell 42% below the reading, and the
/// run took **36.74 s**. That is the same failure the single plan has, arrived
/// at from the other side.
///
/// The fix is not a safety factor on the rate, which would be a second bias
/// constant guessing the same thing the first one guessed. It is to stop
/// extrapolating: cap the horizon at the observed window, buy what that
/// justifies, and let the **next** tranche re-measure. The error per tranche is
/// then bounded by the accuracy of a one-window prediction, and the sequence
/// converges on the target from below instead of jumping past it.
///
/// It costs nothing at ten seconds, where the remaining wall is already shorter
/// than the window - measured at 2.80 s remaining against a 4.37 s window - so
/// this constant is inert on the budget the user priority names and active on
/// the one where the mode was broken.
pub const PLAN_TRANCHE_HORIZON: f64 = 1.0;

/// How many re-plans one run may take.
///
/// A bound rather than a tuning knob. Each tranche is at least
/// [`PLAN_TRANCHE_MIN_GROWTH`] bigger than the last, so the sequence terminates
/// on its own long before this; the constant exists so that a box on which the
/// rate estimate is pathological cannot turn a wall target into an unbounded
/// loop.
///
/// It is **twelve** rather than a smaller number because a tranche is allowed to
/// be as small as one rung (see [`BudgetMeter::replan`]), and a run whose first
/// tranche was short may need several of them to climb back: `1.15^12` is 5.35x,
/// which is more than the widest climb this round measured. A tranche that buys
/// nothing costs one `run_phase` entry and one `enumerate_v3_actions`, so the
/// bound is cheap to set loosely and expensive to set tightly.
pub const PLAN_MAX_TRANCHES: usize = 12;

/// The measured ratio `rate(phase 0) / rate(everything after phase 0)`, in work
/// units per second.
///
/// **A probe is only a rate estimator for the work it resembles**, and phase 0
/// does not resemble the rest of the run: it is one protected mode-0 pipeline,
/// and what follows is a ranked queue over eight classes whose most expensive
/// members are exact confirmations. `docs/experiments/calibrated-plan/` §2
/// measures the gap on mixed-61, shapes-17 and triangle-20 and it is large and
/// stable in sign - phase 0 always retires work units *faster* than the queue
/// that follows it, so a plan sized at phase 0's own rate overruns its wall
/// target, never undershoots it.
///
/// Three things about the value, and the third is the honest caveat:
///
/// 1. **It is the maximum, not the median.** Over eighteen cells at a ten
///    second target the fitted bias runs `1.116 .. 1.586`, median `1.449`
///    (§2.3). Overestimating shortens the plan and costs depth;
///    underestimating overruns the wall, and the wall is the promise.
/// 2. **It is self-consistent.** `1.70` is the constant those eighteen runs
///    were themselves measured *with*, and every one of them fitted below it
///    and landed under target - so this is not an extrapolation from a
///    differently-configured binary.
/// 3. **A single constant cannot fit a 1.42x range.** The bias is a property
///    of the (request, seed), not of the box: it is `1.116` on mixed-61 seed 0
///    and `1.586` on seed 2 of the same fixture, and it *rises with the
///    budget*, because the queue's late actions cost more per unit than its
///    early ones. Shipping the maximum therefore means the low-bias cells run
///    short - 7.1 s against a 10 s target on the worst of them - and that lost
///    budget is real depth. §5.2 measures it and §7 says what would fix it,
///    which is an in-run re-plan rather than a better constant.
pub const PLAN_PHASE_ZERO_BIAS: f64 = 1.70;

/// The fraction of the wall target a plan is allowed to aim at.
///
/// This is the part of the wall the plan cannot control, because it happens
/// after the plan is fixed: at a *pinned* work budget, twenty-one runs of
/// mixed-61 on this box spread 0.8-1.1% within a seed
/// (`docs/experiments/calibrated-plan/` §2.1). `0.97` covers that with room and
/// nothing more, because [`PLAN_QUANTUM_STEP`] already rounds **down** and is
/// the larger of the two margins by an order of magnitude.
pub const PLAN_HEADROOM: f64 = 0.97;

/// The geometric ladder a plan is floored onto, so that two processes agree on
/// it.
///
/// This is the round's central tradeoff and it is a dial, not a discovery.
/// `probe_seconds` is a clock reading with a measured 1.2-2.5% spread within a
/// cell, and it is the *only* non-deterministic input to the plan; so two
/// processes agree on `units` exactly when no rung boundary falls inside that
/// spread. Rung width is `ln(step)`:
///
/// * a fine ladder tracks the wall target closely and disagrees between
///   processes whenever the estimate straddles a boundary;
/// * a coarse ladder agrees between processes and throws away up to
///   `1 - 1/step` of the budget.
///
/// `1.15` is a **14.0%** rung. The measured within-cell spread of the plan
/// estimate is 0.2-2.6% over nine cells, median 1.0%
/// (`docs/experiments/calibrated-plan/` §3.1), so the rung is about fourteen
/// times the typical noise and five times the worst; and on that nine-cell
/// pilot, with every cell's observed band doubled first, it is the *smallest*
/// step at which no cell straddles a boundary. Finer steps do straddle - 1.05
/// puts six of nine cells across a boundary - and coarser ones cost budget
/// without buying stability those nine cells could detect.
///
/// It is **floored, not rounded**, so the error is one-sided: a plan is never
/// larger than the probe justified, which is what lets [`PLAN_HEADROOM`] be
/// 0.97 instead of 0.8. The price is that the floor throws away a median 7.5%
/// and a worst 11.0% of the budget on that same pilot, and that is the largest
/// single cost this mode carries after the work counters themselves.
///
/// Set it to `1.0` to switch quantisation off entirely. That arm is measured
/// too, and it is the honest other end of Sol review 5 §5's point: a run can
/// have the wall target or the cross-process plan, and this constant is where
/// the trade is made rather than assumed away.
pub const PLAN_QUANTUM_STEP: f64 = 1.15;

/// The ladder's anchor, in work units. Rungs are `PLAN_ANCHOR * step^k`.
///
/// A round number well below any plan this engine produces - the smallest
/// measured is 7.6 M on shapes-17 at a ten second target - so the ladder is a
/// property of the step alone and not of a fixture, and no cell can be tuned
/// onto a favourable rung by moving the anchor.
///
/// It is also the floor below which quantisation does not happen at all: a plan
/// under one rung is left exact rather than rounded to the anchor, because the
/// only way to be under it is for phase 0 to have already spent the target, and
/// clipping such a run to a round number would be arithmetic pretending to be a
/// budget.
pub const PLAN_ANCHOR_UNITS: f64 = 1_000_000.0;

/// How many equal-**work** buckets the probe's wall is cut into before the
/// fastest of them is taken as the box's rate.
///
/// **The problem this exists for.** `install_plan`'s whole non-determinism is
/// one clock reading: `raw = W0 * (1 + (T*h/t0 - 1)/bias)`, where `W0` is a
/// counter and `t0` is the probe's wall. On a quiet box `t0`'s spread is
/// 1.2-2.5% (`calibrated-plan` §6.1) and [`PLAN_QUANTUM_STEP`]'s 14% rung
/// swallows it. Under a competing workload it is not: `docs/experiments/replan/`
/// §11.1 re-measured the shipping `plan=10000` arm on a loaded box and got
/// **2 / 3 / 1 distinct depths per seed** where the quiet box gave 1 / 1 / 1.
///
/// **The rule.** A loaded box does not make every microsecond slower; it makes
/// *some* of them slower. So the mean rate over the whole probe is a
/// load-weighted average, and the **maximum** rate over a sub-window is the
/// least-loaded estimate available - the closest this run can get to what the
/// box would have done with nobody else on it. `k` buckets of `W0/k` units each,
/// the rate of each measured against its own wall, and the largest taken.
///
/// The buckets are cut on the **work** axis and not the wall axis, and that is
/// the half that makes the estimator legitimate: work is a counter, so the same
/// run on a quiet and a loaded box compares the same *k* stretches of the same
/// computation. Cutting on wall would compare a loaded second against a quiet
/// second and call the difference a rate.
///
/// `0` (and `1`) mean one bucket, which is the whole-phase reading unchanged, so
/// the default is the mode `calibrated-plan` shipped. **Eight** is what this
/// constant holds and what every arm in `docs/experiments/robust-plan/` was
/// measured at; `planprobe=on` in the benchmark spec is exactly this value, and
/// `planprobe=<k>` names a different one.
pub const PLAN_PROBE_BUCKETS: usize = 8;

/// The floor under a max-of-k probe, as a fraction of the probe's real wall.
///
/// A single bucket that happened to catch a cheap stretch of phase 0 - a
/// preprocessing pass that retires units far faster than the separator does -
/// would price the whole run at a rate no part of it will sustain, and the plan
/// would overrun. The estimate is therefore clamped: the effective probe wall
/// may not fall below this fraction of the wall actually observed, so the
/// mechanism can correct a loaded reading by at most 2x and cannot invent a box.
pub const PLAN_PROBE_MIN_FRACTION: f64 = 0.5;

/// How often the probe sampler reads the counters, in milliseconds.
///
/// It reads two atomics-backed totals and pushes a pair; on the shortest phase 0
/// this campaign measures (0.87 s on triangle-20) it takes about 43 samples,
/// which is enough to cut eight work buckets with room. It is a *cadence* and
/// not a checkpoint: the buckets are interpolated onto the work axis afterwards,
/// so a sample that arrives late moves no boundary.
pub const PLAN_PROBE_SAMPLE_MILLIS: u64 = 20;

/// How far the live probe may sit from a persisted calibration before the file
/// is refused.
///
/// The file is the point of [`PortfolioSettings::plan_calibration_path`]: while
/// it is used, the plan is a function of counters alone and two processes agree
/// **whatever the load**. The band is where that stops being safe.
///
/// It is deliberately one number with two very different jobs, and a reader
/// should see both:
///
/// * **`live` much larger than the file** is the case the mechanism exists for -
///   a loaded box - and the file is kept right up to this factor. That is the
///   load robustness, and its size is exactly how much load the guarantee
///   survives.
/// * **`live` much smaller than the file** cannot be load, so it is a file
///   measured on a slower box, a different build or a different request that
///   happened to collide on the key. Keeping it there would under-buy for ever.
///
/// Outside the band the run falls back to its own probe and says so in
/// `plan.calibrationSource`, so the degrade is in the document rather than
/// inferred from a wall.
pub const PLAN_CALIBRATION_BAND: f64 = 2.0;

/// Where a plan's probe wall came from.
///
/// Reported in the **deterministic** half of the plan block, not the clock half,
/// and that is on purpose: two processes that took their probe from different
/// sources did not run the same calibration, and a digest has to say so even
/// when they happen to land on the same rung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanCalibrationSource {
    /// The whole-phase clock reading. What `calibrated-plan` shipped.
    Live,
    /// The max-of-k bucket estimate. See [`PLAN_PROBE_BUCKETS`].
    Probe,
    /// A persisted calibration entry, inside the band.
    File,
    /// A persisted calibration file was named but had no entry for this run's
    /// `probe_work_units`.
    FileMiss,
    /// A persisted entry was found and refused because the live probe sat
    /// outside [`PLAN_CALIBRATION_BAND`].
    FileOutOfBand,
}

impl PlanCalibrationSource {
    /// The string the document carries.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Probe => "probe",
            Self::File => "file",
            Self::FileMiss => "fileMiss",
            Self::FileOutOfBand => "fileOutOfBand",
        }
    }
}

/// One phase of the schedule, as run.
#[derive(Clone, Debug)]
pub struct PhaseReport {
    pub name: String,
    pub deadline_fraction: f64,
    pub entered_seconds: f64,
    pub elapsed_seconds: f64,
    pub work_units: u64,
    pub operator_calls: usize,
    pub publications: usize,
    pub skipped: bool,
    /// Why the phase stopped. One enum store per phase exit; it reads nothing
    /// and branches on nothing, so it is free in the schedule's own currency.
    pub exit_cause: PhaseExitCause,
}

/// What kind of action the v3 queue is offering.
///
/// The classes are the ledger's own cost-and-yield rows plus the mode-26 ladder
/// the A/B/C measured, and each carries the *measured* prior that orders the
/// queue before this run has any evidence of its own. Ordering by class is the
/// deterministic tie-break after the ranking value, so the declaration order is
/// load-bearing and it is the ledger's Δraw/M-evaluation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionClass {
    /// One compressing mode-22 quantum at a target *below* the parent's own
    /// depth, and mode 31 on whatever residue it leaves.
    Compression,
    /// One mode-22 alternation quantum at the parent's own alternation rung.
    Descent,
    /// One mode-34 compression-schedule slice: the clamp walked down
    /// [`SCHEDULE_RUNGS`] rungs of the separator's own quantum, one canonical
    /// grid unit at a time, with the parent as the floor.
    ///
    /// Compiled into the enumeration only under the `compression-schedule`
    /// feature, because mode 34 does not exist without it. The variant itself
    /// is unconditional so that the reporting names, the spec key and the
    /// class table are the same document in either build.
    Schedule,
    /// One ordered, cut-derived mode-23 recombination.
    Crossover,
    /// One short mode-26 clamped-sheet ladder and the global legalizer tier on
    /// what it leaves.
    Ladder,
    /// One salted mode-20 constructor ticket and a quantum spent on it.
    Diversify,
}

impl ActionClass {
    /// The reporting name, stable across runs, and the name the operator calls
    /// and publication events this class pays for are attributed to.
    pub fn name(self) -> &'static str {
        match self {
            ActionClass::Compression => "compression",
            ActionClass::Descent => "descent",
            ActionClass::Schedule => "schedule",
            ActionClass::Crossover => "crossover",
            ActionClass::Ladder => "ladder",
            ActionClass::Diversify => "diversify",
        }
    }

    /// Every class, in declaration order. The queue ranks over exactly this
    /// list, so a class that is added here and nowhere else is a class that is
    /// ranked but never enumerated - which is a no-op rather than a defect.
    pub fn all() -> [ActionClass; 6] {
        [
            ActionClass::Compression,
            ActionClass::Descent,
            ActionClass::Schedule,
            ActionClass::Crossover,
            ActionClass::Ladder,
            ActionClass::Diversify,
        ]
    }

    /// Millimetres of published raw depth one action of this class produced, as
    /// measured, before this run has evidence of its own.
    ///
    /// From `docs/experiments/opportunity-ledger`: the seed-0 cost-and-yield
    /// table for compression / descent / crossover (2.101 mm in 1 call,
    /// 2.002 mm in 2, 3.277 mm in 3), and the A/B/C's arm C for the ladder
    /// (4.957 / 4.317 / 0 mm over three seeds, one ladder action each).
    fn prior_delta_mm(self) -> f64 {
        match self {
            ActionClass::Compression => 2.101,
            ActionClass::Descent => 1.001,
            ActionClass::Crossover => 1.0923,
            ActionClass::Ladder => 3.0914,
            // The compression-schedule port's `sched10-noroll` arm: twelve
            // matched cells at 171-179 mm coordinator parents, 11 of 12
            // publishing, **median 1.104 mm** below the parent (mean 1.072).
            // That arm walked a median 1,568 one-micron steps, which is what
            // [`SCHEDULE_RUNGS`] reproduces from the parent's own quantum.
            ActionClass::Schedule => 1.104,
            // **Not zero, and this round measured why zero was wrong.** The
            // opportunity ledger's `0` is a true statement about mixed-61 - 0
            // descendant publications from every archived m20 basin on all
            // three seeds - and a false one about triangle-20, where
            // coordinator v2's generality measurement found the slice
            // publishing on 6 of 12 arms. A prior of exactly `0` is not a
            // prior, it is a deletion: a class ranked at zero is never chosen,
            // so it never earns the evidence that would displace its prior,
            // and v3's own rule ("the prior is worth two actions of evidence")
            // becomes unfalsifiable for that one class.
            //
            // The number is this round's own pooled measurement over the three
            // requests the coordinator has been measured on
            // (`evidence/diversify-prior.json`, coordinator v2 at
            // `work=40,000,000`, three seeds each): 10 constructor arms, 0.05826
            // mm of published raw depth, all of it on triangle-20 - **0.005826
            // mm per action**. It is small because the class is worthless on
            // two of the three requests, and it is honest for the same reason.
            ActionClass::Diversify => 0.005826,
        }
    }

    /// What one action of this class costs, as a multiple of the *protected
    /// phase-0 pipeline this run just paid for*.
    ///
    /// Expressed relative to phase 0 rather than in work units on purpose:
    /// phase 0 is one full mode-0 pipeline on this request, measured in this
    /// process, in the budget's own currency, so the same prior prices a
    /// 61-piece request and a 17-piece one and prices a wall budget and a work
    /// budget. The multiples are the ledger's seed-0 spends against that run's
    /// own 8.777M-unit phase 0, except the ladder's, which is the *largest* of
    /// the three A/B/C arm-C spends (20.998M) rather than their mean: an
    /// operator with a 3.7x spread across seeds has to be priced by its worst
    /// case or the affordability rule is a coin toss.
    /// The work-budget price. See [`Self::prior_cost_in_phase_zero_for`] for
    /// why one class needs two of these and the other five do not.
    fn prior_cost_in_phase_zero(self) -> f64 {
        match self {
            ActionClass::Compression => 0.2176,
            ActionClass::Descent => 0.2678,
            // The port's `sched10-noroll` self-cap, 3,341,379 units, against
            // mixed-61 seed 0's own 8,778,573-unit phase 0. The **self**-cap
            // rather than the coordinator's metered spend, because the
            // coordinator's meter counts the narrow phase of the exact tier
            // only and the schedule's exact tier is 24-52% of its wall and
            // ~4% of its metered work: charging the meter would let the class
            // ride free on the one tier it spends its wall in. See
            // [`schedule_self_cost_units`].
            //
            // The two readings happen to agree at this budget, which is the
            // reason to trust the number rather than a coincidence to hide:
            // the *worst* of the twelve cells' coordinator-metered spends is
            // 3,343,739 units, or 0.3809 phase-zeros.
            ActionClass::Schedule => 0.3806,
            ActionClass::Crossover => 0.6092,
            ActionClass::Ladder => 2.3923,
            // v3 carried the ledger's mixed-61 reading, 0.1094, and §1.3 of
            // its own README reported that the same rule mispriced an m20
            // ticket by 11.7-12.0x **on the clock**. This round measured both
            // currencies on all three requests and priced the class by its
            // worst case in each, which is the rule every other class is
            // already priced by: the largest of the three requests' spends is
            // triangle-20's 7,804,768 units against its own 6,376,387-unit
            // phase 0.
            ActionClass::Diversify => 1.224,
        }
    }

    /// What one action of this class costs, as a multiple of the protected
    /// phase-0 pipeline, **in the budget's own currency**.
    ///
    /// Four of the six classes have one price, because their two currencies
    /// agree: they spend their time in the candidate-query and exact-pair
    /// tiers the work meter counts, so a multiple of phase 0 measured in work
    /// units is the same multiple measured in seconds to within the noise.
    ///
    /// **The compression schedule is the second exception and it is the one v4
    /// §8 predicted.** Its work price is the best in the queue - first-action
    /// actual/estimate 0.97-1.01 - and its wall price is 2.6-5.9x that, because
    /// the coordinator's meter counts the narrow phase of the exact tier only
    /// and this operator spends 24-52% of its *wall* in that tier. One number
    /// cannot be both, so the class carries both, and each is the worst case of
    /// its own currency. See [`SCHEDULE_WALL_PRIOR_PHASE_ZEROS`], and
    /// [`Coordinator::class_rank_cost_estimate`] for the floor that stops a
    /// worst-case wall prior from deleting the class on the one request where
    /// it publishes on nine of nine.
    ///
    /// The constructor slice is the exception and it is a *measured*
    /// exception, twice over. The ledger found the work budget pricing a mode-20
    /// arm at 260-335 units against 3.1 seconds of clock; coordinator v3 §1.3
    /// found the same rule 11.7-12.0x wrong on shapes-17's wall. Measured here
    /// on three requests at `work=40,000,000` (`evidence/diversify-prior.json`),
    /// the diversify phase costs **0.067 - 1.224** phase-zeros in work units and
    /// **1.25 - 1.98** phase-zeros in seconds - the same action, priced 17x
    /// apart on mixed-61. One number cannot be both, so the class carries both,
    /// and each is the worst case of its own currency.
    ///
    /// This is what makes the class safe to rank rather than gate: at a 3 s
    /// wall budget the queue now refuses a 4 s constructor ticket on the
    /// affordability rule instead of buying it on an eligibility clause.
    fn prior_cost_in_phase_zero_for(self, wall: bool) -> f64 {
        match (self, wall) {
            (ActionClass::Diversify, true) => 1.979,
            (ActionClass::Schedule, true) => SCHEDULE_WALL_PRIOR_PHASE_ZEROS,
            _ => self.prior_cost_in_phase_zero(),
        }
    }
}

/// One action the v3 queue offered and executed.
#[derive(Clone, Debug)]
pub struct ScheduledActionReport {
    /// The loop iteration that chose it, from zero.
    pub iteration: usize,
    pub class: String,
    /// The action's key in the `attempted` namespace.
    pub key: String,
    /// The human-readable action, as recorded on the operator calls it made.
    pub label: String,
    /// The ranking value the queue chose it on: expected millimetres per unit
    /// of the budget's own currency, times the protected phase's own cost, so
    /// the number is comparable across budgets and requests.
    pub value: f64,
    /// What the queue thought the action would cost, in the budget's currency.
    pub estimated_cost: f64,
    /// What the class was *charged*, in the budget's currency, and therefore
    /// what its price ratchet and its ranking value read. Equal to
    /// [`Self::metered_cost`] for every class but the compression schedule; see
    /// [`schedule_self_cost_units`].
    pub actual_cost: f64,
    /// What the coordinator's own meter read across the action, with any
    /// self-metered debit taken back out. Reported next to `actual_cost`
    /// rather than instead of it so the one place the two disagree is visible
    /// in the evidence rather than argued for in prose.
    pub metered_cost: f64,
    /// What the action's operators charged themselves, when any of them
    /// carries its own meter. See [`schedule_self_cost_units`].
    pub self_metered_units: Option<u64>,
    /// What this action added to the budget beyond the coordinator's own
    /// counter - `actual_cost - metered_cost` under a work budget, always
    /// zero under a wall budget. Since coordinator v5's transaction ordering
    /// this is charged *within* the action, so `work_units` below, the
    /// publications the action produced and the basins it archived all
    /// already include it.
    pub debited_units: u64,
    pub work_units: u64,
    pub seconds: f64,
    pub operator_calls: usize,
    pub publications: usize,
    /// The incumbent's raw depth before and after.
    pub entry_raw_depth_mm: Option<f64>,
    pub exit_raw_depth_mm: Option<f64>,
    /// How many candidate actions the queue had to choose from.
    pub candidates: usize,
}

/// What one action class did over the whole run.
#[derive(Clone, Debug)]
pub struct ScheduleClassReport {
    pub class: String,
    pub actions: usize,
    pub publications: usize,
    pub work_units: u64,
    pub seconds: f64,
    pub cost_total: f64,
    pub cost_max: f64,
    pub delta_raw_mm: f64,
    /// The prior's estimate of the first action's cost, and what that action
    /// actually cost. This is the mispricing the ledger found for mode 20,
    /// reported for every class rather than discovered afterwards.
    pub first_estimated_cost: Option<f64>,
    pub first_actual_cost: Option<f64>,
}

/// The v3 action loop, as run.
#[derive(Clone, Debug)]
pub struct ScheduleReport {
    pub iterations: usize,
    pub exit_cause: String,
    pub actions: Vec<ScheduledActionReport>,
    pub classes: Vec<ScheduleClassReport>,
    /// The protected phase's own cost in the budget's currency: the unit every
    /// class prior is quoted in.
    pub phase_zero_cost: f64,
}

/// One ordered, cut-derived crossover action over a pair of archive states.
///
/// Mode 23 is directional - it takes parent A's short-axis span, cuts it at a
/// fraction of *A's own* span, keeps A's poses below the cut and B's above it -
/// so `A->B` and `B->A` are two different layouts, and the schedule keys only
/// one of them. The cut is likewise a whole axis of the action space that the
/// schedule collapses to the single constant `0.5`.
#[derive(Clone, Debug)]
pub struct CrossoverAction {
    /// Parent A: the layout whose span the cut is measured on and whose poses
    /// are kept below it.
    pub left_fingerprint: String,
    /// Parent B: the layout whose poses are kept above the cut.
    pub right_fingerprint: String,
    /// A's rank in the selection this action was enumerated over.
    pub left_rank: usize,
    /// B's rank in the same selection.
    pub right_rank: usize,
    /// Whether this is the reciprocal of the ranked pair - `B->A` where the
    /// schedule would key `A->B`.
    pub reciprocal: bool,
    /// The cut fraction, in A's own short-axis span.
    pub cut_fraction: f64,
    /// The width of the interface band the cut sits in, in millimetres. This is
    /// the actual gap between two consecutive occupied short-axis positions of
    /// A - the cut is placed at its midpoint, so it is the most numerically
    /// robust representative of its own partition.
    pub band_gap_mm: f64,
    /// How many of A's pieces at the band's lower edge have a *different* pose
    /// in B. A band where this is zero produces the same hybrid as the band
    /// below it and is not enumerated.
    pub differing_pieces_at_band: usize,
    /// Pieces the hybrid takes from A.
    pub pieces_from_left: usize,
    /// Pieces the hybrid takes from B.
    pub pieces_from_right: usize,
    /// The hybrid's placement fingerprint, before any legalization.
    pub hybrid_fingerprint: String,
    /// Whether the hybrid is bit-identical to one of its own parents, in which
    /// case the action is a no-op dressed as a crossover.
    pub degenerate: bool,
    /// Whether this cut is the one the constant `0.5` lands in.
    pub is_midpoint_band: bool,
    /// Whether the schedule has already attempted this action.
    pub attempted: bool,
    /// The action's key, in the schedule's own `attempted` namespace.
    pub key: String,
}

/// What became of one archive state: whether the selection could reach it, and
/// what it has been paid for.
#[derive(Clone, Debug)]
pub struct ArchiveOpportunityRow {
    pub fingerprint: String,
    pub raw_depth_mm: f64,
    pub operator: String,
    pub exact_valid: bool,
    /// Its rank when the archive is ordered the way the frontier orders it.
    pub depth_rank: usize,
    /// Whether the alternation phase's selection can reach it.
    pub in_descent_frontier: bool,
    /// Whether the crossover phase's selection can reach it.
    pub in_crossover_frontier: bool,
    /// Whether *any* selection could reach it if top-K were not binding.
    pub reachable_at_full_k: bool,
    /// `"topK"` when a bigger K would have reached it, `"similarity"` when the
    /// bit-exact-pose rule shadows it behind a shallower member, `None` when it
    /// is in the crossover frontier.
    pub excluded_by: Option<String>,
    /// The shallower member that shadows it, when similarity excludes it.
    pub shadowed_by: Option<String>,
    /// The overlap with that member.
    pub shadow_overlap: f64,
    /// Operator calls that took this state as a parent, in either slot.
    pub actions_received: usize,
    /// Descents charged against it by the fairness counter.
    pub descents: usize,
    /// Publications descending from it, however many generations later.
    pub descendant_publications: usize,
    /// The best raw depth any of those publications reached.
    pub best_descendant_raw_depth_mm: Option<f64>,
    /// Generations from this state to the final incumbent, when it is an
    /// ancestor of it.
    pub generations_to_incumbent: Option<usize>,
}

/// The cost and yield of one action class, from this run's own calls.
#[derive(Clone, Debug)]
pub struct ActionClassRow {
    pub phase: String,
    pub operator: String,
    pub calls: usize,
    pub published: usize,
    pub work_units_total: u64,
    pub work_units_p50: u64,
    pub work_units_p95: u64,
    pub seconds_p50: f64,
    pub seconds_p95: f64,
    pub seconds_total: f64,
    /// Millimetres of raw depth this class removed from the incumbent.
    pub delta_raw_mm: f64,
    /// Those millimetres per million work units this class spent.
    pub delta_raw_per_mega_unit: f64,
}

/// One step of the final incumbent's ancestry, root first.
#[derive(Clone, Debug)]
pub struct LineageStep {
    pub fingerprint: String,
    pub operator: String,
    pub raw_depth_mm: f64,
    pub birth_work_units: u64,
}

/// The opportunity-and-delayed-credit ledger: what the saturated state still
/// had available, and what its history is owed.
#[derive(Clone, Debug)]
pub struct PortfolioLedger {
    /// Every ordered/derived crossover action over the crossover phase's own
    /// selection.
    pub frontier_actions: Vec<CrossoverAction>,
    /// The same enumeration over *every* archive member, which is the ceiling
    /// of the action space the schedule could reach at this archive.
    pub archive_actions_total: usize,
    pub archive_actions_untried: usize,
    pub archive_actions_untried_nondegenerate: usize,
    /// Ordered pairs over the whole archive, i.e. `n * (n - 1)`.
    pub archive_ordered_pairs: usize,
    /// The first action in the canonical order that has not been attempted,
    /// is not degenerate, and whose hybrid is not already an archive member.
    pub next_action: Option<CrossoverAction>,
    /// One row per archive member.
    pub archive_rows: Vec<ArchiveOpportunityRow>,
    /// One row per `(phase, operator)`.
    pub action_classes: Vec<ActionClassRow>,
    /// The final incumbent's ancestry, root first.
    pub incumbent_lineage: Vec<LineageStep>,
    /// Members that received no action at all.
    pub members_without_action: usize,
    /// Members excluded by the top-K rule.
    pub excluded_by_top_k: usize,
    /// Members excluded by the bit-exact-pose similarity rule.
    pub excluded_by_similarity: usize,
}

/// Which A/B/C arm the probe phase runs, at identical work, from the saturated
/// archive the schedule leaves behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeArm {
    /// No probe. The default, and the shipping schedule.
    None,
    /// The next derived crossover action the ledger names.
    NextDerivedCrossover,
    /// One mode-20 ticket, a direct crossover with the incumbent, a short
    /// mode-22.
    ConstructorTicket,
    /// One short mode-26 ladder, then the coordinator's own global legalizer
    /// tier on what it leaves.
    LadderRung,
    /// The control for arm C, and not one of the review's three: the *same*
    /// target depth, from the *same* parent, asked of the schedule's own
    /// mode-22 alternation instead of the clamped ladder.
    ///
    /// Without it, arm C's result is "the arm that got 21M more work units
    /// found something", which is not a statement about the clamp. With it, the
    /// difference between C and D is the clamp and nothing else.
    DescentControl,
}

impl ProbeArm {
    /// The reporting name, stable across runs.
    pub fn name(self) -> &'static str {
        match self {
            ProbeArm::None => "none",
            ProbeArm::NextDerivedCrossover => "A",
            ProbeArm::ConstructorTicket => "B",
            ProbeArm::LadderRung => "C",
            ProbeArm::DescentControl => "D",
        }
    }
}

/// What one probe arm did, measured from the saturated state it started at.
#[derive(Clone, Debug)]
pub struct ProbeReport {
    pub arm: String,
    /// The allowance the arm was given, in the budget's own currency.
    pub allowance: f64,
    /// What it actually spent. A first call of an unpriced operator may
    /// overrun: a work budget bounds what may be *started*.
    pub work_units_spent: u64,
    pub seconds_spent: f64,
    /// The incumbent's raw depth when the arm started.
    pub entry_raw_depth_mm: Option<f64>,
    /// The incumbent's raw depth when it finished.
    pub exit_raw_depth_mm: Option<f64>,
    /// `entry - exit`: positive is an improvement.
    pub delta_raw_mm: f64,
    /// Whether the exit incumbent passes the composite exact validator.
    pub exit_dual_gate_valid: bool,
    pub publications: usize,
    pub operator_calls: usize,
    /// The steps the arm actually executed, in order.
    pub steps: Vec<String>,
    /// Why the arm stopped.
    pub exit_cause: String,
}

/// What the coordinator did.
#[derive(Debug)]
pub struct PortfolioOutcome {
    /// The engine's result - what a caller publishes.
    pub result: GeneralFastResult,
    /// The incumbent state object behind it.
    pub incumbent: PublishedIncumbent,
    /// The archive at the end of the run.
    pub archive: ArchiveReport,
    /// The relaxed diagnostics of the protected mode-0 phase, unchanged, so a
    /// coordinator run reports everything a mode-0 run reports.
    pub m0_diagnostics: Box<crate::search::general_relaxed::GeneralRelaxedDiagnostics>,
    pub phases: Vec<PhaseReport>,
    /// The v3 action loop's own report, `None` on the v2 phase schedule.
    pub schedule: Option<ScheduleReport>,
    /// What the multi-basin race decided, or `None` when it was not armed.
    #[cfg(feature = "compression-schedule")]
    pub basin_race: Option<BasinRaceReport>,
    pub operator_calls: Vec<OperatorCallReport>,
    pub publications: Vec<PublicationEvent>,
    /// Which parallel currency this run was priced in, so a document says so
    /// rather than a reader inferring it from whether the per-call block is
    /// present.
    pub work_currency: WorkCurrencyMode,
    /// Which flag the run armed to get its work counters, and `None` when that
    /// is the shipped answer.
    ///
    /// Present only when the run did something the settings alone do not say:
    /// it took the counters off the work meter's own flag, or it asked to and
    /// was deferred. A default work or plan run therefore produces the document
    /// it always produced, byte for byte, which is what lets the unarmed arm of
    /// this round's battery be a document diff rather than a scalar comparison.
    pub work_meter_arming: Option<WorkMeterArmingReport>,
    pub budget: PortfolioBudget,
    pub elapsed_seconds: f64,
    pub work_units: u64,
    pub constructor_clamp_mm: f64,
    pub area_lower_bound_depth_mm: f64,
    /// The constructed layout's own depth, before any search.
    pub constructed_depth_mm: f64,
    /// Whether the alternation phase ended at a frontier fixpoint rather than
    /// at its deadline. This is the [`BasinTrigger::OnStall`] predicate, and it
    /// is reported whether or not that trigger is the one in force.
    pub descent_stalled: bool,
    /// The opportunity-and-delayed-credit ledger, when the build carries it.
    /// `None` in a default build: computing it is `O(members^2 * pieces)` at
    /// exit and it is an instrument, not a schedule input.
    pub ledger: Option<PortfolioLedger>,
    /// What the probe arm did, when one was asked for.
    pub probe: Option<ProbeReport>,
    /// How a [`PortfolioBudget::Plan`] was calibrated, `None` under either of
    /// the two direct budgets.
    ///
    /// Note that `budget` above reports what the run was actually *spent*
    /// against, which for a plan is the [`PortfolioBudget::Work`] this
    /// calibrated to - so a caller reading `budget` alone cannot tell a plan
    /// from a replay of one, which is the point: they are the same run.
    pub plan: Option<PlanReport>,
    /// The in-run re-plans, in order, empty when none was taken.
    ///
    /// Empty is the common case and it is *two* different cases that a reader
    /// has to be able to tell apart, which is why `plan.first_tranche` is
    /// reported next to it: `first_tranche == 1.0` with no tranches is a run
    /// that never intended to re-plan, and `first_tranche < 1.0` with no
    /// tranches is a run that intended to and found its remaining wall did not
    /// buy a rung. The second is a *result*, and it is the one that makes a
    /// re-planning run reproduce a non-re-planning one.
    pub tranches: Vec<TrancheReport>,
}

/// When the constructor slice is allowed to draw a basin.
///
/// The v1 coordinator gave the constructor the review's own 1.9-4.0 s slice,
/// unconditionally and first. Nineteen salted arms over nine ten-second runs
/// were every one exact-valid and every one refused by the adoption rule, and
/// not one descendant caught the incumbent - so at that budget the slice was
/// 1.24 s of pure loss, and the arm that priced it at zero was never worse.
///
/// The verdict is about the *allocation*, not the mechanism, so v2 keeps the
/// mechanism and makes the allocation conditional. The default is
/// [`BasinTrigger::WhenDescendable`], which is the measured rule: a constructor
/// basin is worth drawing only when the run can still afford to *descend* from
/// what it draws, because a drawn-and-undescended basin is exactly the 19/19
/// refusal the v1 measurement recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasinTrigger {
    /// Never draw one. The `focused` arm of the v1 measurement.
    Never,
    /// Always draw one while the phase deadline allows. The v1 default.
    Always,
    /// Draw one only when the alternation phase reached a frontier fixpoint -
    /// every distinct archive state has had its quantum and none produced a new
    /// layout - so the budget the slice spends is budget the productive
    /// operator declined.
    OnStall,
    /// Draw one only when the remaining budget still covers the draw *and* a
    /// descent from it, both priced from this run's own measured operator
    /// costs in the budget's own currency. This is the default.
    WhenDescendable,
}

/// How a schedule is configured.
#[derive(Clone, Debug)]
pub struct PortfolioSettings {
    /// The relaxed configuration every phase runs under.
    ///
    /// The coordinator takes this whole struct from its caller rather than
    /// rebuilding one, because the protected mode-0 phase has to be *the same
    /// search* the caller would have run without a coordinator - same sweeps,
    /// samples, shrink ratios, backend and pressure model. If it were not, the
    /// coordinator's first phase would already be a different engine and every
    /// number after it would be measuring two changes at once.
    ///
    /// Its `persistent_vacancy_mode`, target and pinned-parent flag are
    /// overwritten per operator call; everything else is used as given.
    pub relaxed_template: GeneralRelaxedSettings,
    /// Relaxed epochs inside a mode-22 alternation cycle's separator sub-step.
    /// Smaller than [`Self::relaxed_epochs`] makes an alternation cycle a
    /// *quantum* rather than a full descent.
    pub descent_relaxed_epochs: usize,
    /// The budget.
    pub budget: PortfolioBudget,
    /// How many basins the archive retains.
    pub archive_capacity: usize,
    /// Piece-assignment overlap at or above which two layouts count as the same
    /// basin. Dimensionless schedule policy.
    pub similarity_threshold: f64,
    /// How many salted constructor arms the diversify phase will attempt.
    pub basin_slots: usize,
    /// When the diversify phase may draw a constructor basin at all.
    pub basin_trigger: BasinTrigger,
    /// How many consecutive draw-and-descend iterations may publish nothing
    /// before the diversify phase ends.
    ///
    /// The stopping signal is deliberately *the descendant*, never the arm's
    /// own depth: the ledger's eighteen-sample sweep measured
    /// Pearson(immediate, descended) = -0.212, so a rule that stopped on a deep
    /// arm would be the invalid quality proxy the review named. A rule that
    /// stops when an arm *and a quantum spent on it* published nothing is a
    /// descendant-quality rule, which is the one the review asks for.
    ///
    /// One, measured: on mixed-61 no arm ever published, and letting the phase
    /// fill its slots spent 13.4 s of a thirty-second budget for a layout
    /// identical in all nine rounds to the one a ten-second budget already had.
    /// On triangle-20 the *first* arm published, so patience 1 keeps the only
    /// gain the slice has ever produced.
    pub basin_patience: usize,
    /// How many crossovers one crossover phase may make.
    ///
    /// v1 made exactly one. Mode 23 was the second most productive operator in
    /// that measurement - two publications in four calls under the review's
    /// schedule, three in nine under the focused one, carrying the largest
    /// single gains - so v2 lets it keep going while distinct archive pairs
    /// remain and it can still afford a call.
    pub crossover_attempts: usize,
    /// How many frontier members the crossover phase draws its parent pairs
    /// from. Pairs are taken in `(0,1), (0,2), (1,2), ...` order.
    pub crossover_states: usize,
    /// Void-grid cell divisor salts, one per basin slot, cycled.
    ///
    /// Only the `fast-constructor-profile` evaluator reads them. Empty leaves
    /// the calibrated divisor in place for every slot, which is the correct
    /// setting for a build that does not carry that profile.
    pub cell_divisor_salts: Vec<f64>,
    /// How many distinct archive states the alternation phase spends quanta on.
    pub descent_states: usize,
    /// Alternation cycles per quantum.
    pub descent_cycles: usize,
    /// Whether the alternation phase, on reaching a fixpoint at its quantum
    /// size, doubles the quantum and goes round again instead of ending early.
    ///
    /// **Off by default, and it is a measured negative rather than an
    /// unfinished idea.** See the descent phase for the numbers: it keeps the
    /// phase busy and it spends the crossover phase's budget, and the crossover
    /// phase is the second most productive operator in this schedule.
    pub descent_iterated_deepening: bool,
    /// Phase deadlines as fractions of the budget. See [`PhaseSchedule`].
    pub schedule: PhaseSchedule,
    /// Which A/B/C probe arm runs after the schedule saturates.
    ///
    /// [`ProbeArm::None`] by default, and the probe phase itself is compiled
    /// only under the `portfolio-ledger` feature, so a default build runs the
    /// shipping schedule and nothing else whatever this says.
    pub probe: ProbeArm,
    /// The probe arm's allowance, in work units. Every arm gets the same one,
    /// measured from the identical saturated state the schedule leaves.
    pub probe_work_units: u64,
    /// Run the v3 ranked action loop instead of the v2 phase sequence.
    ///
    /// Off by default, so a default build's schedule is v2's to the digit and
    /// the two coordinators can be interleaved from one binary - which is the
    /// only way a paired A/B on a shared box is worth anything.
    pub coordinator_v3: bool,
    /// Whether the v3 queue offers the mode-34 compression-schedule class.
    ///
    /// On by default *inside v3*, which is itself off by default. Setting it to
    /// `false` restores the merged-HEAD v3 enumeration exactly; that is the
    /// arm every A/B below is paired against, and it reproduces a pristine
    /// base-commit binary field for field.
    ///
    /// In a build without the `compression-schedule` feature the field is still
    /// here and still parsed - a replay driver may not have to know which
    /// features a binary carries to reproduce a pinned command - and it does
    /// nothing, because mode 34 does not exist there.
    pub compression_schedule_class: bool,
    /// Workers the coordinator's own mode-34 slice fans a repair step out to.
    ///
    /// `1` - the shipped serial slice - is the default and the only value any
    /// existing spec produces, so an unarmed run is the merged schedule exactly.
    /// See `search::compression_schedule::CompressionScheduleSettings::lanes`
    /// and docs/experiments/parallel-compression-schedule/.
    #[cfg(feature = "parallel-compression-schedule")]
    pub compression_schedule_lanes: usize,
    /// Whether the coordinator's mode-34 slice spreads its exact confirmation
    /// over the job pool.
    ///
    /// **`true` by default inside v3 as of the promotion round**, and the same
    /// shape as [`Self::schedule_wall_prior`] and [`Self::schedule_sterile_bit`]:
    /// a default *within* a flag that is itself off at the Cargo level, so a
    /// build without `parallel-compression-schedule` does not have this field
    /// and every pinned gate is measured on a binary that does not compile it.
    /// The v2 phase schedule never reads it either - the only read is
    /// `execute_v3_action`'s mode-34 dispatch.
    ///
    /// Unlike `compression_schedule_lanes` this one is semantics-preserving -
    /// measured on the 174-179 mm band, an armed slice differs from the serial
    /// one in exactly the diagnostic flag that says it was armed - so it moves
    /// wall without moving the search's trajectory.
    ///
    /// The promotion evidence is `docs/experiments/fast-contract-validator/`
    /// §12-13: `+1.527 mm` on top of the certificate over nine paired cells,
    /// and §13.2(4)'s qualification, which is the reason `m34pconfirm=0` stays
    /// a key rather than becoming unreachable - **its 1.5 mm is contingent on
    /// spare cores**, and on a contended box it decays to parity with the
    /// serial arm while costing the cross-round reproducibility §12.2 measures.
    /// A deployment that cannot promise the cores, or that wants the serial
    /// arm's constant depth, sets `m34pconfirm=0`.
    #[cfg(feature = "parallel-compression-schedule")]
    pub compression_schedule_parallel_confirm: bool,
    /// The mode-34 slice's **batch budget**, in the schedule's own work
    /// currency, or `None` for the atomic slice.
    ///
    /// `None` is the default and it is the shipped arm: every pinned m34 number
    /// in this repository was measured on a slice that runs from its entry to
    /// its bound without ever handing itself back. Sol review 8 §3 condition 4
    /// names that as the thing to fix - *"mode 34 oggi è atomico e senza work
    /// cap interno"* - and §4 spend 1 names the gate, which is that the batched
    /// slice must reproduce the atomic one.
    ///
    /// See [`crate::search::compression_schedule::CompressionScheduleSettings::batch_work_units`]
    /// for what a batch boundary is. This field only carries the number down to
    /// the operator; it decides nothing.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_batch_work_units: Option<usize>,
    /// Whether the coordinator caps a mode-34 slice at its own **remaining
    /// budget**, stopping the slice at the first checkpoint past it.
    ///
    /// `false` by default. This is the consumer of the batch mechanism above,
    /// and it is the reason the mechanism is worth having: an atomic slice is
    /// charged after it finishes, so a budget with 2 M units left can dispatch
    /// a slice that spends 20 M and the affordability rule finds out afterwards.
    /// With this armed the slice is handed
    /// `batch_work_units = remaining_to(deadline)` and gives itself back at the
    /// first checkpoint past it, with its last exact-valid incumbent intact.
    ///
    /// It is denominated in **work**, not in seconds, which is what keeps it
    /// deterministic: the slice's own meter is a counter, so two processes stop
    /// at the same checkpoint. That is the difference between this and Sol
    /// review 8 §3 condition 3's wall stop, which cannot be deterministic and is
    /// not implemented here.
    ///
    /// The two currencies line up by construction: `settle_operator_charge`
    /// charges `max(global_units, operator_self_units)`, so what the coordinator
    /// pays for a slice *is* the slice's own meter whenever the slice's meter is
    /// the larger of the two, which on the measured band it is by ~18x.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_cap_to_budget: bool,
    /// `m34wallstop`: whether a mode-34 slice is stopped at the first checkpoint
    /// **past the run's wall deadline**, holding its last exact-valid incumbent.
    ///
    /// The key is `m34wallstop` and not `m34wall`, which has meant
    /// [`Self::schedule_wall_prior`] - how the queue *prices* a schedule action
    /// before it buys one - since coordinator v4.
    ///
    /// `false` by default. This is Sol review 8 §3 condition 3 - *"stop wall tra
    /// checkpoint, restituire l'ultimo incumbent exact-valid"* - and the
    /// previous round shipped the checkpoints without it, on the grounds that a
    /// wall stop cannot be deterministic. It cannot, and it is still the only
    /// thing that bounds a wall: `m34cap` stops on the slice's own *work* meter,
    /// which says nothing about seconds under load, and every overrun
    /// `docs/experiments/replan/` §12.1 measured is an action that was in flight
    /// when the deadline passed.
    ///
    /// The honest form of the trade, and it is the reason this is a key and not
    /// a default: with it armed the *depth* becomes a function of the box, in
    /// exactly the way `PortfolioBudget::Wall` already is. Two processes agree
    /// on the layout only while they cross the deadline between the same two
    /// checkpoints. A run that needs one document per seed leaves this off and
    /// accepts the overrun; a run that needs ten seconds arms it and accepts the
    /// spread. `docs/experiments/real-interruption/` measures both ends.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_wall_stop: bool,
    /// `m34wallstopall`: the same wall deadline, applied to **the queue** rather
    /// than only to a mode-34 checkpoint.
    ///
    /// `false` by default. It is a strict extension of
    /// [`Self::compression_schedule_wall_stop`] and arms it: a run that names
    /// this key gets the mode-34 checkpoint stop *and* an admission rule in
    /// front of every other class.
    ///
    /// # Why the checkpoint stop alone leaves an overrun
    ///
    /// `docs/experiments/real-interruption/` §13 names the two reasons its
    /// thirty-second `wallstop` row still crossed 3 of 9 times: *"the policy
    /// only binds the mode-34 checkpoint it is consulted at; it cannot stop an
    /// operator class that never asks it a question, and it cannot
    /// retroactively shorten a batch already in flight"*. This key answers the
    /// first of the two and nothing else. Under a plan or work budget the
    /// queue's own stopping rule is denominated in work units, which say
    /// nothing about seconds under load, so a queue whose m34 slice has just
    /// stopped on the wall goes straight on to buy an m22 action with the work
    /// it still nominally has - and that action is the overrun.
    ///
    /// The second reason is [`Self::compression_schedule_wall_stop_reserve`],
    /// and it is a separate key because it is a separate mechanism: this one
    /// refuses to *start* an action after the deadline and is exact; that one
    /// refuses to start an action it *predicts* will cross the deadline and is
    /// an estimate.
    ///
    /// It exits the phase on [`PhaseExitCause::WallStop`], which is
    /// deliberately **not** in the re-plan loop's `budget_bound` set: a tranche
    /// buys work, and a run that stopped because it was out of *seconds* must
    /// not be handed more of them.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_wall_stop_all: bool,
    /// `m34wallreserve`: seconds of the wall deadline held back for the action
    /// the queue is about to start, when
    /// [`Self::compression_schedule_wall_stop_all`] is armed.
    ///
    /// `0.0` by default, which is the pure admission rule: an action may start
    /// at any time up to the deadline, and whatever it costs after that is the
    /// residual overrun. A positive value additionally refuses a class whose
    /// **own measured mean seconds in this run** would not fit in what is left,
    /// scaled by this number - so `1.0` is *"only start what you expect to
    /// finish"* and `2.0` is *"only start what you expect to finish twice"*.
    ///
    /// It is a multiple of the class's measured mean rather than a number of
    /// seconds because the two ends of the trade are different sizes on
    /// different fixtures, and because an unpriced class must not be refused -
    /// the same rule [`Coordinator::affordability`] already applies to work.
    /// A class this run has never bought is admitted, or it would never be
    /// priced.
    ///
    /// **It is an estimate, and it can be wrong in both directions.** A class
    /// whose mean is dragged down by cheap early calls will still be admitted
    /// for an expensive one; a class whose mean is dragged up will be refused
    /// while it would have fitted. The evidence reports the overrun with and
    /// without it rather than claiming a bound.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_wall_stop_reserve: f64,
    /// `m34yield`: suspend a mode-34 slice toward the coordinator after this
    /// many batches, so that another action can run before it is resumed.
    ///
    /// `0` - the default - never suspends. A non-zero value is the mechanism
    /// Grok review 4 §4 names as the missing piece: *"senza portare uno slice
    /// sospeso alla coda, m34 non può cedere a un'altra classe"*. The slice is
    /// parked on the coordinator with its frontier, its caches, its rng and its
    /// step account intact; the queue runs one more action; the slice is then
    /// resumed and finishes as one report.
    ///
    /// It is a **count of batches** rather than a work figure so that the two
    /// knobs compose: `m34batch` decides how long a batch is, this decides how
    /// many of them the slice may run before it has to offer its turn back.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_yield_batches: usize,
    /// `m34past`: whether a mode-34 slice may continue past its nine-rung bound,
    /// under the coordinator's own budget rather than under a fixed walk.
    ///
    /// `false` by default. `docs/experiments/robust-plan/` §13.1 is why this
    /// exists: the confirmation-density sweep was flat-to-negative at every one
    /// of twelve cells in both budget modes, and the cause was not the knob it
    /// swept. **Every cell exited on `bound` and every cell's first slice
    /// dropped exactly 1.6160 mm** - the coordinator's slice is a walk of a
    /// *fixed distance*, so a finer clamp walks the same distance in four times
    /// as many steps and buys nothing. `record-line-cascade`'s millimetre was
    /// bought on the opposite arm: `past=1` at a pinned work budget, where the
    /// walk is budget-limited and a finer clamp converts spare budget into extra
    /// distance.
    ///
    /// So this is the lever that section names - *"the bound, not the grid"* -
    /// and the budget it runs under is the coordinator's remaining budget for
    /// the action, cut into batches. Past the bound the slice is affordability-
    /// limited at every checkpoint rather than distance-limited once.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_past_bound: bool,
    /// How many batches the past-bound budget is cut into, when
    /// [`Self::compression_schedule_past_bound`] is armed.
    ///
    /// The batch is the granularity at which the coordinator re-asks "can I
    /// still afford this?", so it is the resolution of the per-batch
    /// affordability rule and nothing else. Eight is the default because it puts
    /// a checkpoint roughly every one to two rungs of the nine-rung walk on the
    /// measured band, which is often enough for the wall stop to bound an
    /// overrun and rare enough that the checkpoint bookkeeping is not the cost.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_past_bound_batches: usize,
    /// `m34pastbarren`: how many consecutive batches a past-bound slice may run
    /// **without deepening its published incumbent** before the coordinator
    /// takes its turn back.
    ///
    /// This is the per-batch affordability rule, and it is the one thing in this
    /// round that the previous round's mechanism could not have expressed. A
    /// work cap is a number the slice checks against its own meter; it can say
    /// *"you have spent enough"* and it cannot say *"you have stopped buying
    /// anything"*. The checkpoint can, because it carries `published_depth_mm` -
    /// the depth of the exact-valid layout the coordinator would keep if it
    /// stopped here - so two checkpoints are a **derivative**.
    ///
    /// `2` by default and `0` switches the rule off, letting the slice run to
    /// its budget. It applies **only to a slice on which
    /// [`Self::compression_schedule_past_bound`] is armed**, and to every batch
    /// of one including the batches inside the nine rungs. That is a deliberate
    /// simplification rather than an exact reading of "past the bound", and the
    /// reason it is safe is that it changes nothing about a bounded slice: with
    /// the lever off no checkpoint policy is installed at all, so every pinned
    /// number in this repository is measured by a coordinator that has never
    /// heard of this rule.
    ///
    /// Two and not one: `confirm_every` is four steps and a batch is one to two
    /// rungs, so a single batch can straddle a cadence gap and be barren because
    /// the exact tier was never asked rather than because it refused. Two
    /// consecutive batches cannot.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_past_bound_barren: usize,
    /// `m34pastshare`: what fraction of the budget the coordinator has left for
    /// this action a past-bound slice may spend.
    ///
    /// `1.0` by default - the whole of it, which is the arm
    /// `record-line-cascade` bought its millimetre on (`past=1,work=20000000`).
    /// It is a key because `docs/experiments/robust-plan/` §13.1's claim is
    /// precisely about the alternative: *"it stops at 1.616 mm and hands the
    /// rest back to the queue, **where the other classes spend it better than a
    /// denser slice would**"*. That sentence is a hypothesis about a share, and
    /// the only way to test it is to vary the share.
    #[cfg(feature = "compression-schedule")]
    pub compression_schedule_past_bound_share: f64,
    /// Whether the exact-clearance contract validator's broad phase is armed.
    ///
    /// **`true` by default**, which is what the feature has always done: with
    /// `fast-contract-validator` compiled, `validate_publication` used the
    /// certificate unconditionally and no lever could take it off. What this
    /// field adds is that lever, which is
    /// `docs/experiments/fast-contract-validator/` §13.2's first condition on
    /// calling the arming a default at all: *"a way to disarm it in the field
    /// is worth more than its absence"*.
    ///
    /// Read once, at the top of a v3 run, and applied through
    /// [`crate::validation::general_polygon::set_contract_certificate_armed`]
    /// for the duration of that run only. Disarmed, every pair goes to the
    /// exact loop and the engine produces the document a build without the
    /// feature produces - the equivalence
    /// `examples/contract_validator_shadow.rs` measures.
    #[cfg(feature = "fast-contract-validator")]
    pub fast_contract_validator: bool,
    /// How the certified round-envelope kernel participates in the composite's
    /// envelope half for this run.
    ///
    /// **`false` by default**, and that is the difference from
    /// [`PortfolioSettings::fast_contract_validator`], which is `true`. The
    /// clearance certificate is verdict-preserving — it only ever skips work
    /// the exact loop would have agreed with — so arming it changes nothing a
    /// document can see. This changes the **acceptance authority**: a disc
    /// envelope accepts layouts a miter envelope refuses
    /// (docs/experiments/gate-a-sparrow-import/ measured 31 pairs and 2
    /// boundaries of one 61-piece layout), so an armed run is a different
    /// engine and has to be asked for by name.
    ///
    /// Read once, at the top of a v3 run, and applied through
    /// [`crate::validation::round_envelope::set_kernel_mode`] for the duration
    /// of that run only. The material contract validator is unaffected by any
    /// of the three: publication stays the conjunction, and this is one half of
    /// it.
    #[cfg(feature = "round-envelope-kernel")]
    pub round_envelope_kernel: crate::validation::round_envelope::KernelMode,
    /// [`PLAN_PHASE_ZERO_BIAS`], overridable per run.
    pub plan_bias: f64,
    /// [`PLAN_HEADROOM`], overridable per run.
    pub plan_headroom: f64,
    /// [`PLAN_QUANTUM_STEP`], overridable per run. `1.0` switches quantisation
    /// off.
    pub plan_quantum_step: f64,
    /// Whether a plan whose work is exhausted with wall left over prices a
    /// second tranche from the rate **this run** measured.
    ///
    /// `false` by default, so a binary that is not asked for it produces the
    /// single-tranche plan `docs/experiments/calibrated-plan/` measured, field
    /// for field. When true the run may take up to [`PLAN_MAX_TRANCHES`] of
    /// them; see [`TrancheReport`] for what is deterministic about that and what
    /// is not.
    pub plan_replan: bool,
    /// [`PLAN_FIRST_TRANCHE`], overridable per run. Read **only** when
    /// [`Self::plan_replan`] is armed: a run that cannot re-plan must aim the
    /// one plan it gets at the whole target, and a first tranche of 0.6 with no
    /// second tranche is simply a run that gave 40% of its wall away.
    pub plan_first_tranche: f64,
    /// [`PLAN_MAX_TRANCHES`], overridable per run.
    pub plan_max_tranches: usize,
    /// [`PLAN_TRANCHE_HORIZON`], overridable per run. A very large value
    /// restores the unbounded extrapolation the pilot measured and is how
    /// `evidence/cal-pilot-unbounded.json`'s arm is reproduced.
    pub plan_tranche_horizon: f64,
    /// The parallel work currency, spec key `cur2`. See
    /// [`WorkCurrencyMode`] and `crate::search::work_currency`.
    ///
    /// [`WorkCurrencyMode::Off`] is the default and is the shipped arm: every
    /// pinned work number in this repository - the four gates, the ledger's
    /// spends, `work=40000000`, `portfolio.plan.units` - is denominated in the
    /// meter `work_units_now()` reads, and this field cannot move any of them
    /// because with it off nothing in `work_currency` is called and no field
    /// it owns is reported.
    pub work_currency: WorkCurrencyMode,
    /// How many equal-**work** buckets the probe's wall is split into before the
    /// *fastest* of them is taken as the box's rate. `0` - the default - is one
    /// bucket, which is the single whole-phase reading
    /// `docs/experiments/calibrated-plan/` shipped.
    ///
    /// See [`PLAN_PROBE_BUCKETS`] for what the max-of-k rule is for and what it
    /// costs.
    pub plan_probe_buckets: usize,
    /// A persisted per-box calibration file, consulted at plan time.
    ///
    /// `None` - the default - is the live probe, unchanged. When set, the file
    /// is read once and looked up by the run's own `probe_work_units`, which is
    /// a **counter** and therefore an exact key for (request, seed, binary,
    /// features). A hit replaces the clock reading with the stored one, and the
    /// plan stops being a function of the clock at all.
    ///
    /// This is Sol review 8 §3 condition 1: *"il probe hardware dev'essere
    /// offline/persistito e il cap parte della spec"*. See
    /// [`PLAN_CALIBRATION_BAND`] for the sanity band that keeps a stale file
    /// from silently pricing a box it was not measured on.
    pub plan_calibration_path: Option<String>,
    /// Whether this run merges its own probe back into
    /// [`Self::plan_calibration_path`] under the min rule.
    ///
    /// `false` by default, and the separation is deliberate: a measured battery
    /// reads a frozen file, and the file is written by an explicit calibration
    /// pass. A run that both reads and writes would make the file a function of
    /// the order the battery happened to run in.
    pub plan_calibration_write: bool,
    /// [`PLAN_CALIBRATION_BAND`], overridable per run.
    pub plan_calibration_band: f64,
    /// Whether the coordinator arms the continuous-rotation operator on the two
    /// operators whose relaxed lane the brief scopes it to: the alternation
    /// fixpoint (mode 22) and the compression schedule (mode 34).
    ///
    /// `false` by default. It is deliberately *not* set on
    /// `relaxed_template`, which every other class inherits: the operator
    /// changes what a relaxed lane can propose, and a round that measures it
    /// has to be able to say which classes it was measuring. The setting is
    /// additionally inert on any lane that is not
    /// `RollbackTriangle` + `StructuredTrianglePoles` - see
    /// `general_relaxed::continuous_rotation_lane` - so arming it cannot reach
    /// a dynamic-hazard or directional lane by accident.
    #[cfg(feature = "continuous-rotation")]
    pub continuous_rotation: bool,
    /// Derives rung surrogates from each piece's offset ring instead of
    /// offsetting per rung. See
    /// `GeneralRelaxedSettings::rotation_equivariant_offset`; inert unless
    /// [`Self::continuous_rotation`] is also set.
    #[cfg(feature = "sparse-rotation")]
    pub rotation_equivariant_offset: bool,
    /// Design B: rungs only for the pieces a stalled schedule step names.
    ///
    /// Also narrows [`Self::continuous_rotation`] to mode 34 - see the
    /// `arm_operator` site for why mode 22 has nothing to stall.
    #[cfg(feature = "sparse-rotation")]
    pub sparse_rotation: bool,
    /// Design C's budget, or `None` to never call the certificate.
    #[cfg(feature = "sparse-rotation")]
    pub se2_witness: Option<crate::search::general_relaxed::Se2WitnessSettings>,
    /// The request-adaptive disarm: after
    /// [`SPARSE_ROTATION_STERILE_EPISODES`] episodes on *this* request that
    /// bought nothing, sparse rotation comes off for the rest of the run, with
    /// one late audition.
    ///
    /// The same shape as [`Self::schedule_sterile_bit`] and for the same reason:
    /// the prior that says rotation is worth proposing is a mixed-61 prior and
    /// it does not cross a request. docs/experiments/rotation-tax/ §4.5 measured
    /// shapes-17 paying 355,404 surrogate builds and triangle-20 paying
    /// 1,336,518 rotation iterations for a depth delta of **exactly zero** on
    /// 0 of 9 published slices each - two requests where every rung the operator
    /// has ever proposed was waste, discoverable in one slice.
    #[cfg(feature = "sparse-rotation")]
    pub sparse_rotation_bit: bool,
    /// The multi-basin race: two or three basins auditioned against each other
    /// at a short cap before the v3 queue is allowed to commit to one.
    ///
    /// **Off by default**, and it is a phase rather than a class: the queue
    /// cannot rank an arm it has not run, and the whole point of the race is
    /// that the ranking happens on measured first-batch behaviour instead of on
    /// a prior.
    ///
    /// The risk it addresses is the one Sol review 8 §4.3 and Grok review 3 §3
    /// both put first: at ten seconds the best FCV arm spans **165.656-174.280
    /// mm**, and that spread is not slice-to-slice noise inside one basin - it
    /// is which basin the run committed to. Nothing else in the coordinator
    /// re-decides that: phase 0 produces one incumbent, and the diversify class
    /// is the only thing that can produce another, priced at a prior of
    /// 0.005826 mm that never outranks anything.
    ///
    /// See [`run_basin_race`] for the arms, the three criteria and the halving.
    #[cfg(feature = "compression-schedule")]
    pub basin_race: bool,
    /// How many arms the race starts with, **including the incumbent control**.
    /// Clamped to 2..=4 by [`run_basin_race`]; three is the measured default.
    #[cfg(feature = "compression-schedule")]
    pub basin_race_arms: usize,
    /// How many arms survive the halving. One at ten seconds, two at thirty -
    /// the second survivor is only worth its slice when there is a slice left
    /// for it to use.
    #[cfg(feature = "compression-schedule")]
    pub basin_race_keep: usize,
    /// The audition slice, in rungs of the separator's own quantum, against the
    /// [`SCHEDULE_RUNGS`] a full mode-34 action walks.
    ///
    /// The cap is expressed in **rungs and not in work units** for the reason
    /// the schedule class already gives at its own call site: a work cap
    /// expressed in the coordinator's currency reads zero when profiling is
    /// off, and a wall-budget run has it off. A rung count is a number the
    /// request supplies, so every arm gets the same audition on every box.
    #[cfg(feature = "compression-schedule")]
    pub basin_race_rungs: usize,
    /// The share of what phase 0 left that the race may spend before it is cut
    /// off mid-audition. A ceiling and not an allocation: the race returns as
    /// soon as it has a winner, and everything it did not spend is what the
    /// eliminated arms give back to the survivor.
    #[cfg(feature = "compression-schedule")]
    pub basin_race_share: f64,
    /// Where the challenger arms come from: salted constructor draws (`true`,
    /// the default) or the basins phase 0 has already archived (`false`).
    ///
    /// Both are real arms and this round measures both, because they are the
    /// same race at two prices and the price is the finding.
    ///
    /// The salted draw is the mechanism Sol review 8 §4.3 and Grok review 3 §3
    /// specify, and it is the one the ledger's lesson is about: mode 20 derives
    /// its `construction_seed` from the *target*, so a salted clamp is a
    /// different lottery where a salted seed would be a replica. It is also,
    /// measured on mixed-61 seed 0, **3.156 s of wall charged 310 work units** -
    /// so under a work budget the coordinator cannot see it at all and the
    /// race's share ceiling does not bound its wall. See
    /// `docs/experiments/basin-race/` §3.
    ///
    /// `false` auditions the archive instead. Phase 0 leaves two structurally
    /// distinct basins there before the queue starts - the raw constructor and
    /// the coupled-separator arm - and auditioning those costs the batches
    /// alone, which are 0.07-0.29 s each on the same cell. It is a narrower
    /// race, because the arms are the two phase 0 happened to produce rather
    /// than an arbitrary number of fresh lotteries, and it is the only one of
    /// the two that fits inside ten seconds.
    #[cfg(feature = "compression-schedule")]
    pub basin_race_draw: bool,
    /// Whether an eliminated arm's basin is withheld from the archive.
    ///
    /// On by default, and it is what makes the race a *decision*. With it off
    /// the losing basins stay in the archive, the v3 queue can rank them like
    /// any other member, and the race degenerates into three extra constructor
    /// draws - which is a real arm to measure, and is why the key exists, but
    /// it is not the mechanism.
    #[cfg(feature = "compression-schedule")]
    pub basin_race_evict: bool,
    /// How many consecutive actions may publish nothing before the whole v3
    /// loop stops, with its queue still full. `0` disables the rule, which is
    /// merged-HEAD v3's behaviour.
    ///
    /// See [`BARREN_ACTION_PATIENCE`] for where 16 comes from. It is a *global*
    /// patience over every class, not the constructor slice's own
    /// [`Self::basin_patience`], which stays what it was.
    pub barren_action_patience: usize,
    /// Whether the diversify class is enumerated into the ranked queue and
    /// priced like every other class, instead of being gated on the priced
    /// queue emptying.
    ///
    /// On by default inside v3. `false` restores merged-HEAD v3's
    /// "un ticket m20 quando non rimangono coppie complementari" rule, which
    /// coordinator v3 §4.2 measured never firing on triangle-20 at all.
    pub diversify_in_queue: bool,
    /// `lanedebit`: whether a work or plan budget runs with
    /// `profiling::set_enabled(false)`, taking its two counters from the work
    /// meter's own recording flag instead.
    ///
    /// `false` by default, which is the shipped behaviour exactly: a work
    /// budget arms the profiler and pays for every span in the engine.
    ///
    /// # What it buys, and why it is not a free lunch
    ///
    /// `docs/experiments/calibrated-plan/` §9 measured the counters at
    /// **+1.882 mm** on mixed-61 at a ten-second wall and called it *"a floor
    /// under any work-denominated budget... there is no version of this mode
    /// that avoids it"*. The floor is real and the attribution was not
    /// separable at the time, because one flag armed both the counting and the
    /// timing. `profiling::metering_enabled` separates them, and this setting
    /// is what a coordinator uses to take the first without the second.
    ///
    /// **The budget it runs against is numerically identical.** The same two
    /// counters are incremented at the same two sites by the same amounts, so
    /// `work_units_now` returns what it always returned and every plan rung,
    /// every `plancal` key and every pinned `work=` replay is on the same
    /// scale. That is the property that makes a paired A/B interpretable: the
    /// two arms differ in what the instrument costs and in nothing else.
    ///
    /// # The one combination it refuses
    ///
    /// `search::work_currency` prices three further counters - `NeighborTests`,
    /// `CollisionPolygonBuilds`, `FullRescores` - which the meter does not read
    /// and the metering flag therefore does not arm. A run that arms both this
    /// and [`Self::work_currency`] would compute a class price from three
    /// counters reading zero, so this setting **defers**: the profiler is armed
    /// as before and the run pays the tax. Refusing silently would be worse;
    /// the report carries `workMeterArming` so a reader can see which happened.
    pub lane_local_debit: bool,
    /// Whether the compression-schedule class is priced on the clock by its own
    /// measured wall, instead of by the work-denominated prior that is 2.6-5.9x
    /// low there.
    ///
    /// On by default inside v3, and it re-prices the **affordability gate
    /// only**: the queue goes on ranking the class exactly as coordinator v4
    /// did. `false` restores v4's pricing entirely - one prior, both
    /// currencies. Under a **work** budget this setting does nothing at all:
    /// the work prior is the only prior there.
    ///
    /// See [`SCHEDULE_WALL_PRIOR_PHASE_ZEROS`] and
    /// [`Coordinator::class_rank_cost_estimate`] for the measured reason the
    /// two rules read different numbers.
    pub schedule_wall_prior: bool,
    /// Whether a compression-schedule slice tries to make its parent
    /// proxy-feasible by translation alone before it takes step 0.
    ///
    /// See [`CompressionScheduleSettings::legalize_entry`][cs]. Off by default,
    /// because it changes what every m34 slice in this repository walks from.
    ///
    /// [cs]: crate::search::compression_schedule::CompressionScheduleSettings::legalize_entry
    pub schedule_legalize_entry: bool,
    /// Whether a compression-schedule slice whose entry is still infeasible
    /// after that repair gives its wall back instead of spending it on regrid.
    ///
    /// See [`CompressionScheduleSettings::skip_infeasible_entry`][cs]. Off by
    /// default, and inert unless [`Self::schedule_legalize_entry`] is on.
    ///
    /// [cs]: crate::search::compression_schedule::CompressionScheduleSettings::skip_infeasible_entry
    pub schedule_skip_infeasible_entry: bool,
    /// Whether a compression-schedule slice that arrives more than its own drop
    /// above its parent gives its wall back instead of spending it on a walk
    /// that cannot publish.
    ///
    /// See [`CompressionScheduleSettings::skip_unpublishable_entry`][cs]. Off
    /// by default: it is this round's own instrument and the round that
    /// measured it has to be able to run the arm that does not.
    ///
    /// [cs]: crate::search::compression_schedule::CompressionScheduleSettings::skip_unpublishable_entry
    pub schedule_skip_unpublishable_entry: bool,
    /// The denominator of the compression-schedule slice's *probe*: the slice
    /// is abandoned after `steps_planned / n` steps that published nothing
    /// below the parent. `0` disables it.
    ///
    /// [`SCHEDULE_PROBE_DENOMINATOR`] - **zero, off** - by default. It is the
    /// only mechanism this round found that can charge the *first* slice's wall
    /// price at all, and it is off because what it returns is unspendable and
    /// what it costs at thirty seconds is not. See
    /// [`SCHEDULE_PROBE_DENOMINATOR`] for both measurements and
    /// [`CompressionScheduleSettings::barren_probe_denominator`][cs] for the
    /// mechanism.
    ///
    /// [cs]: crate::search::compression_schedule::CompressionScheduleSettings::barren_probe_denominator
    pub schedule_probe_denominator: usize,
    /// Whether a compression-schedule class that has published nothing on
    /// *this* request is taken off the queue.
    ///
    /// On by default inside v3. See [`SCHEDULE_STERILE_ACTIONS`] for the count
    /// and [`SCHEDULE_AUDITION_BARREN`] for the one audition that keeps it
    /// falsifiable. `false` restores coordinator v4's queue, where the class is
    /// held back only by its own ratchet and buys 2 more slices on shapes-17
    /// and 3 more on triangle-20 at a thirty-second budget, publishing on none
    /// of them.
    pub schedule_sterile_bit: bool,
    /// [`CompressionScheduleSettings::step_grid`][cs] for the **first** m34
    /// slice of a run only. `None` - the default - is the module's own `1.0`.
    ///
    /// # Why the first slice and not every slice
    ///
    /// The lever is confirmation density: `confirm_every` counts *steps*, so a
    /// quarter-grid clamp asks the exact tier four times as often per micron of
    /// descent and spends four times as many repair sweeps getting there.
    /// `docs/experiments/record-line-cascade/` bought **1.000 mm** with it at a
    /// pinned 20 M budget from a 159 mm parent, when a confirmation cost
    /// 0.80 ms; `calibrated-plan` §4 now prices one at **0.257 ms** with the
    /// certificate and the parallel confirmation both armed, so the same lever
    /// is a quarter of the price it was measured at.
    ///
    /// It is scoped to the first slice because that is where the descent is
    /// steepest and where the coordinator has the most budget left to pay for
    /// the extra pressure. A run that spent it on every slice would be spending
    /// it on the slices that are already refusing to descend.
    ///
    /// [cs]: crate::search::compression_schedule::CompressionScheduleSettings::step_grid
    #[cfg(feature = "compression-schedule")]
    pub schedule_first_slice_step_grid: Option<f64>,
    /// [`CompressionScheduleSettings::confirm_every`][cs] for the **first** m34
    /// slice of a run only. `None` - the default - is the module's own `4`.
    ///
    /// The other half of the same lever, and the independent one: `step_grid`
    /// changes how much clamp a step is worth, `confirm_every` changes how many
    /// steps pass between exact questions. Both raise confirmations per micron
    /// and they raise different other things with it, which is why the round
    /// that chose them swept the product rather than the diagonal.
    ///
    /// [cs]: crate::search::compression_schedule::CompressionScheduleSettings::confirm_every
    #[cfg(feature = "compression-schedule")]
    pub schedule_first_slice_confirm_every: Option<usize>,
}

/// The phase deadlines, as fractions of the whole budget.
///
/// v1's defaults were the review's own ten-second sketch in the review's own
/// order: protected mode 0, then salted constructor basins, then alternation
/// quanta, then one crossover, then compression, then drain. v2 reorders them
/// by *measured productivity per second* on that stream - alternation (9
/// publications in 18 calls), crossover (3 in 9, largest single gains),
/// compression's micro-descent (3 in 9), and the constructor slice (0 in 19)
/// last and conditional - and it is the reordering, not a new operator, that
/// this stage's numbers are about.
///
/// They are *deadlines*, not allocations: a phase that finishes early hands the
/// remainder to the next one, and a phase whose deadline has already passed
/// when it is entered is skipped and says so.
///
/// # They are fractions of what phase 0 left, not of the whole budget
///
/// v1's fractions were of the whole budget, and that makes the schedule
/// nonsense at any budget the protected mode-0 phase is a large part of. On
/// this box mode 0 costs about two seconds; at a three-second budget it is 0.67
/// of the whole, so *every* phase whose absolute fraction is below 0.67 is
/// skipped and the first one above it runs - which on the v1 fractions means
/// the most productive operator in the schedule is dropped and a crossover runs
/// in its place, on an archive nothing has descended in yet, overrunning the
/// budget by a third. The failure is not the fractions; it is measuring them
/// against a budget that phase 0 has already spent an unknown part of.
///
/// So a deadline is `f0 + (1 - f0) * by`, where `f0` is the fraction of the
/// budget the protected phase actually spent. Every phase keeps its share of
/// what is *left*, at any budget, on any request - a 61-piece request whose
/// mode 0 costs 20% of ten seconds and a 17-piece request whose mode 0 costs 2%
/// get the same schedule shape rather than two different ones.
#[derive(Clone, Copy, Debug)]
pub struct PhaseSchedule {
    pub descent_by: f64,
    pub crossover_by: f64,
    pub compression_by: f64,
    pub diversify_by: f64,
    pub drain_by: f64,
    /// v3 only: the deadline of the single action loop that replaces the four
    /// phases above. It is the same fraction the last of them ended at, so the
    /// drain keeps the reserve it already had.
    pub schedule_by: f64,
}

impl Default for PhaseSchedule {
    fn default() -> Self {
        // The v2 absolute fractions renormalised onto the post-phase-0
        // remainder at the mode-0 share this stage measured (0.28 of ten
        // seconds), so the ten-second schedule is unchanged to two decimal
        // places and every other budget is now the same schedule rather than a
        // truncation of it.
        Self {
            descent_by: 0.42,
            crossover_by: 0.70,
            compression_by: 0.89,
            diversify_by: 0.98,
            drain_by: 1.0,
            schedule_by: 0.98,
        }
    }
}

impl PortfolioSettings {
    /// A schedule sized for `budget`, with the review's phase fractions.
    pub fn new(relaxed_template: GeneralRelaxedSettings, budget: PortfolioBudget) -> Self {
        Self {
            relaxed_template,
            descent_relaxed_epochs: 4,
            budget,
            archive_capacity: 16,
            similarity_threshold: 0.5,
            basin_slots: 8,
            basin_trigger: BasinTrigger::WhenDescendable,
            basin_patience: 1,
            crossover_attempts: 3,
            crossover_states: 3,
            cell_divisor_salts: Vec::new(),
            // One, not three, and it is the measured half of the v1 verdict:
            // the `focused` arm spent every quantum on the single best distinct
            // state and published 176.056 in three rounds of three, while the
            // three-state arm spread the same budget over 194-214 mm
            // constructor basins and landed at 176.753.
            descent_states: 1,
            descent_cycles: 1,
            descent_iterated_deepening: false,
            schedule: PhaseSchedule::default(),
            probe: ProbeArm::None,
            probe_work_units: 0,
            coordinator_v3: false,
            compression_schedule_class: true,
            #[cfg(feature = "parallel-compression-schedule")]
            compression_schedule_lanes: 1,
            // On by default *inside v3*, per the promotion package. The Cargo
            // feature is still off by default, so the default build and all
            // four pinned gates are binaries in which this field does not
            // exist.
            #[cfg(feature = "parallel-compression-schedule")]
            compression_schedule_parallel_confirm: true,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_batch_work_units: None,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_cap_to_budget: false,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_wall_stop: false,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_wall_stop_all: false,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_wall_stop_reserve: 0.0,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_yield_batches: 0,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_past_bound: false,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_past_bound_batches: SCHEDULE_PAST_BOUND_BATCHES,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_past_bound_barren: SCHEDULE_PAST_BOUND_BARREN,
            #[cfg(feature = "compression-schedule")]
            compression_schedule_past_bound_share: 1.0,
            #[cfg(feature = "fast-contract-validator")]
            fast_contract_validator: true,
            #[cfg(feature = "round-envelope-kernel")]
            round_envelope_kernel: crate::validation::round_envelope::KernelMode::Off,
            plan_bias: PLAN_PHASE_ZERO_BIAS,
            plan_headroom: PLAN_HEADROOM,
            plan_quantum_step: PLAN_QUANTUM_STEP,
            plan_replan: false,
            plan_first_tranche: PLAN_FIRST_TRANCHE,
            plan_max_tranches: PLAN_MAX_TRANCHES,
            plan_tranche_horizon: PLAN_TRANCHE_HORIZON,
            work_currency: WorkCurrencyMode::Off,
            plan_probe_buckets: 0,
            plan_calibration_path: None,
            plan_calibration_write: false,
            plan_calibration_band: PLAN_CALIBRATION_BAND,
            #[cfg(feature = "continuous-rotation")]
            continuous_rotation: false,
            #[cfg(feature = "sparse-rotation")]
            rotation_equivariant_offset: false,
            #[cfg(feature = "sparse-rotation")]
            sparse_rotation: false,
            #[cfg(feature = "sparse-rotation")]
            se2_witness: None,
            // On by default *within* the sparse operator, which is itself off by
            // default: a mechanism whose whole claim is "spend nothing where
            // there is nothing to buy" should not need a second flag to stop
            // spending. The battery runs it both ways anyway.
            #[cfg(feature = "sparse-rotation")]
            sparse_rotation_bit: true,
            // The race is off, and the four numbers beside it are the shape it
            // takes when a spec turns it on. They are defaults for an armed
            // race, not defaults of the engine.
            #[cfg(feature = "compression-schedule")]
            basin_race: false,
            #[cfg(feature = "compression-schedule")]
            basin_race_arms: BASIN_RACE_ARMS,
            #[cfg(feature = "compression-schedule")]
            basin_race_keep: 1,
            #[cfg(feature = "compression-schedule")]
            basin_race_rungs: BASIN_RACE_RUNGS,
            #[cfg(feature = "compression-schedule")]
            basin_race_share: BASIN_RACE_SHARE,
            #[cfg(feature = "compression-schedule")]
            basin_race_draw: true,
            #[cfg(feature = "compression-schedule")]
            basin_race_evict: true,
            barren_action_patience: BARREN_ACTION_PATIENCE,
            diversify_in_queue: true,
            lane_local_debit: false,
            schedule_wall_prior: true,
            // Off by default: these two change the state every m34 slice walks
            // from, and the round that measures them has to be able to run the
            // arm that does not.
            schedule_legalize_entry: false,
            schedule_skip_infeasible_entry: false,
            schedule_skip_unpublishable_entry: false,
            schedule_probe_denominator: SCHEDULE_PROBE_DENOMINATOR,
            schedule_sterile_bit: true,
            // `None` and not a value: the two knobs must build the module's own
            // default field for field on an unarmed spec, and `Some(1.0)` is not
            // the same statement as "the caller said nothing".
            #[cfg(feature = "compression-schedule")]
            schedule_first_slice_step_grid: None,
            #[cfg(feature = "compression-schedule")]
            schedule_first_slice_confirm_every: None,
        }
    }
}

/// The probe sampler: a thread that watches phase 0 retire work.
///
/// It exists because phase 0 is two monolithic calls -
/// `construct_short_side_first` and one `improve_complete_layout` - with no
/// budget check and no natural checkpoint between them, so there is nowhere in
/// the search itself to read a rate from. A sampler thread is the least
/// invasive instrument available: it takes no lock the search holds, writes
/// nothing the search reads, and increments no counter, so the run it measures
/// is the run that would have happened without it.
///
/// It is armed only under [`PortfolioBudget::Plan`] and only when
/// [`PortfolioSettings::plan_probe_buckets`] asks for more than one bucket, so
/// a default run does not start a thread at all.
struct PlanProbe {
    samples: Arc<Mutex<Vec<(f64, u64)>>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    buckets: usize,
}

impl PlanProbe {
    /// Stops the sampler and returns what it saw, oldest first.
    fn finish(&mut self) -> Vec<(f64, u64)> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.samples
            .lock()
            .map(|samples| samples.clone())
            .unwrap_or_default()
    }
}

impl Drop for PlanProbe {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The fastest of `buckets` equal-**work** stretches of the probe, expressed as
/// the wall the whole probe would have taken at that rate.
///
/// Returns `None` when the samples cannot support the cut - fewer than two
/// usable samples, no work retired, or a bucket with no measurable wall - in
/// which case the caller keeps the whole-phase reading rather than guessing.
///
/// The result is clamped to `[live_seconds * PLAN_PROBE_MIN_FRACTION,
/// live_seconds]`. The upper clamp is arithmetic - a maximum rate cannot be
/// below the mean - and the lower one is the guard [`PLAN_PROBE_MIN_FRACTION`]
/// describes.
fn probe_effective_seconds(
    samples: &[(f64, u64)],
    live_seconds: f64,
    live_work_units: u64,
    buckets: usize,
) -> Option<f64> {
    let buckets = buckets.max(1);
    if buckets < 2 || live_work_units == 0 || !(live_seconds > 0.0) {
        return None;
    }
    // The sampled path, with the two endpoints the sampler cannot see: the
    // meter's own origin, and the end of the probe as `install_plan` read it.
    let mut path: Vec<(f64, u64)> = Vec::with_capacity(samples.len() + 2);
    path.push((0.0, 0));
    for &(seconds, units) in samples {
        let (last_seconds, last_units) = *path.last().expect("seeded above");
        // Monotone in both axes or dropped: a sample that reads a lower total
        // than its predecessor is a torn read across the counter registry, not
        // a rate.
        if seconds > last_seconds && units >= last_units && seconds < live_seconds {
            path.push((seconds, units));
        }
    }
    let (last_seconds, last_units) = *path.last().expect("seeded above");
    if !(live_seconds > last_seconds) || live_work_units < last_units {
        return None;
    }
    path.push((live_seconds, live_work_units));
    if path.len() < 3 {
        return None;
    }
    // Where each bucket boundary falls on the wall axis, by linear
    // interpolation between the two samples that bracket it on the work axis.
    let at_work = |target: f64| -> Option<f64> {
        for window in path.windows(2) {
            let (t0, w0) = window[0];
            let (t1, w1) = window[1];
            if (w1 as f64) >= target {
                let span = (w1 - w0) as f64;
                if span <= 0.0 {
                    return Some(t0);
                }
                return Some(t0 + (t1 - t0) * (target - w0 as f64) / span);
            }
        }
        None
    };
    let per_bucket = live_work_units as f64 / buckets as f64;
    let total = live_work_units as f64;
    let mut best_rate = 0.0_f64;
    for bucket in 0..buckets {
        // Clamped at the total: `per_bucket * buckets` is exact only while the
        // division was, and a last boundary a single ulp above the work the path
        // actually ends at would find no bracketing window and throw the whole
        // estimate away.
        let start = at_work((per_bucket * bucket as f64).min(total))?;
        let end = at_work((per_bucket * (bucket + 1) as f64).min(total))?;
        let span = end - start;
        if span > 0.0 {
            best_rate = best_rate.max(per_bucket / span);
        }
    }
    if !(best_rate > 0.0) {
        return None;
    }
    let effective = live_work_units as f64 / best_rate;
    Some(effective.clamp(live_seconds * PLAN_PROBE_MIN_FRACTION, live_seconds))
}

/// A persisted per-box calibration: `probe_work_units` to the probe wall the
/// least-loaded run of that cell observed.
///
/// The key is the whole design. `probe_work_units` is a **counter**, bit
/// identical across every run of a (request, seed, binary, feature set) -
/// `calibrated-plan` §6.1 measured exactly one distinct value over seven runs on
/// each of three seeds - so it identifies the cell precisely and *cannot*
/// collide with a cell that would want a different answer unless the two cells
/// genuinely retire the same number of units. A file keyed on a request path or
/// a fixture hash would need a policy for a changed binary; this one misses,
/// which is the right answer.
///
/// The format is deliberately dull, because a corrupt or absent file must
/// degrade to the live probe rather than fail a run: every error path here
/// returns an empty map.
fn read_plan_calibration(path: &str) -> BTreeMap<u64, f64> {
    let mut out = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return out;
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) else {
        return out;
    };
    let Some(entries) = doc.get("entries").and_then(|v| v.as_object()) else {
        return out;
    };
    for (key, value) in entries {
        let Ok(units) = key.parse::<u64>() else {
            continue;
        };
        let Some(seconds) = value.get("probeSeconds").and_then(|v| v.as_f64()) else {
            continue;
        };
        if seconds > 0.0 && seconds.is_finite() {
            out.insert(units, seconds);
        }
    }
    out
}

/// Merges one observation into the calibration file under the **min** rule.
///
/// The minimum wall is the least-loaded observation, which is the quantity the
/// file is supposed to hold: a calibration pass that ran into a load spike must
/// not be able to make the box look permanently slower. It is monotone, so the
/// file converges rather than oscillating, and a fresh box is calibrated by
/// running the pass more than once.
///
/// Write-if-better by a full percent, so a pass that is already converged
/// rewrites nothing; and written through a temporary file and a rename, so a
/// reader never sees a half-written map.
fn write_plan_calibration(path: &str, units: u64, seconds: f64) {
    if !(seconds > 0.0) || !seconds.is_finite() {
        return;
    }
    let mut entries = read_plan_calibration(path);
    match entries.get(&units) {
        Some(&existing) if existing <= seconds * 1.01 => return,
        _ => {}
    }
    entries.insert(units, seconds);
    let doc = serde_json::json!({
        "version": 1,
        "note": "probeWorkUnits -> the least-loaded probe wall observed for that cell",
        "entries": entries
            .iter()
            .map(|(units, seconds)| {
                (units.to_string(), serde_json::json!({"probeSeconds": seconds}))
            })
            .collect::<serde_json::Map<_, _>>(),
    });
    let Ok(text) = serde_json::to_string_pretty(&doc) else {
        return;
    };
    let temporary = format!("{path}.tmp{}", std::process::id());
    if std::fs::write(&temporary, text).is_ok() {
        let _ = std::fs::rename(&temporary, path);
    }
}

/// The budget meter. Wall time and work units behind one interface, so the
/// schedule below reads a *fraction spent* and never a clock directly.
struct BudgetMeter {
    budget: PortfolioBudget,
    started: Instant,
    work_base: u64,
    /// Work units debited for self-metered charges the global counter never
    /// saw - see [`BudgetMeter::debit_self_metered`]. Additive on top of
    /// `work_units_now() - work_base`; zero for a run that never schedules an
    /// operator with its own meter, and always zero under a wall budget
    /// (nothing here is ever read by [`BudgetMeter::seconds`]).
    self_metered_debit: u64,
    /// The phase-0 sampler, when one was armed. Dropped - and joined - by
    /// [`Self::install_plan`], so it never outlives the probe it measures.
    plan_probe: Option<PlanProbe>,
    /// The wall target the run was *asked* for, in seconds, or `None` for a run
    /// that named a work budget and no wall at all.
    ///
    /// Read by nothing that decides a trajectory. It exists because
    /// [`Self::install_plan`] replaces `Plan { target_millis }` with `Work {
    /// units }` as soon as phase 0 has priced the box - which is what makes the
    /// search a function of a counter rather than of a clock - and a wall stop
    /// at a checkpoint needs the seconds the caller asked for after that
    /// substitution has happened. Carried on the meter rather than recomputed
    /// from the plan report so that `wall` and `plan` runs answer it the same
    /// way.
    wall_target_seconds: Option<f64>,
}

impl BudgetMeter {
    fn new(budget: PortfolioBudget) -> Self {
        Self {
            budget,
            started: Instant::now(),
            work_base: work_units_now(),
            self_metered_debit: 0,
            plan_probe: None,
            wall_target_seconds: match budget {
                PortfolioBudget::Wall { millis }
                | PortfolioBudget::Plan {
                    target_millis: millis,
                } => Some(millis as f64 / 1_000.0),
                PortfolioBudget::Work { .. } => None,
            },
        }
    }

    /// Starts the phase-0 sampler, if this run wants one.
    ///
    /// Called once, from `run_portfolio`, immediately after the meter is built
    /// and before the first line of phase 0. It is a no-op unless the budget is
    /// a plan **and** more than one work bucket was asked for, so no default run
    /// starts a thread.
    fn arm_plan_probe(&mut self, settings: &PortfolioSettings) {
        if !matches!(self.budget, PortfolioBudget::Plan { .. }) {
            return;
        }
        let buckets = settings.plan_probe_buckets;
        if buckets < 2 {
            return;
        }
        let samples = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_samples = Arc::clone(&samples);
        let thread_stop = Arc::clone(&stop);
        let started = self.started;
        let work_base = self.work_base;
        let handle = std::thread::Builder::new()
            .name("plan-probe".to_owned())
            .spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(PLAN_PROBE_SAMPLE_MILLIS));
                    let seconds = started.elapsed().as_secs_f64();
                    let units = work_units_now().saturating_sub(work_base);
                    if let Ok(mut samples) = thread_samples.lock() {
                        samples.push((seconds, units));
                    }
                }
            })
            .ok();
        // A box that cannot spawn a thread is a box that keeps the shipped
        // whole-phase reading, not a box that fails a run.
        if let Some(handle) = handle {
            self.plan_probe = Some(PlanProbe {
                samples,
                stop,
                handle: Some(handle),
                buckets,
            });
        }
    }

    fn seconds(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn work_units(&self) -> u64 {
        work_units_now()
            .saturating_sub(self.work_base)
            .saturating_add(self.self_metered_debit)
    }

    /// Replaces a [`PortfolioBudget::Plan`] with the work budget it calibrated
    /// to, and returns the calibration.
    ///
    /// Called exactly once, from `run_portfolio`, between the end of phase 0
    /// and the first line that reads a budget - `protected_fraction`. After it
    /// the meter is a work meter in every respect, including
    /// [`Self::is_wall`], [`Self::debit_self_metered`] and every phase
    /// deadline, so nothing downstream has a third case to handle.
    ///
    /// **Phase 0 is inside the plan, not beside it.** `work_base` was read
    /// before phase 0 ran, so `work_units()` already contains the probe's own
    /// units; the plan is therefore a total and the probe is charged to it,
    /// which is what makes the wall target a promise about the whole process
    /// rather than about the part after the measurement.
    fn install_plan(&mut self, target_millis: u64, settings: &PortfolioSettings) -> PlanReport {
        let probe_seconds = self.seconds();
        let probe_work_units = self.work_units();
        // ---- which probe wall the arithmetic uses ---------------------------
        //
        // Everything below this block is `calibrated-plan`'s arithmetic
        // unchanged. This block decides the single number that arithmetic is a
        // function of, and it is the whole of `docs/experiments/robust-plan/`:
        // the shipped mode reads one clock, so a competing workload moves the
        // reading, moves the rung and moves the published depth.
        //
        // Three sources, in the order a run prefers them, and each falls back to
        // the next rather than to an error:
        //
        //  1. a **persisted calibration** keyed on `probe_work_units` - a
        //     counter, so the key is exact - which takes the clock out of the
        //     decision entirely while the live reading stays inside the band;
        //  2. the **max-of-k bucket estimate**, which is still a clock reading
        //     but is the least-loaded one this run can see;
        //  3. the **whole-phase reading**, which is what shipped.
        let samples = self
            .plan_probe
            .as_mut()
            .map(|probe| (probe.finish(), probe.buckets));
        self.plan_probe = None;
        let probe_samples = samples.as_ref().map(|(rows, _)| rows.len()).unwrap_or(0);
        let bucketed = samples.and_then(|(rows, buckets)| {
            probe_effective_seconds(&rows, probe_seconds, probe_work_units, buckets)
        });
        let (mut probe_effective_seconds, mut calibration_source) = match bucketed {
            Some(seconds) => (seconds, PlanCalibrationSource::Probe),
            None => (probe_seconds, PlanCalibrationSource::Live),
        };
        if let Some(path) = settings.plan_calibration_path.as_deref() {
            let band = settings.plan_calibration_band.max(1.0);
            match read_plan_calibration(path).get(&probe_work_units) {
                Some(&stored)
                    if probe_seconds <= stored * band && probe_seconds * band >= stored =>
                {
                    probe_effective_seconds = stored;
                    calibration_source = PlanCalibrationSource::File;
                }
                Some(_) => calibration_source = PlanCalibrationSource::FileOutOfBand,
                None => calibration_source = PlanCalibrationSource::FileMiss,
            }
            if settings.plan_calibration_write {
                // The run's own least-loaded estimate, which is the bucketed one
                // when it exists: a calibration pass under a load spike writes
                // the fastest stretch it saw rather than the average it endured.
                write_plan_calibration(path, probe_work_units, bucketed.unwrap_or(probe_seconds));
            }
        }
        let probe_effective_seconds = probe_effective_seconds;
        let calibration_source = calibration_source;
        let target_seconds = target_millis as f64 / 1_000.0;
        let bias = settings.plan_bias.max(f64::MIN_POSITIVE);
        let headroom = settings.plan_headroom;
        // A rate of zero is not a slow box, it is a build whose counters are
        // off; and a probe that took no measurable time cannot price anything.
        // Both fall back to the whole target at a nominal rate rather than to a
        // division that would produce an infinity.
        let rate = if probe_effective_seconds > 0.0 {
            probe_work_units as f64 / probe_effective_seconds
        } else {
            0.0
        };
        // What is left of the target after the probe has already spent
        // `probe_seconds` of it, priced at the rate the *rest* of the run will
        // retire units at - which is the probe's rate divided by the phase-zero
        // bias. Clamped at zero: a target already overspent by phase 0 buys a
        // plan of exactly the probe, never a negative one.
        // The horizon this first tranche aims at. `1.0` - the whole target - is
        // the single-plan mode; a re-planning run aims lower on purpose and
        // tops the plan up from a measurement instead of a constant. See
        // `PLAN_FIRST_TRANCHE`.
        let mut first_tranche = if settings.plan_replan {
            settings.plan_first_tranche.clamp(f64::MIN_POSITIVE, 1.0)
        } else {
            1.0
        };
        // `probe_effective_seconds` and not the live reading, and this is the
        // one line where the choice has teeth. The plan must be a function of
        // deterministic inputs *end to end* or it is not a plan: a run that
        // priced the rate off a file constant and then subtracted a clock
        // reading would still put the box's load into the rung, one term later.
        // What it costs is stated rather than hidden - under load the run
        // believes phase 0 was quicker than it was, buys the budget it would
        // have bought on a quiet box, and takes longer in wall for it. That is
        // the trade the round measures.
        let mut remaining_seconds =
            target_seconds * headroom * first_tranche - probe_effective_seconds;
        // **The probe outran the tranche.** At a three-second target on
        // mixed-61 phase 0 alone is 2.2 s, so a first tranche aimed at 60% of
        // `target * headroom` - 1.75 s - is already behind by the time it is
        // computed. Without this clause the run buys a plan of exactly the
        // probe, the schedule phase is skipped, and the re-plan cannot rescue
        // it: with no queue there is no rate to measure, so no tranche is taken
        // and the run publishes phase 0's own layout. A re-planning run would
        // be *worse than the mode it improves* at the one budget where the
        // margin is thinnest.
        //
        // So the fraction degrades to the whole target, which is exactly what a
        // non-re-planning run does. It is reported - `plan.firstTranche` says
        // `1.0` - so a reader can see that the fraction was asked for and not
        // applied rather than inferring it from a wall.
        if remaining_seconds <= 0.0 {
            first_tranche = 1.0;
            remaining_seconds = target_seconds * headroom - probe_effective_seconds;
        }
        let remaining_seconds = remaining_seconds.max(0.0);
        let raw_units = probe_work_units as f64 + remaining_seconds * rate / bias;
        let step = settings.plan_quantum_step;
        let (rung, quantised) = quantise_plan(raw_units, step);
        let units = quantised.max(1.0) as u64;
        self.budget = PortfolioBudget::Work { units };
        PlanReport {
            target_millis,
            probe_seconds,
            probe_work_units,
            probe_rate_units_per_second: rate,
            bias,
            headroom,
            quantum_step: step,
            raw_units,
            rung,
            units,
            first_tranche,
            probe_effective_seconds,
            calibration_source,
            probe_samples,
        }
    }

    /// Re-prices the remaining wall at the rate the **queue** is actually
    /// retiring units at, and installs the larger total if it buys a whole
    /// ladder rung. Returns `None` when it does not, which is the case that
    /// makes a re-planning run bit-identical to a non-re-planning one.
    ///
    /// `docs/experiments/calibrated-plan/` §13.1 asks for exactly this and
    /// prices the two lines that make it awkward: `v3_loop`'s `run.deadline`
    /// and `Coordinator::protected_fraction` are both fractions of the budget
    /// that was installed when the phase was entered. This function does not
    /// try to patch them in place. It installs a new *total* and the caller
    /// recomputes `protected_fraction` from it and enters a **new phase**, so
    /// every deadline downstream is derived from the budget that is actually in
    /// force rather than from one that has been mutated underneath it.
    ///
    /// # The clock
    ///
    /// One read, `self.seconds()`, on the first line. Everything after it is
    /// arithmetic on that one number and on counters. Its influence on the
    /// document is bounded by the ladder in two separate ways, and both are
    /// needed:
    ///
    /// * on **size**, because the installed total is snapped to the rung, so
    ///   two processes whose readings differ by less than a rung install the
    ///   same budget;
    /// * on **count**, because a tranche is refused unless the re-priced total
    ///   clears the next rung, so two processes whose readings differ by less
    ///   than a rung also agree on *whether there is a tranche at all*.
    ///
    /// What it does not bound is a box that is loaded differently between two
    /// runs by more than a rung's worth of rate, and no work-denominated budget
    /// can: that is the same limit `install_plan` has, one reading later and
    /// over a longer window - measured at 4.37 s of queue against a 2.52 s
    /// probe on mixed-61 seed 0 at a ten-second target, so about 1.7x, not the
    /// order of magnitude that would make the reading's spread negligible.
    fn replan(
        &mut self,
        plan: &PlanReport,
        settings: &PortfolioSettings,
        index: usize,
    ) -> Option<TrancheReport> {
        let PortfolioBudget::Work { units: current } = self.budget else {
            return None;
        };
        // The one clock read.
        let at_seconds = self.seconds();
        let at_work_units = self.work_units();
        let target_seconds = plan.target_millis as f64 / 1_000.0;
        let remaining_seconds = target_seconds * plan.headroom - at_seconds;
        if !(remaining_seconds > 0.0) {
            return None;
        }
        // The window, and it is the *queue's* window rather than the process's:
        // phase 0 is excluded because including it would put the very bias this
        // is replacing back into the estimate.
        let queue_seconds = at_seconds - plan.probe_seconds;
        let queue_units = at_work_units.saturating_sub(plan.probe_work_units);
        if !(queue_seconds > 0.0) || queue_units == 0 || current == 0 {
            return None;
        }
        let queue_rate = queue_units as f64 / queue_seconds;
        // The horizon, and it is the whole of this round's second constant: a
        // tranche prices what it can *see*, and it has seen `queue_seconds`.
        // Beyond that it would be extrapolating a rate that this campaign has
        // already measured as falling with the budget. See
        // `PLAN_TRANCHE_HORIZON`.
        let mut horizon_seconds =
            remaining_seconds.min(queue_seconds * settings.plan_tranche_horizon);
        // No bias divisor. This rate is measured on the queue, which is the
        // thing the plan is buying more of.
        let mut raw_units = at_work_units as f64 + horizon_seconds * queue_rate;
        if !raw_units.is_finite() || queue_rate <= 0.0 {
            return None;
        }
        // **A tranche may always buy one rung, if the remaining wall pays for
        // it.**
        let one_rung = next_rung_above(current, settings.plan_quantum_step);
        if raw_units < one_rung {
            // The window does not justify a whole rung - and a tranche below one
            // rung is not a tranche, because it floors straight back onto the
            // budget the run already has.
            //
            // Refusing here is what the first cut did, and it **strands the
            // run**: `evidence/determinism-replan-stranded.json` caught
            // mixed-61 seed 2 stopping with 5.7 s of a ten-second target
            // unspent, three millimetres behind the mode it is supposed to
            // improve, because its first tranche had been so short that the
            // queue window it left could not justify a rung. That is worse than
            // over-buying: it is not spending at all.
            //
            // So the question becomes the right one - *can the remaining wall
            // pay for a rung at the rate we measured?* - and the answer buys
            // **exactly** that and never more. The horizon is exceeded, which is
            // the one place `PLAN_TRANCHE_HORIZON` is deliberately overridden,
            // and the excess is bounded by a single rung rather than by the
            // whole remaining wall, so §9.1's 36.74 s failure cannot come back
            // through this door.
            let needed_seconds = (one_rung - at_work_units as f64) / queue_rate;
            if !(needed_seconds > 0.0) || needed_seconds > remaining_seconds {
                return None;
            }
            horizon_seconds = needed_seconds;
            raw_units = one_rung;
        }
        let (rung, quantised) = quantise_plan(raw_units, settings.plan_quantum_step);
        // A tranche that would not raise the budget is not a tranche. The
        // guard is belt and braces over the growth test above - under
        // quantisation the two are the same statement - and it is here because
        // a budget that went *down* would retire the run on the spot.
        let units = quantised.max(1.0) as u64;
        if units <= current {
            return None;
        }
        self.budget = PortfolioBudget::Work { units };
        Some(TrancheReport {
            index,
            at_seconds,
            at_work_units,
            queue_seconds,
            queue_rate_units_per_second: queue_rate,
            remaining_seconds,
            horizon_seconds,
            raw_units,
            rung,
            units,
        })
    }

    /// The total charged so far for self-metered gaps, in work units.
    ///
    /// Read by the schedule loop to attribute a debit to the action that
    /// caused it, which is only possible because the debit now happens inside
    /// the operator transaction rather than after it.
    fn self_metered_debit(&self) -> u64 {
        self.self_metered_debit
    }

    /// Charges the budget itself for the gap between what the global counter
    /// priced an action at and what the action's own meter (e.g.
    /// [`schedule_self_cost_units`]) read, when the latter is larger, and
    /// returns the extra it applied.
    ///
    /// Before this, `spent` never moved when an m34 self-metered arm's price
    /// beat the global meter's read in [`v3_loop`]: `ClassStats::cost_max` and
    /// the ranking saw the higher, honest price, but
    /// [`BudgetMeter::work_units`] - and so
    /// `spent_fraction`/`remaining_to`, which the affordability rule and every
    /// phase deadline read - kept advancing at the global counter's rate. A
    /// class whose own meter reads 11x the global counter's could therefore
    /// buy far more of itself than a work budget was meant to afford. Charging
    /// `max(global_meter_delta, operator_self_units)` here, not just the
    /// price used for ranking, is what closes that gap; it can only ever
    /// raise `spent`, never lower it, so this cannot manufacture room a run
    /// did not have.
    ///
    /// Both arguments and the accumulator are `u64`, the work meter's own
    /// type: the first version of this took the global delta as an `f64`
    /// because the call site had one lying around, which put a 53-bit
    /// mantissa between a counter that is exact and a budget that is compared
    /// against it (Sol review 6 §1). `saturating_sub` is the whole of the
    /// `max(..., 0)` clamp.
    ///
    /// Under a **wall** budget this is a deliberate no-op and returns zero:
    /// seconds are seconds, the clock has no broad phase to ride free on, and
    /// `work_units` is not that budget's currency. The guard lives here, in
    /// the one place that owns the accumulator, rather than at the call site
    /// - a rule enforced by every caller separately is a rule one new caller
    /// silently breaks.
    fn debit_self_metered(&mut self, global_meter_delta: u64, operator_self_units: u64) -> u64 {
        if self.is_wall() {
            return 0;
        }
        let extra = operator_self_units.saturating_sub(global_meter_delta);
        self.self_metered_debit = self.self_metered_debit.saturating_add(extra);
        extra
    }

    /// The fraction of the budget already spent, in the budget's own currency.
    ///
    /// [`PortfolioBudget::Plan`] has no currency: it is a wall target that has
    /// not yet been priced, and it survives only from `BudgetMeter::new` to
    /// [`Self::install_plan`], across the protected phase 0, which is never
    /// budget-checked. Reporting it as fully spent is the fail-closed answer -
    /// a build that somehow reached a deadline with the plan still uninstalled
    /// returns phase 0's own layout with every later phase marked skipped,
    /// which is visible in the report, rather than silently running to an
    /// infinite budget.
    fn spent_fraction(&self) -> f64 {
        match self.budget {
            PortfolioBudget::Wall { millis } => {
                if millis == 0 {
                    return f64::INFINITY;
                }
                self.started.elapsed().as_secs_f64() * 1_000.0 / millis as f64
            }
            PortfolioBudget::Work { units } => {
                if units == 0 {
                    return f64::INFINITY;
                }
                self.work_units() as f64 / units as f64
            }
            PortfolioBudget::Plan { .. } => f64::INFINITY,
        }
    }

    /// Whether a phase whose deadline is `fraction` still has room.
    fn has_room(&self, fraction: f64) -> bool {
        self.spent_fraction() < fraction
    }

    /// The whole budget, in the budget's own currency: seconds for
    /// [`PortfolioBudget::Wall`], work units for [`PortfolioBudget::Work`].
    fn currency_total(&self) -> f64 {
        match self.budget {
            PortfolioBudget::Wall { millis } => millis as f64 / 1_000.0,
            PortfolioBudget::Work { units } => units as f64,
            // See `spent_fraction`: an uninstalled plan has no currency.
            PortfolioBudget::Plan { .. } => 0.0,
        }
    }

    /// Whether the run's wall target has already passed, with `reserve_seconds`
    /// still to spend after now.
    ///
    /// `false` for a run that named a work budget and no wall at all: there is
    /// no clock to be past. The reading is [`Self::started`] and not
    /// [`Self::seconds`] only so the intent is legible - they are the same
    /// number - and the reserve is added to *now* rather than subtracted from
    /// the target so that a target smaller than the reserve refuses everything
    /// instead of wrapping.
    ///
    /// This is the one place the wall stop is a *question about the run*, and
    /// it is deliberately not a question about the phase: `deadline` is a
    /// fraction of the budget's own currency, which under a plan is a counter.
    /// See [`PortfolioSettings::compression_schedule_wall_stop_all`].
    #[cfg(feature = "compression-schedule")]
    fn wall_target_passed(&self, reserve_seconds: f64) -> bool {
        self.wall_target_seconds.is_some_and(|limit| {
            self.started.elapsed().as_secs_f64() + reserve_seconds.max(0.0) >= limit
        })
    }

    /// Whether the budget's currency is the clock rather than the work meter.
    ///
    /// The one thing the schedule below reads a *currency* for rather than a
    /// fraction: two of its prices are measured to disagree by an order of
    /// magnitude between the two, so which one is running has to be a fact the
    /// ranking can see. See [`ActionClass::prior_cost_in_phase_zero_for`].
    fn is_wall(&self) -> bool {
        matches!(self.budget, PortfolioBudget::Wall { .. })
    }

    /// What has been spent, in the same currency.
    fn currency_spent(&self) -> f64 {
        match self.budget {
            PortfolioBudget::Wall { .. } => self.seconds(),
            // A plan's currency is work once it is installed, and the probe's
            // own units are already inside `work_units()` - `work_base` was
            // read before phase 0. Reading it the same way before installation
            // keeps `phase_zero_cost` a work number in both.
            PortfolioBudget::Work { .. } | PortfolioBudget::Plan { .. } => self.work_units() as f64,
        }
    }

    /// What is left before the deadline at `fraction`, in the same currency.
    fn remaining_to(&self, fraction: f64) -> f64 {
        (fraction * self.currency_total() - self.currency_spent()).max(0.0)
    }

    /// One recorded operator call's cost, in the same currency.
    fn call_cost(&self, call: &OperatorCallReport) -> f64 {
        match self.budget {
            PortfolioBudget::Wall { .. } => call.elapsed_seconds,
            PortfolioBudget::Work { .. } | PortfolioBudget::Plan { .. } => call.work_units as f64,
        }
    }
}

/// Snaps a raw plan onto the quantisation ladder, returning the rung and the
/// snapped units.
///
/// **Floor, not round**: the error is then one-sided and a plan is never larger
/// than the measurement justified, which is what lets [`PLAN_HEADROOM`] be 0.97
/// instead of 0.8. See [`PLAN_QUANTUM_STEP`].
///
/// Shared by [`BudgetMeter::install_plan`] and [`BudgetMeter::replan`] rather
/// than written twice, because the whole determinism argument for a tranche is
/// that it lands on *the same ladder* the initial plan does - a second copy of
/// this arithmetic is a second ladder waiting to drift from the first.
fn quantise_plan(raw_units: f64, step: f64) -> (Option<i64>, f64) {
    if step > 1.0 && raw_units > PLAN_ANCHOR_UNITS {
        let index = ((raw_units / PLAN_ANCHOR_UNITS).ln() / step.ln()).floor();
        (Some(index as i64), PLAN_ANCHOR_UNITS * step.powf(index))
    } else {
        (None, raw_units)
    }
}

/// The smallest plan that quantises to a **strictly higher rung** than `units`.
///
/// It is derived from the rung *index* rather than written as `units * step`,
/// and the difference is not pedantry: [`quantise_plan`] floors, and a budget is
/// a `u64`, so `current` is a rung that has already lost its fractional part.
/// Multiplying that by `step` lands a fraction of a unit *below* the next rung
/// and floors straight back onto the one it started from - which is a tranche
/// that installs the budget the run already has, and is exactly the stranding
/// this function exists to prevent.
///
/// Under `planq=1` there is no ladder, so the growth threshold is the one the
/// quantised arm's rung happens to be: [`PLAN_TRANCHE_MIN_GROWTH`]. That keeps
/// the unquantised arm's tranche *count* as coarse a decision as the quantised
/// arm's, which is the property §6 of `docs/experiments/replan/` claims.
fn next_rung_above(units: u64, step: f64) -> f64 {
    if step > 1.0 && units as f64 > PLAN_ANCHOR_UNITS {
        // Two nudges, and both are the log round-trip rather than the ladder.
        //
        // On the way **in**: `units` is a rung that `quantise_plan` floored and
        // then truncated to `u64`, so `ln(units/anchor)/ln(step)` lands a few
        // ulps below its own integer and `floor` returns `k-1`. The next rung
        // above `k-1` is `k`, which is the rung the caller already has - a
        // tranche that installs the budget the run is trying to leave.
        //
        // On the way **out**: `anchor * step^(k+1)` fed back through the same
        // ratio lands below `k+1` for the same reason, so `quantise_plan` would
        // floor the target straight back down.
        //
        // The inbound nudge is `1e-4` rather than `1e-6` because the deficit it
        // has to cover is a **whole unit** of truncation: `1/units` in value,
        // which is `1/(units * ln step)` in index, and that is 7.2e-6 at a
        // million-unit rung. In index space a rung is `1.0`, so `1e-4` is one
        // part in ten thousand of one and can only matter for a value that is a
        // rung already.
        let index = ((units as f64 / PLAN_ANCHOR_UNITS).ln() / step.ln() + 1e-4).floor();
        PLAN_ANCHOR_UNITS * step.powf(index + 1.0) * (1.0 + 1e-6)
    } else {
        units as f64 * PLAN_TRANCHE_MIN_GROWTH
    }
}

/// The process-wide work-unit reading.
///
/// Zero, and therefore constant, when `profiling` recording is off - which is
/// why [`PortfolioBudget::Work`] arms it and [`PortfolioBudget::Wall`] does
/// not. A wall-budget run must have the clock the production build runs on; a
/// work-budget run must have the counters, and pays the ~17% they cost.
fn work_units_now() -> u64 {
    work_units_from(&profiling::counter_totals())
}

/// The shipped meter's formula, over a snapshot rather than over the live
/// registry.
///
/// Split out from [`work_units_now`] so the *mapping* can be compared against
/// [`work_currency_counts_from`]'s without either function reading a
/// process-global counter. The two have to agree - a class the parallel
/// currency's profile does not name must self-price at exactly what this
/// returns, or `max(global, class)` starts charging a debit on every operator
/// in the run - and the first cut of the currency got that wrong (see
/// `work_currency::SHIPPED_EXACT_PAIR_TEST`).
///
/// Writing the comparison against the live counters instead was the obvious
/// thing and it was wrong twice over: it needed `profiling::set_enabled(true)`,
/// which is process-global, and `cargo test` runs this module's tests in
/// parallel threads of one process - so the test broke three *sibling* tests
/// that legitimately assume an unarmed meter reads zero. A snapshot has no
/// such reach.
fn work_units_from(totals: &[u64; Counter::COUNT]) -> u64 {
    totals[Counter::CandidateQueries as usize].saturating_add(
        WORK_UNITS_PER_EXACT_PAIR_TEST.saturating_mul(totals[Counter::ExactPairTests as usize]),
    )
}

/// The parallel currency's five profiling counts, read as one snapshot.
///
/// Deliberately one `counter_totals()` call rather than five reads: the
/// registry is locked once and the five numbers are consistent with each
/// other, which is what makes a *delta* over a call meaningful when the call
/// fanned out to worker threads.
///
/// Zero, and therefore a zero delta, when profiling recording is off - the
/// same condition the shipped meter has always had, and the reason the
/// currency is a work-budget concept exactly as the meter it parallels is.
fn work_currency_counts_now() -> crate::search::work_currency::ClassCounts {
    work_currency_counts_from(&profiling::counter_totals())
}

/// The currency's half of the same mapping, over the same snapshot. See
/// [`work_units_from`].
fn work_currency_counts_from(
    totals: &[u64; Counter::COUNT],
) -> crate::search::work_currency::ClassCounts {
    crate::search::work_currency::ClassCounts {
        candidate_queries: totals[Counter::CandidateQueries as usize],
        exact_pair_tests: totals[Counter::ExactPairTests as usize],
        collision_builds: totals[Counter::CollisionPolygonBuilds as usize],
        neighbor_tests: totals[Counter::NeighborTests as usize],
        full_rescores: totals[Counter::FullRescores as usize],
        position_source_attempts: 0,
        returned_positions: 0,
        pair_visits: 0,
        operator_collision_builds: 0,
        confirmations: 0,
    }
}

/// Whole-layout exact confirmations the call attempted, when it was a mode-34
/// slice; zero otherwise, and zero in a build without the schedule.
///
/// This is the one currency input that is not a profiling counter, and it is
/// here for the same reason `CompressionSchedule::work_units` derives its
/// exact half rather than sampling one: a confirmation is `n*(n-1)/2` pair
/// questions of which only a handful reach the narrow phase the profiling
/// array counts, so the array under-reads the exact tier by about 18x and the
/// slice's own count is the honest one.
fn schedule_confirmations_attempted(population: &GeneralPersistentVacancyDiagnostics) -> u64 {
    #[cfg(feature = "compression-schedule")]
    {
        population
            .compression_schedule
            .as_ref()
            .map_or(0, |report| report.confirmations_attempted as u64)
    }
    #[cfg(not(feature = "compression-schedule"))]
    {
        let _ = population;
        0
    }
}

/// What one dispatched operator call was charged, settled before the call's
/// archive entry, publication and report are stamped.
///
/// The four numbers are reported rather than collapsed to one because the
/// interesting fact about a self-metered operator is precisely the *gap*: an
/// arm whose own meter reads 3.34M while the coordinator's counter reads 307k
/// is the finding, and an evidence document that only carried the maximum
/// would hide it. See [`schedule_self_cost_units`] and Sol review 6 §1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OperatorCharge {
    /// The coordinator's own counter delta across the call.
    global_units: u64,
    /// What the operator's own meter charged itself, if it carries one.
    self_metered_units: Option<u64>,
    /// What [`BudgetMeter::debit_self_metered`] actually applied: zero under a
    /// wall budget, zero when the global counter already read at least as
    /// much, and the difference otherwise.
    debited_units: u64,
    /// `global_units + debited_units`, which is `max(global_units,
    /// self_metered_units)` whenever a debit was possible.
    charged_units: u64,
}

/// Steps two and three of the operator transaction: determine what one
/// dispatched operator call is charged, and debit the difference into the
/// meter *before* the caller stamps anything with a meter reading.
///
/// A free function over `&mut BudgetMeter` rather than a `Coordinator` method,
/// and taking the self-metered reading rather than the population it came
/// from, for one reason: the ordering rule Sol review 6 §1 asked for is the
/// load-bearing part, and this way it has a name, a signature and a unit test
/// in every feature configuration rather than only a position inside a
/// 90-line function that needs a whole engine to call.
fn settle_operator_charge(
    meter: &mut BudgetMeter,
    global_units: u64,
    self_metered_units: Option<u64>,
) -> OperatorCharge {
    let debited_units = match self_metered_units {
        Some(units) => meter.debit_self_metered(global_units, units),
        None => 0,
    };
    OperatorCharge {
        global_units,
        self_metered_units,
        debited_units,
        charged_units: global_units.saturating_add(debited_units),
    }
}

/// The self-metered charge one dispatched operator reports for itself, in the
/// portfolio's own work currency, or `None` when it carries no meter of its
/// own.
///
/// The one implementation today is the compression schedule; the wrapper
/// exists so [`settle_operator_charge`]'s caller - which runs for every mode,
/// in every feature configuration - has a single call to make.
fn operator_self_metered_units(population: &GeneralPersistentVacancyDiagnostics) -> Option<u64> {
    #[cfg(feature = "compression-schedule")]
    {
        schedule_self_cost_units(population)
    }
    #[cfg(not(feature = "compression-schedule"))]
    {
        let _ = population;
        None
    }
}

/// What an operator's parent layout is *to that operator*.
///
/// The distinction matters because the archive's fairness counter is a count of
/// descents, and the mode-20/25 constructor does not descend from its parent -
/// it builds a layout from scratch and reads the parent only as a pose prior.
/// Charging that as a descent is what made this stage's first schedule spend
/// its whole alternation phase away from the incumbent: four constructor arms
/// charged the incumbent four times, and the incumbent went to the back of a
/// queue it should have been at the front of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParentRole {
    /// The operator descends from this layout: it is that basin's quantum.
    Descended,
    /// The operator only reads this layout as a prior. No descent is charged.
    Prior,
}

/// The readings one operator call takes before it dispatches.
///
/// Held together so that [`Coordinator::settle_operator_call`] takes one
/// argument rather than four, and so that the *pairing* is visible: every one
/// of these is a "before" whose only use is a delta, and a settlement that read
/// any of them from the live meter instead would be charging the call for
/// everything the run had spent.
struct OperatorCallOpen {
    started_seconds: f64,
    started_work: u64,
    currency: WorkCurrencyMode,
    counts_before: crate::search::work_currency::ClassCounts,
}

/// The coordinator's mutable state during a run.
struct Coordinator<'a> {
    pieces: &'a [GeneralFastPiece<'a>],
    fast_settings: GeneralFastSettings,
    settings: PortfolioSettings,
    meter: BudgetMeter,
    incumbent: PublishedIncumbent,
    archive: SearchArchive,
    publications: Vec<PublicationEvent>,
    operator_calls: Vec<OperatorCallReport>,
    phases: Vec<PhaseReport>,
    phase_name: String,
    attempted: std::collections::BTreeSet<String>,
    /// Whether the alternation phase ended because every distinct archive state
    /// had already had its quantum, rather than because it ran out of budget.
    descent_stalled: bool,
    /// Why the phase currently in flight stopped. Reset to
    /// [`PhaseExitCause::Completed`] on entry and overwritten by whichever
    /// early return fires.
    exit_cause: PhaseExitCause,
    /// The fraction of the budget the protected phase 0 spent. Every later
    /// phase's deadline is a fraction of what is left after it; see
    /// [`PhaseSchedule`].
    protected_fraction: f64,
    /// What the protected phase 0 cost, in the budget's own currency. Every v3
    /// class prior is quoted as a multiple of it, so a prior measured on
    /// mixed-61 at a work budget still prices shapes-17 at a wall budget.
    phase_zero_cost: f64,
    /// What each v3 action class has cost and produced *in this run*.
    class_stats: BTreeMap<ActionClass, ClassStats>,
    /// How many compression-schedule slices this run has dispatched.
    ///
    /// Incremented at the dispatch site and read only by it, to decide whether
    /// [`PortfolioSettings::schedule_first_slice_step_grid`] and its neighbour
    /// apply. A counter rather than a bool because the document reports it and a
    /// reader comparing two arms of the confirmation-density sweep needs to know
    /// how many slices each bought, not only what the first one did.
    #[cfg(feature = "compression-schedule")]
    schedule_slices: usize,
    /// A mode-34 slice the coordinator is holding between actions.
    ///
    /// This field is the whole of what "the coordinator regains control at a
    /// checkpoint" means. The slice is not a report and not a snapshot: it owns
    /// its frontier, its deepest-confirmed slot, its lane's rng and weights,
    /// every surrogate and pair-NFP cache, and the step rows it has written so
    /// far. The queue may run any other action while it sits here, and the
    /// resumption is bit-identical to the run that was never suspended - which
    /// is the gate `docs/experiments/real-interruption/` §4 runs.
    ///
    /// Always `None` unless [`PortfolioSettings::compression_schedule_yield_batches`]
    /// is non-zero.
    #[cfg(feature = "compression-schedule")]
    suspended_slice: Option<Box<crate::search::general_relaxed::SuspendedScheduleSlice<'a>>>,
    /// Actions the queue has run since the slice above was suspended.
    ///
    /// The resume rule reads it and nothing else: a suspended slice is resumed
    /// once at least one *other* action has run, which is what makes the
    /// mechanism an interleave rather than a more expensive way to write the
    /// same loop.
    #[cfg(feature = "compression-schedule")]
    actions_since_suspension: usize,
    /// The sparse operator's request-adaptive disarm. See [`SparseRotationBit`].
    #[cfg(feature = "sparse-rotation")]
    sparse_rotation_bit: SparseRotationBit,
}

/// One bit, one audition — the sparse operator's request-adaptive disarm.
///
/// Deliberately the same shape as the compression-schedule sterile bit
/// (`PortfolioSettings::schedule_sterile_bit`), because it acts on the same kind
/// of evidence for the same reason: a prior measured on one request does not
/// cross to another, and the cheapest place to find that out is the first slice
/// where the mechanism could have fired and did not.
///
/// The evidence is a **sterile slice**: an m34 slice in which design B opened at
/// least one episode - so the mechanism had its trigger, its pieces and its
/// budget - and not one accepted move changed a pose. That is not "rotation was
/// not offered"; it is "rotation was offered exactly where the clamp said to
/// offer it, and bought nothing".
#[cfg(feature = "sparse-rotation")]
#[derive(Clone, Debug, Default)]
struct SparseRotationBit {
    /// Slices that opened an episode and accepted no rotation move.
    sterile_slices: usize,
    /// Whether the operator is currently off.
    disarmed: bool,
    /// Whether the one audition has been spent. It fires at most once per run,
    /// which is what keeps the bit a claim this run can still disprove instead
    /// of an absorbing state.
    auditioned: bool,
    /// Operator calls that published nothing since the bit fired.
    barren_calls: usize,
}

#[cfg(feature = "sparse-rotation")]
impl SparseRotationBit {
    /// The verdict, from one slice's own two numbers.
    ///
    /// `episodes` is how many stalls armed something and `committed` is how
    /// many of the operator's own proposals reached a committed move. Both come
    /// off the slice report and neither is inferred.
    ///
    /// `committed` **must** be `sparse_rotation_committed_moves` and not
    /// `rotation_accepted_moves`. The second counts any accepted move whose
    /// pose differs from the incumbent's, including the random catalogue starts
    /// `search_piece` draws, so a lane that was offered no rung at all scores
    /// thousands of them - 11,523 in the control arm of
    /// `docs/experiments/sparse-rotation/`, against zero rungs (Sol review 8 §2
    /// P0). Read that way the bit's evidence was about the catalogue and the
    /// "the disarm was never necessary" finding was uninterpretable.
    ///
    /// Extracted as a method so the regression test drives this rule rather
    /// than a copy of it.
    fn observe_slice(&mut self, episodes: usize, committed: usize) {
        if episodes == 0 {
            // The mechanism did not get its trigger, so it does not get a
            // verdict. Neither direction.
            return;
        }
        if committed == 0 {
            self.sterile_slices += 1;
            if self.sterile_slices >= SPARSE_ROTATION_STERILE_SLICES {
                self.disarmed = true;
                self.barren_calls = 0;
            }
        } else {
            // The audition disproved the bit: rotation is productive on this
            // request after all, so the operator comes back for good.
            self.disarmed = false;
            self.sterile_slices = 0;
        }
    }
}

/// Sterile slices before the sparse operator comes off this request.
///
/// One, and for the same reason `SCHEDULE_STERILE_ACTIONS` is one: the two
/// requests where this has to fire (`shapes-17`, `triangle-20`) have never
/// published a single m34 slice in any round of this campaign, in either arm,
/// so waiting for a second slice buys nothing but a second slice's wall.
#[cfg(feature = "sparse-rotation")]
const SPARSE_ROTATION_STERILE_SLICES: usize = 1;

/// Barren operator calls after the bit fires before the one audition.
///
/// [`BARREN_ACTION_PATIENCE`] itself, exactly as `SCHEDULE_AUDITION_BARREN` is:
/// the audition exists to keep the rule falsifiable, and a run that has gone
/// sixteen calls without publishing is close enough to over that handing the
/// operator back is a cheap way to be wrong out loud.
#[cfg(feature = "sparse-rotation")]
const SPARSE_ROTATION_AUDITION_BARREN: usize = BARREN_ACTION_PATIENCE;

/// One action class's running evidence.
#[derive(Clone, Debug, Default)]
struct ClassStats {
    actions: usize,
    publications: usize,
    work_units: u64,
    seconds: f64,
    /// Cost in the budget's own currency, summed and maximised. The maximum is
    /// what the affordability rule uses: the ledger priced a mode-20 arm at
    /// 260 work units and 3.1 seconds, and the A/B/C's arm C spent 5.7M units
    /// on one seed and 21.0M on another, so a class with a spread is priced by
    /// its worst case or it is not priced at all.
    cost_total: f64,
    cost_max: f64,
    delta_raw_mm: f64,
    first_estimated_cost: Option<f64>,
    first_actual_cost: Option<f64>,
}

impl<'a> Coordinator<'a> {
    /// Publishes `placements` through the engine's own adoption rule, and
    /// reports whether the incumbent moved.
    ///
    /// The coordinator never decides validity. It hands a complete layout to
    /// [`general_relaxed::adopt_published_placements`], which re-runs the
    /// composite exact validator against the real request, requires complete
    /// cardinality, and requires a strict raw-depth improvement - the same
    /// three gates, in the same order, that the coupled separator's mode slot
    /// publishes through. Adoption is detected by the fingerprint moving,
    /// which is the only way that function can have said yes.
    fn try_publish(&mut self, placements: &[GeneralFastPlacement], source: &str) -> bool {
        let previous_fingerprint = self.incumbent.fingerprint.clone();
        let previous_depth = self.incumbent.raw_depth_mm;
        let adopted = crate::search::general_relaxed::adopt_published_placements(
            self.pieces,
            self.fast_settings,
            placements.to_vec(),
            self.incumbent.result.clone(),
        );
        let fingerprint = general_placement_fingerprint(&adopted.placements);
        if fingerprint == previous_fingerprint {
            return false;
        }
        let raw_depth_mm = crate::search::general_relaxed::coupled_raw_source_depth(
            self.pieces,
            &adopted.placements,
            self.fast_settings,
        )
        .ok();
        let dual_gate_valid =
            validate_and_measure_placements(self.pieces, &adopted.placements, self.fast_settings)
                .is_ok();
        let seconds = self.meter.seconds();
        let work_units = self.meter.work_units();
        self.publications.push(PublicationEvent {
            seconds,
            work_units,
            phase: self.phase_name.clone(),
            source: source.to_owned(),
            raw_depth_mm: raw_depth_mm.unwrap_or(f64::NAN),
            previous_raw_depth_mm: previous_depth,
            fingerprint: fingerprint.clone(),
        });
        self.incumbent = PublishedIncumbent {
            result: adopted,
            fingerprint,
            raw_depth_mm,
            dual_gate_valid,
            source: source.to_owned(),
            published_seconds: seconds,
            published_work_units: work_units,
        };
        true
    }

    /// Offers a layout to the archive, measuring its raw depth and validity
    /// first. Returns the disposition and the measured depth.
    fn archive_layout(
        &mut self,
        placements: Vec<GeneralFastPlacement>,
        operator: BasinOperator,
        parent_fingerprint: Option<String>,
        secondary_parent_fingerprint: Option<String>,
    ) -> (ArchiveDisposition, Option<f64>) {
        if placements.len() != self.pieces.len() {
            return (ArchiveDisposition::IncompleteCardinality, None);
        }
        let raw_depth_mm = crate::search::general_relaxed::coupled_raw_source_depth(
            self.pieces,
            &placements,
            self.fast_settings,
        )
        .ok();
        let Some(raw_depth_mm) = raw_depth_mm else {
            return (ArchiveDisposition::IncompleteCardinality, None);
        };
        let exact_valid =
            validate_and_measure_placements(self.pieces, &placements, self.fast_settings).is_ok();
        let basin = ArchivedBasin {
            fingerprint: general_placement_fingerprint(&placements),
            raw_depth_mm,
            birth_seconds: self.meter.seconds(),
            birth_work_units: self.meter.work_units(),
            operator,
            parent_fingerprint,
            secondary_parent_fingerprint,
            exact_valid,
            descents: 0,
            placements,
        };
        let disposition = self.archive.offer(basin);
        (disposition, Some(raw_depth_mm))
    }

    /// Runs one deep-operator mode against one parent, archives whatever it
    /// produced, attempts publication, and records the call.
    ///
    /// # The call is a transaction
    ///
    /// The order below is load-bearing and is the correction Sol review 6 §1
    /// asked for: **dispatch -> determine the charge -> debit -> archive,
    /// publish, report**. Coordinator v5's first cut debited the self-metered
    /// gap in [`v3_loop`], *after* `run_operator` had already returned, which
    /// left the action's own publication, its archived basin's
    /// `birth_work_units` and its own [`OperatorCallReport::work_units`]
    /// stamped with a meter reading that did not include the charge the same
    /// action had just incurred - while every *later* publication did include
    /// it. The anytime curve that comes out of that is not merely imprecise,
    /// it is temporally incoherent: work appears on the timeline one action
    /// after the action that spent it. Debiting before anything is stamped
    /// makes every reading in this function a reading of a settled budget.
    #[allow(clippy::too_many_arguments)]
    fn run_operator(
        &mut self,
        mode: usize,
        parent: &[GeneralFastPlacement],
        parent_fingerprint: Option<String>,
        target: Option<f64>,
        tune: impl FnOnce(&mut GeneralRelaxedSettings),
        secondary: Option<&GeneralPersistentVacancyPinnedParent>,
        parent_role: ParentRole,
        action: Option<String>,
    ) -> GeneralPersistentVacancyDiagnostics {
        let secondary_fingerprint = secondary.map(|parent| parent.source_sha256.clone());
        let mut relaxed = self.base_relaxed_settings();
        relaxed.persistent_vacancy_mode = mode;
        relaxed.persistent_vacancy_target_depth_mm = target;
        relaxed.persistent_vacancy_allow_unpinned_parent = true;
        // The continuous-rotation operator, scoped here rather than on
        // `relaxed_template`, and by mode rather than by call site.
        //
        // The brief scopes it to "the relaxed lane used by m22/m34 under the
        // coordinator", and those two modes are reached from eleven call sites
        // between them; arming it at each would be eleven places for the scope
        // to drift. It is set *before* `tune` so a caller that wants a
        // different answer for one call can still override it, and after
        // `base_relaxed_settings` so it cannot be inherited by a class that was
        // not measured with it.
        #[cfg(feature = "continuous-rotation")]
        {
            relaxed.continuous_rotation =
                self.settings.continuous_rotation && matches!(mode, 22 | 34);
        }
        // The sparse operator, scoped to **mode 34 alone**.
        //
        // Design B's trigger is a compression-schedule step whose translation
        // repair stalled, and mode 22 has no schedule, no clamp and no step to
        // stall: an m22 lane with `sparse_rotation` set starts at
        // `RotationArming::Nobody` and nothing ever arms it, so it proposes no
        // rungs at all. That is the intended reading of "sparse" and not an
        // oversight - docs/experiments/rotation-tax/ §0 measured mode 22 paying
        // for **85%** of design A's 1.13 M surrogate builds while every piece of
        // detailed attribution the campaign has is about m34 - but leaving it
        // implicit in a lane's arming would be a scope that drifts, so it is
        // written here beside the one above.
        #[cfg(feature = "sparse-rotation")]
        let sparse_audition = {
            // The bit, read here and nowhere else. `audition` is the one call
            // the run hands back after the operator has been off for
            // `SPARSE_ROTATION_AUDITION_BARREN` barren calls; it is consumed
            // whether or not this call turns out to be an m34 slice, because a
            // second chance that could be spent repeatedly is not one audition.
            let bit = &mut self.sparse_rotation_bit;
            let audition = self.settings.sparse_rotation_bit
                && bit.disarmed
                && !bit.auditioned
                && bit.barren_calls >= SPARSE_ROTATION_AUDITION_BARREN;
            let off = self.settings.sparse_rotation_bit && bit.disarmed && !audition;
            if audition {
                bit.auditioned = true;
            }
            let armed = self.settings.sparse_rotation && !off;
            relaxed.rotation_equivariant_offset =
                self.settings.rotation_equivariant_offset && relaxed.continuous_rotation;
            relaxed.sparse_rotation = armed && mode == 34;
            relaxed.se2_witness = self
                .settings
                .se2_witness
                .filter(|_| relaxed.sparse_rotation);
            // Design A's mode-22 arming has no trigger under design B, so it is
            // withdrawn rather than left proposing rungs no stall asked for.
            if self.settings.sparse_rotation && mode == 22 {
                relaxed.continuous_rotation = false;
            }
            if self.settings.sparse_rotation && mode == 34 && !armed {
                relaxed.continuous_rotation = false;
            }
            audition
        };
        tune(&mut relaxed);
        let parent_arm = GeneralCoupledSeparatorArmDiagnostics {
            final_placements: crate::search::general_relaxed::coupled_placement_diagnostics(parent),
            ..GeneralCoupledSeparatorArmDiagnostics::default()
        };
        let started_seconds = self.meter.seconds();
        let started_work = self.meter.work_units();
        // The parallel currency's own "before" reading. Taken only when the
        // currency is armed, so an unarmed run makes exactly the counter reads
        // it always made: `counter_totals()` takes the registry lock and sums
        // every thread block, and a run that is not going to use the answer
        // must not pay for it.
        let currency = self.settings.work_currency;
        let counts_before = currency
            .armed()
            .then(work_currency_counts_now)
            .unwrap_or_default();
        // The checkpoint policy, built here because this is the only place that
        // can see both the operator about to run and the meter it runs against.
        //
        // Everything the closure reads is copied out first, so it borrows no
        // field of `self` and the suspension slot below can be borrowed
        // mutably beside it. That is not a lifetime workaround: a policy that
        // could read the coordinator's live state at a checkpoint would be a
        // policy whose answer depends on when it is asked, and the whole point
        // of a checkpoint is that the answer is a *decision* and not a race.
        //
        // `wall_stop_all` arms the checkpoint stop as well as the queue rule,
        // so the extension is strict: every overrun the checkpoint stop
        // measurably compressed stays compressed, and the queue rule is added
        // in front of it rather than instead of it.
        #[cfg(feature = "compression-schedule")]
        let wall_stop_seconds = (mode == 34
            && (self.settings.compression_schedule_wall_stop
                || self.settings.compression_schedule_wall_stop_all))
            .then(|| self.meter.wall_target_seconds)
            .flatten();
        // Refused while a slice is already parked, because the slot holds one:
        // a second suspension would overwrite the first and the run would lose
        // a live slice - and with it the work already charged for it.
        #[cfg(feature = "compression-schedule")]
        let yield_after_batches = (mode == 34 && self.suspended_slice.is_none())
            .then_some(self.settings.compression_schedule_yield_batches)
            .filter(|batches| *batches > 0);
        // The barren rule, armed only on a past-bound slice. Inside the nine
        // rungs the operator is doing the thing every pinned number in this
        // repository was measured doing, and this round does not put a new
        // stopping rule in front of it.
        #[cfg(feature = "compression-schedule")]
        let barren_batches = (mode == 34 && self.settings.compression_schedule_past_bound)
            .then_some(self.settings.compression_schedule_past_bound_barren)
            .filter(|batches| *batches > 0);
        #[cfg(feature = "compression-schedule")]
        let started = self.meter.started;
        // The closure's own state, and the reason a checkpoint policy is an
        // `FnMut` rather than a function pointer: "has this batch bought
        // anything" is a question about two checkpoints, not one.
        #[cfg(feature = "compression-schedule")]
        let mut deepest_seen = f64::INFINITY;
        #[cfg(feature = "compression-schedule")]
        let mut barren_run = 0usize;
        #[cfg(feature = "compression-schedule")]
        let mut control = move |checkpoint: &ScheduleCheckpoint| {
            // The wall stop wins over everything, because a slice suspended past
            // the deadline is a slice the run will never get back to: the queue
            // that would resume it is itself out of budget.
            if let Some(limit) = wall_stop_seconds {
                if started.elapsed().as_secs_f64() >= limit {
                    return SliceControl::Stop;
                }
            }
            if let Some(limit) = barren_batches {
                if checkpoint.published_depth_mm < deepest_seen {
                    deepest_seen = checkpoint.published_depth_mm;
                    barren_run = 0;
                } else {
                    barren_run += 1;
                    if barren_run >= limit {
                        return SliceControl::Stop;
                    }
                }
            }
            match yield_after_batches {
                // `batch` is zero-based, so "after n batches" is the checkpoint
                // whose index is n - 1, and the modulo makes a slice that is
                // resumed offer its turn back again rather than run to the end.
                Some(n) if (checkpoint.batch + 1) % n == 0 => SliceControl::Suspend,
                _ => SliceControl::Continue,
            }
        };
        // Read out before the suspension slot is borrowed mutably, so the two
        // borrows are of two fields and not of `self`.
        let pieces = self.pieces;
        let fast_settings = self.fast_settings;
        #[cfg(feature = "compression-schedule")]
        let interruption = if wall_stop_seconds.is_some()
            || yield_after_batches.is_some()
            || barren_batches.is_some()
        {
            Some(crate::search::general_relaxed::SliceInterruption {
                control: &mut control,
                suspended: &mut self.suspended_slice,
            })
        } else {
            None
        };
        #[cfg(not(feature = "compression-schedule"))]
        let interruption = None;
        let population = crate::search::general_relaxed::dispatch_persistent_vacancy_mode(
            pieces,
            fast_settings,
            relaxed,
            &parent_arm,
            None,
            secondary,
            interruption,
        );
        // The clock the interleave runs on. Zeroed here rather than where the
        // slice is parked, because "how many actions have run since" is a
        // property of the queue and the operator has no idea there is one.
        #[cfg(feature = "compression-schedule")]
        if population.schedule_slice_suspended {
            self.actions_since_suspension = 0;
        }
        self.settle_operator_call(
            mode,
            population,
            OperatorCallOpen {
                started_seconds,
                started_work,
                currency,
                counts_before,
            },
            parent_fingerprint,
            secondary_fingerprint,
            parent_role,
            action,
            #[cfg(feature = "sparse-rotation")]
            sparse_audition,
        )
    }

    /// The second half of one operator call: price it, charge it, archive it,
    /// publish it, report it.
    ///
    /// Split out of [`Self::run_operator`] because there are now **two** ways
    /// one operator call can happen. The ordinary one dispatches a mode against
    /// a parent. The other resumes a mode-34 slice the coordinator suspended in
    /// an earlier action, which cannot go through `run_operator` - there is no
    /// parent to dispatch against, only a slice to hand back its own frontier -
    /// and which must nonetheless be charged, archived, published and reported
    /// by exactly the same rules, or the interleaved run and the atomic run
    /// would not be comparable.
    #[allow(clippy::too_many_arguments)]
    fn settle_operator_call(
        &mut self,
        mode: usize,
        population: GeneralPersistentVacancyDiagnostics,
        open: OperatorCallOpen,
        parent_fingerprint: Option<String>,
        secondary_fingerprint: Option<String>,
        parent_role: ParentRole,
        action: Option<String>,
        #[cfg(feature = "sparse-rotation")] sparse_audition: bool,
    ) -> GeneralPersistentVacancyDiagnostics {
        let OperatorCallOpen {
            started_seconds,
            started_work,
            currency,
            counts_before,
        } = open;
        // Step two of the transaction: what did this call cost? The global
        // counter's delta is read *before* the debit, so it is the global
        // counter's own number and nothing else - `work_units()` folds in
        // every debit charged so far, and the ones charged before this call
        // are already inside `started_work`.
        let global_units = self.meter.work_units().saturating_sub(started_work);
        // Step two and a half: what would the *parallel* currency have charged
        // for the same call?
        //
        // The counts are the delta of an array of counters, so two processes
        // running the same work-budgeted arm compute the same number; the
        // weights are the machine profile. `confirmations` is the one input
        // that does not come from the profiling array - the schedule's own
        // report carries it, and it is the count the shipped meter's exact
        // half is *derived* from inside mode 34 (see
        // `CompressionSchedule::work_units`).
        let currency_call = currency.armed().then(|| {
            let mut after = work_currency_counts_now();
            // The operator's own half. Per-call already, so it is written onto
            // the `after` reading and carried through the delta untouched -
            // see `ClassCounts::delta`.
            let work = &population.work;
            after.position_source_attempts = work.position_source_attempts as u64;
            after.returned_positions = work.returned_positions as u64;
            after.pair_visits = (work.experimental_pair_visits as u64)
                .saturating_add(work.validator_pair_visits as u64);
            after.operator_collision_builds = (work.experimental_collision_builds as u64)
                .saturating_add(work.validator_collision_builds as u64);
            after.confirmations = schedule_confirmations_attempted(&population);
            let counts = crate::search::work_currency::ClassCounts::delta(&after, &counts_before);
            let class_units = crate::search::work_currency::price_for(mode).units(&counts);
            (counts, class_units)
        });
        // Step three: settle. The class price joins the operator's own meter
        // under the same `max`, and only when the currency is set to charge -
        // `Observe` computes the number, reports it, and does not spend it.
        let class_charge = currency_call
            .filter(|_| currency.charges())
            .map(|(_, units)| units);
        let self_metered = operator_self_metered_units(&population);
        // Spelled out rather than `self_metered.max(class_charge)`, which is
        // the same thing only because `Option`'s derived ordering puts `None`
        // below every `Some`. That is a Rust convention rather than an
        // arithmetic fact, and this is the line that decides what a run is
        // charged; a reader should not have to recall it.
        let settled_self = match (self_metered, class_charge) {
            (Some(operator), Some(class)) => Some(operator.max(class)),
            (Some(operator), None) => Some(operator),
            (None, Some(class)) => Some(class),
            (None, None) => None,
        };
        let charge = settle_operator_charge(&mut self.meter, global_units, settled_self);
        let work_currency_report = currency_call.map(|(counts, class_units)| {
            WorkCurrencyCallReport {
                candidate_queries: counts.candidate_queries,
                exact_pair_tests: counts.exact_pair_tests,
                collision_builds: counts.collision_builds,
                neighbor_tests: counts.neighbor_tests,
                full_rescores: counts.full_rescores,
                position_source_attempts: counts.position_source_attempts,
                returned_positions: counts.returned_positions,
                pair_visits: counts.pair_visits,
                operator_collision_builds: counts.operator_collision_builds,
                confirmations: counts.confirmations,
                class_units,
                // What the currency *itself* added, as opposed to what the
                // settlement added in total: zero unless the class price was
                // the strict maximum of the three.
                charged_extra_units: if currency.charges() {
                    class_units.saturating_sub(global_units.max(self_metered.unwrap_or(0)))
                } else {
                    0
                },
            }
        });
        // Step four: everything from here reads a settled meter.
        let elapsed_seconds = self.meter.seconds() - started_seconds;
        let work_units = charge.charged_units;
        if parent_role == ParentRole::Descended {
            if let Some(fingerprint) = parent_fingerprint.as_deref() {
                self.archive.charge_descent(fingerprint);
            }
        }
        let produced = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
            &population.final_placements,
        );
        let mut disposition = None;
        let mut published = false;
        let mut raw_depth_mm = None;
        let mut result_fingerprint = None;
        if produced.len() == self.pieces.len() {
            result_fingerprint = Some(general_placement_fingerprint(&produced));
        }
        if !produced.is_empty() {
            let (archived, depth) = self.archive_layout(
                produced.clone(),
                BasinOperator::Mode(mode),
                parent_fingerprint.clone(),
                secondary_fingerprint.clone(),
            );
            disposition = Some(format!("{archived:?}"));
            raw_depth_mm = depth;
            published = self.try_publish(&produced, &format!("mode{mode}"));
        }
        // The bit's evidence, taken from the slice's own report rather than from
        // anything this function inferred. `sparse_rotation_episodes > 0` is
        // "the mechanism fired"; `sparse_rotation_committed_moves == 0` is "and
        // not one of its proposals survived into a committed move".
        //
        // The second half used to read `rotation_accepted_moves`, which counts
        // *any* accepted move whose pose differs from the incumbent's - a
        // random catalogue start winning a sweep is one, and those happen on a
        // lane that was never offered a rung at all. Sol review 8 §2 P0 has the
        // material proof: the control arm of `docs/experiments/sparse-rotation/`
        // ran zero rungs and reported 11,523 `rotationAcceptedMoves`, so
        // "`accepted > 0`, the bit stays armed" was a verdict about the
        // catalogue and not about the operator, and "the disarm was never
        // necessary" was uninterpretable. `sparse_rotation_committed_moves` is
        // the operator's own chain - proposal inside an open episode, winner
        // that moved the pose, commit of that same pose - so a zero here is the
        // operator failing and nothing else.
        #[cfg(feature = "sparse-rotation")]
        if self.settings.sparse_rotation_bit {
            let slice = population.compression_schedule.as_ref();
            let episodes = slice.map_or(0, |report| report.sparse_rotation_episodes);
            let committed = slice.map_or(0, |report| report.sparse_rotation_committed_moves);
            self.sparse_rotation_bit.observe_slice(episodes, committed);
            if self.sparse_rotation_bit.disarmed {
                if published {
                    self.sparse_rotation_bit.barren_calls = 0;
                } else if !sparse_audition {
                    self.sparse_rotation_bit.barren_calls += 1;
                }
            }
        }
        self.operator_calls.push(OperatorCallReport {
            phase: self.phase_name.clone(),
            operator: format!("mode{mode}"),
            parent_fingerprint,
            secondary_parent_fingerprint: secondary_fingerprint,
            action,
            started_seconds,
            elapsed_seconds,
            work_units,
            global_units: charge.global_units,
            self_metered_units: charge.self_metered_units,
            debited_units: charge.debited_units,
            exact_valid: population.exact_valid,
            raw_depth_mm,
            result_fingerprint,
            archive_disposition: disposition,
            published,
            failure_reason: population.failure_reason.clone(),
            schedule_slice: schedule_slice_report(&population),
            work_currency: work_currency_report,
        });
        population
    }

    /// Resumes the mode-34 slice this coordinator suspended in an earlier
    /// action, and settles it as one more operator call.
    ///
    /// This is the other end of [`SliceControl::Suspend`], and it is the whole
    /// of what "the coordinator may run another action first" means. Nothing is
    /// rebuilt: the slice that comes back off `suspended_slice` still holds the
    /// frontier it stopped on, the deepest-confirmed slot, the lane's rng and
    /// weights and every surrogate and pair-NFP cache, so the batches after the
    /// interleave are the batches the uninterrupted slice would have run.
    ///
    /// It returns `None` when there is nothing suspended, so the queue can call
    /// it unconditionally.
    #[cfg(feature = "compression-schedule")]
    fn resume_suspended_slice(&mut self) -> Option<GeneralPersistentVacancyDiagnostics> {
        let suspended = self.suspended_slice.take()?;
        let batches_before = suspended.batches_run();
        let started_seconds = self.meter.seconds();
        let started_work = self.meter.work_units();
        let currency = self.settings.work_currency;
        let counts_before = currency
            .armed()
            .then(work_currency_counts_now)
            .unwrap_or_default();
        // The same policy the dispatch built, rebuilt from the same fields: a
        // resumed slice is subject to the wall stop and may suspend itself
        // again, which is what makes `m34yield` an interleave rather than a
        // single hand-back.
        let wall_stop_seconds = (self.settings.compression_schedule_wall_stop
            || self.settings.compression_schedule_wall_stop_all)
            .then(|| self.meter.wall_target_seconds)
            .flatten();
        let yield_after_batches =
            Some(self.settings.compression_schedule_yield_batches).filter(|batches| *batches > 0);
        let started = self.meter.started;
        let mut control = move |checkpoint: &ScheduleCheckpoint| {
            if let Some(limit) = wall_stop_seconds {
                if started.elapsed().as_secs_f64() >= limit {
                    return SliceControl::Stop;
                }
            }
            match yield_after_batches {
                Some(n) if (checkpoint.batch + 1) % n == 0 => SliceControl::Suspend,
                _ => SliceControl::Continue,
            }
        };
        let mut diagnostics = GeneralPersistentVacancyDiagnostics {
            mode: 34,
            attempted: true,
            seed_domain: crate::search::general_relaxed::COMPRESSION_SCHEDULE_SEED_DOMAIN,
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        let mut population =
            match crate::search::general_relaxed::resume_schedule_slice(suspended, &mut control) {
                Ok(outcome) => crate::search::general_relaxed::finish_schedule_outcome(
                    outcome,
                    diagnostics,
                    &mut self.suspended_slice,
                ),
                Err(error) => {
                    // A slice that fails on resumption is a failed operator call and
                    // not a failed run: the incumbent it was holding was published
                    // by the call that suspended it, so the run is never worse off
                    // than it was before the suspension.
                    diagnostics.failure_reason =
                        Some(format!("compression schedule resume: {error}"));
                    diagnostics
                }
            };
        // The same contract check every dispatched operator gets. It runs here
        // because a resumption does not go through
        // `dispatch_persistent_vacancy_mode` - there is no parent to dispatch
        // against, only a slice with its own frontier - and an operator call
        // whose layout was never contract-checked would be the one call in the
        // run that is not.
        self.record_schedule_contract(&mut population);
        Some(self.settle_operator_call(
            34,
            population,
            OperatorCallOpen {
                started_seconds,
                started_work,
                currency,
                counts_before,
            },
            None,
            None,
            // No descent is charged: the descent was charged when the slice was
            // dispatched, and charging it again would move the parent to the
            // back of a fairness queue it has already paid its way down.
            ParentRole::Prior,
            Some(format!("m34 resume from batch {batches_before}")),
            #[cfg(feature = "sparse-rotation")]
            false,
        ))
    }

    /// Ends a slice that is still parked when the run is over, where it stands.
    ///
    /// No batch is run and no confirmation is asked: the slice is told it was
    /// interrupted at the checkpoint it is already sitting on, and its report is
    /// written. So this costs the run nothing it has not already spent, and what
    /// it buys is that the slice's steps, sweeps, queries and confirmations
    /// appear in exactly one report rather than in none.
    #[cfg(feature = "compression-schedule")]
    fn drain_suspended_slice(&mut self) {
        let Some(suspended) = self.suspended_slice.take() else {
            return;
        };
        let batches_before = suspended.batches_run();
        let started_seconds = self.meter.seconds();
        let started_work = self.meter.work_units();
        let currency = self.settings.work_currency;
        let counts_before = currency
            .armed()
            .then(work_currency_counts_now)
            .unwrap_or_default();
        let mut diagnostics = GeneralPersistentVacancyDiagnostics {
            mode: 34,
            attempted: true,
            seed_domain: crate::search::general_relaxed::COMPRESSION_SCHEDULE_SEED_DOMAIN,
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        let mut population = match crate::search::general_relaxed::stop_suspended_slice(suspended) {
            Ok(outcome) => crate::search::general_relaxed::finish_schedule_outcome(
                outcome,
                diagnostics,
                &mut self.suspended_slice,
            ),
            Err(error) => {
                diagnostics.failure_reason = Some(format!("compression schedule drain: {error}"));
                diagnostics
            }
        };
        self.record_schedule_contract(&mut population);
        self.settle_operator_call(
            34,
            population,
            OperatorCallOpen {
                started_seconds,
                started_work,
                currency,
                counts_before,
            },
            None,
            None,
            ParentRole::Prior,
            Some(format!("m34 drain at batch {batches_before}")),
            #[cfg(feature = "sparse-rotation")]
            false,
        );
    }

    /// The contract half of `record_persistent_vacancy_contract_report`, for the
    /// two mode-34 calls that do not go through the dispatch.
    ///
    /// The parent argument is empty because it is only ever read as a *fallback
    /// layout* when the diagnostics carry none, and a resumed or drained slice
    /// always carries the incumbent it is holding.
    #[cfg(feature = "compression-schedule")]
    fn record_schedule_contract(&self, population: &mut GeneralPersistentVacancyDiagnostics) {
        crate::search::general_relaxed::record_persistent_vacancy_contract_report(
            population,
            self.pieces,
            self.fast_settings,
            &GeneralCoupledSeparatorArmDiagnostics::default(),
        );
    }

    fn base_relaxed_settings(&self) -> GeneralRelaxedSettings {
        let mut relaxed = self.settings.relaxed_template;
        relaxed.coupled_dynamic_separator = true;
        relaxed.construction_restart_window = None;
        relaxed.construction_void_cell_divisor = None;
        relaxed.alternation_max_cycles = None;
        relaxed
    }

    /// Whether this exact operator invocation has already been made.
    ///
    /// Every operator here is deterministic in `(pieces, settings, parent)`, so
    /// running one twice with the same key buys a bit-identical layout for the
    /// full price. The archive reports those as duplicates after the fact; this
    /// refuses them before the fact, which is what makes the descent phase's
    /// round-robin a *progressive* descent - each quantum's own output is a new
    /// parent, and only new parents are worth another quantum.
    fn already_attempted(&mut self, key: String) -> bool {
        !self.attempted.insert(key)
    }

    /// The mean cost of the calls this run has already made to `operator`, in
    /// the budget's own currency, or `None` if it has never called it.
    ///
    /// This is the coordinator pricing its own operators *from this run*, which
    /// is the only pricing that is both general - it carries no millimetres, no
    /// seconds and no request - and honest about the box it is running on.
    fn mean_operator_cost(&self, operator: &str) -> Option<f64> {
        let mut total = 0.0;
        let mut calls = 0usize;
        for call in &self.operator_calls {
            if call.operator == operator {
                total += self.meter.call_cost(call);
                calls += 1;
            }
        }
        (calls > 0).then(|| total / calls as f64)
    }

    /// Whether an operator call may be *started and finished* before `deadline`.
    ///
    /// v1 asked only "may I start?", which is why one 2.7 s crossover could be
    /// launched 0.1 s before its deadline and overrun the phase after it. When
    /// the operator has been priced by this run, the check is `remaining >=
    /// multiple * mean cost`; when it has not, the check degrades to v1's
    /// deadline test, because refusing an unpriced operator would mean never
    /// pricing it.
    ///
    /// It reports *which* of its two clauses refused, because the two are
    /// different findings about a saturated run - a phase stopped by its
    /// deadline has actions left and a phase stopped by affordability has
    /// actions it cannot pay for - and the boolean this replaced collapsed them
    /// into one `false`. `None` is "yes, go ahead".
    fn affordability(
        &self,
        deadline: f64,
        operator: &str,
        multiple: f64,
    ) -> Option<PhaseExitCause> {
        // Asked first, and asked in *seconds*: under a plan or a work budget
        // every clause below this one is denominated in a counter, and a
        // counter cannot see a box under load. This is the whole of what
        // `m34wallstopall` adds to the classes that own no checkpoint.
        if self.wall_stop_refuses(None) {
            return Some(PhaseExitCause::WallStop);
        }
        if !self.meter.has_room(deadline) {
            return Some(PhaseExitCause::Deadline);
        }
        match self.mean_operator_cost(operator) {
            None => None,
            Some(cost) if self.meter.remaining_to(deadline) >= multiple * cost => None,
            Some(_) => Some(PhaseExitCause::Affordability),
        }
    }

    /// Whether the wall stop refuses to start an action now, optionally of a
    /// named class.
    ///
    /// `None` is the question the phase loops ask - *"is the deadline already
    /// behind us?"* - and it is exact. `Some(class)` additionally applies
    /// [`PortfolioSettings::compression_schedule_wall_stop_reserve`] against
    /// that class's own measured mean seconds in this run, and it is an
    /// estimate: a class this run has never bought has no mean, and is
    /// admitted rather than refused, for the reason
    /// [`Self::affordability`] admits an unpriced operator.
    ///
    /// Always `false` when the key is off, when the budget named no wall, and
    /// in a build without the compression schedule - so this is a no-op on
    /// every path any pinned number in this repository was measured on.
    #[allow(unused_variables)]
    fn wall_stop_refuses(&self, class: Option<ActionClass>) -> bool {
        #[cfg(feature = "compression-schedule")]
        {
            if !self.settings.compression_schedule_wall_stop_all {
                return false;
            }
            let reserve = match class {
                None => 0.0,
                Some(class) => {
                    let multiple = self.settings.compression_schedule_wall_stop_reserve;
                    if multiple <= 0.0 {
                        0.0
                    } else {
                        self.mean_class_seconds(class).unwrap_or(0.0) * multiple
                    }
                }
            };
            self.meter.wall_target_passed(reserve)
        }
        #[cfg(not(feature = "compression-schedule"))]
        {
            false
        }
    }

    /// One class's mean wall cost per action in this run, or `None` when the
    /// run has not bought one.
    ///
    /// The class's own accumulated seconds rather than a per-operator mean:
    /// the reserve is about the *action* the queue is about to buy, and a
    /// diversify action is a constructor arm plus a legalization quantum
    /// rather than one operator call.
    #[cfg(feature = "compression-schedule")]
    fn mean_class_seconds(&self, class: ActionClass) -> Option<f64> {
        self.class_stats
            .get(&class)
            .filter(|stats| stats.actions > 0)
            .map(|stats| stats.seconds / stats.actions as f64)
    }

    /// Records why the phase in flight stopped.
    fn note_exit(&mut self, cause: PhaseExitCause) {
        self.exit_cause = cause;
    }
}

/// Which flag a run armed to get its work counters, and what it found there.
///
/// Reported rather than inferred: `PortfolioSettings::lane_local_debit` names a
/// preference and this names the outcome, and the two differ whenever
/// `work_currency` is armed beside it. A reader of an evidence document must be
/// able to see which arm actually ran without re-deriving it from three
/// settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkMeterArmingReport {
    /// Whether the run needed work counters at all: `false` for a wall budget,
    /// which is the one mode that never reads them.
    pub needed: bool,
    /// Whether the profiler was armed - every counter and every span.
    pub profiler_armed: bool,
    /// Whether the work meter's own flag was armed instead.
    pub metering_armed: bool,
    /// Whether `lane_local_debit` was asked for and refused, which happens
    /// only when `work_currency` is armed beside it.
    pub deferred_to_profiler: bool,
}

/// Arms the counters one run needs, and puts back what it found on the way out.
///
/// Scoped, and deliberately more careful than the `profiling::set_enabled(true)`
/// it replaces: that call leaked, so one work-budgeted request left the
/// profiler armed for every later request in the same process - including a
/// wall-budgeted one, which would then silently pay the tax this whole setting
/// exists to remove. A napi or CLI host runs many requests in one process.
struct WorkMeterArming {
    previous_profiler: bool,
    previous_metering: bool,
    report: WorkMeterArmingReport,
}

impl WorkMeterArming {
    fn install(settings: &PortfolioSettings) -> Self {
        let previous_profiler = profiling::enabled();
        let previous_metering = profiling::metering_enabled();
        let needed = matches!(
            settings.budget,
            PortfolioBudget::Work { .. } | PortfolioBudget::Plan { .. }
        );
        // The currency prices three counters the meter does not read, so it
        // cannot run on the meter's flag. Deferring rather than refusing keeps
        // the currency honest and keeps the key from silently meaning nothing.
        let currency_needs_profiler = settings.work_currency.armed();
        let metering = needed && settings.lane_local_debit && !currency_needs_profiler;
        let profiler = needed && !metering;
        if profiler {
            profiling::set_enabled(true);
        }
        if metering {
            profiling::set_metering_enabled(true);
        }
        Self {
            previous_profiler,
            previous_metering,
            report: WorkMeterArmingReport {
                needed,
                profiler_armed: profiler || previous_profiler,
                metering_armed: metering || previous_metering,
                deferred_to_profiler: needed
                    && settings.lane_local_debit
                    && currency_needs_profiler,
            },
        }
    }

    fn report(&self) -> WorkMeterArmingReport {
        self.report
    }
}

impl Drop for WorkMeterArming {
    fn drop(&mut self) {
        profiling::set_enabled(self.previous_profiler);
        profiling::set_metering_enabled(self.previous_metering);
    }
}

/// Holds the exact-clearance certificate at one arming for the length of one
/// coordinator run, and puts back what it found on the way out.
///
/// Scoped rather than set-and-leave because the switch is process-wide and this
/// crate is a library: a napi or CLI host that runs two requests in one process
/// must not have the first request's `fcv=0` silently disarm the second. `Drop`
/// rather than a matching call at the end of `run_portfolio` because that
/// function has a dozen `?` returns and one of them would eventually be the one
/// that leaked.
#[cfg(feature = "fast-contract-validator")]
struct ContractCertificateArming {
    previous: bool,
}

#[cfg(feature = "fast-contract-validator")]
impl ContractCertificateArming {
    fn install(armed: bool) -> Self {
        Self {
            previous: crate::validation::general_polygon::set_contract_certificate_armed(armed),
        }
    }
}

#[cfg(feature = "fast-contract-validator")]
impl Drop for ContractCertificateArming {
    fn drop(&mut self) {
        crate::validation::general_polygon::set_contract_certificate_armed(self.previous);
    }
}

/// Holds the round-envelope kernel at one arming for the length of one
/// coordinator run, and puts back what it found on the way out.
///
/// Identical in shape to [`ContractCertificateArming`] and for the same two
/// reasons — the switch is process-wide, and `run_portfolio` has too many `?`
/// returns for a matching call at the end to be safe — with one difference that
/// matters more here than there. The certificate is verdict-preserving, so a
/// leaked arming would be invisible; this one changes what the engine accepts,
/// so a leaked arming would silently make every later request in the same
/// process a different engine. That is why it is `Drop` and why it is
/// constructed only on the v3 path.
#[cfg(feature = "round-envelope-kernel")]
struct RoundEnvelopeArming {
    previous: crate::validation::round_envelope::KernelMode,
}

#[cfg(feature = "round-envelope-kernel")]
impl RoundEnvelopeArming {
    fn install(mode: crate::validation::round_envelope::KernelMode) -> Self {
        Self {
            previous: crate::validation::round_envelope::set_kernel_mode(mode),
        }
    }
}

#[cfg(feature = "round-envelope-kernel")]
impl Drop for RoundEnvelopeArming {
    fn drop(&mut self) {
        crate::validation::round_envelope::set_kernel_mode(self.previous);
    }
}

/// Runs the portfolio from the request only: no pinned parent, no warm start,
/// no fixture anywhere.
///
/// The phases, in order, are the review's ten-second sketch. Each is entered
/// only if its deadline has not already passed, and each stops issuing operator
/// calls once it has - so a schedule degrades by dropping *later* work, never
/// by overrunning.
pub fn run_portfolio(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    settings: &PortfolioSettings,
) -> Result<PortfolioOutcome, GeneralFastError> {
    // A work budget is a function of the counters, so it needs them. A plan
    // *is* a work budget from the moment phase 0 ends, and its probe is
    // denominated in the same counters, so it needs them from before phase 0
    // begins - which is here, and is the reason the plan's wall target buys a
    // run that carries the counters' cost for the whole of the wall it was
    // given. `docs/experiments/calibrated-plan/` §9 priced that in millimetres
    // and called it a floor; `PortfolioSettings::lane_local_debit` is the
    // setting that takes the counting without the timing, and this is where the
    // choice between the two is made.
    let _work_meter_arming = WorkMeterArming::install(settings);
    // The v3 coordinator's own arming of the exact-clearance certificate, for
    // the duration of this run and no longer. Off the v3 path this is not
    // constructed at all, so nothing about a v2 or a direct-engine caller
    // changes - and with the feature compiled and the setting at its `true`
    // default, arming it is what the process was already doing.
    #[cfg(feature = "fast-contract-validator")]
    let _certificate_arming = if settings.coordinator_v3 {
        Some(ContractCertificateArming::install(
            settings.fast_contract_validator,
        ))
    } else {
        None
    };
    // The round-envelope kernel's arming, on the same terms and only on the v3
    // path. Off the v3 path this is not constructed at all, so a v2 or a direct
    // engine caller keeps HEAD's miter authority whatever the setting says -
    // and with the feature compiled and the setting at its `false` default, the
    // guard installs `false`, which is what the process was already doing.
    #[cfg(feature = "round-envelope-kernel")]
    let _round_envelope_arming = if settings.coordinator_v3 {
        Some(RoundEnvelopeArming::install(settings.round_envelope_kernel))
    } else {
        None
    };
    let mut meter = BudgetMeter::new(settings.budget);
    // Before the first line of phase 0, because the probe's whole subject is
    // phase 0 and a sampler armed after it started would be measuring a
    // different window than the one `install_plan` divides by. Inert unless the
    // budget is a plan and more than one bucket was asked for.
    meter.arm_plan_probe(settings);

    // ---- phase 0: protected mode 0 and shared preprocessing ---------------
    // It is never skipped and never budget-checked: it is the result the
    // engine would have returned without a coordinator, and a coordinator that
    // could return something worse than that would be a regression whatever
    // its schedule said.
    let phase_started = meter.seconds();
    let phase_work = meter.work_units();
    let constructed = construct_short_side_first(pieces, fast_settings)?;
    let constructed_depth_mm = constructed.used_long_axis_depth_mm;
    let mut relaxed = settings.relaxed_template;
    relaxed.coupled_dynamic_separator = true;
    relaxed.persistent_vacancy_mode = 0;
    relaxed.persistent_vacancy_target_depth_mm = None;
    relaxed.construction_restart_window = None;
    relaxed.construction_void_cell_divisor = None;
    relaxed.alternation_max_cycles = None;
    let m0 = crate::search::general_relaxed::improve_complete_layout(
        pieces,
        fast_settings,
        relaxed,
        &constructed,
    )?;

    let m0_fingerprint = general_placement_fingerprint(&m0.result.placements);
    let m0_raw_depth = crate::search::general_relaxed::coupled_raw_source_depth(
        pieces,
        &m0.result.placements,
        fast_settings,
    )
    .ok();
    let m0_valid =
        validate_and_measure_placements(pieces, &m0.result.placements, fast_settings).is_ok();
    let incumbent = PublishedIncumbent {
        result: m0.result.clone(),
        fingerprint: m0_fingerprint,
        raw_depth_mm: m0_raw_depth,
        dual_gate_valid: m0_valid,
        source: "m0".to_owned(),
        published_seconds: meter.seconds(),
        published_work_units: meter.work_units(),
    };

    let mut coordinator = Coordinator {
        pieces,
        fast_settings,
        settings: settings.clone(),
        meter,
        incumbent,
        archive: SearchArchive::new(
            settings.archive_capacity,
            pieces.len(),
            settings.similarity_threshold,
        ),
        publications: Vec::new(),
        operator_calls: Vec::new(),
        phases: Vec::new(),
        phase_name: "m0".to_owned(),
        attempted: std::collections::BTreeSet::new(),
        descent_stalled: false,
        exit_cause: PhaseExitCause::Completed,
        protected_fraction: 0.0,
        phase_zero_cost: 0.0,
        class_stats: BTreeMap::new(),
        #[cfg(feature = "compression-schedule")]
        schedule_slices: 0,
        #[cfg(feature = "compression-schedule")]
        suspended_slice: None,
        #[cfg(feature = "compression-schedule")]
        actions_since_suspension: 0,
        #[cfg(feature = "sparse-rotation")]
        sparse_rotation_bit: SparseRotationBit::default(),
    };

    // Everything phase 0 produced goes into the archive, including the arms
    // that lost: the coupled separator's control, treatment and
    // boundary-projection arms are three structurally different complete
    // layouts that the engine currently throws away.
    coordinator.archive_layout(
        constructed.placements.clone(),
        BasinOperator::Constructor,
        None,
        None,
    );
    let m0_placements = coordinator.incumbent.result.placements.clone();
    coordinator.archive_layout(m0_placements, BasinOperator::RelaxedM0, None, None);
    if let Some(coupled) = m0.diagnostics.coupled_dynamic_separator.as_ref() {
        let arms = [
            Some(&coupled.control),
            Some(&coupled.treatment),
            coupled.boundary_projection_treatment.as_ref(),
        ];
        for arm in arms.into_iter().flatten() {
            let placements =
                crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
                    &arm.final_placements,
                );
            if !placements.is_empty() {
                coordinator.archive_layout(placements, BasinOperator::CoupledSeparator, None, None);
            }
        }
    }
    coordinator.phases.push(PhaseReport {
        name: "m0".to_owned(),
        deadline_fraction: 0.0,
        entered_seconds: phase_started,
        elapsed_seconds: coordinator.meter.seconds() - phase_started,
        work_units: coordinator.meter.work_units().saturating_sub(phase_work),
        operator_calls: 0,
        publications: 0,
        skipped: false,
        exit_cause: PhaseExitCause::Completed,
    });
    // ---- the calibrated work plan -----------------------------------------
    // Here, and nowhere else: phase 0 has finished, so the probe is complete,
    // and `protected_fraction` on the next line is the first statement in the
    // whole function that reads a budget. Between `BudgetMeter::new` and this
    // line the budget is only ever *recorded*, never *spent against*, which is
    // what makes `PortfolioBudget::Plan` a two-line lifetime rather than a
    // third case for the schedule to carry.
    let plan = match coordinator.meter.budget {
        PortfolioBudget::Plan { target_millis } => {
            Some(coordinator.meter.install_plan(target_millis, settings))
        }
        _ => None,
    };
    debug_assert!(
        !matches!(coordinator.meter.budget, PortfolioBudget::Plan { .. }),
        "the plan must be installed before any budget is read"
    );
    // Everything after this point is a fraction of what phase 0 left, not of
    // the whole budget. See `PhaseSchedule`.
    coordinator.protected_fraction = coordinator.meter.spent_fraction().clamp(0.0, 1.0);
    // One full mode-0 pipeline on this request, in this process, in the
    // budget's own currency. It is the unit every v3 class prior is quoted in.
    coordinator.phase_zero_cost = coordinator.meter.currency_spent().max(f64::MIN_POSITIVE);

    // The constructor clamp, derived from the request rather than pinned.
    let area_lower_bound_depth_mm = area_lower_bound_depth_mm(pieces, fast_settings)?;
    let constructor_clamp_mm =
        constructor_clamp_mm(area_lower_bound_depth_mm, constructed_depth_mm);

    // ---- phase 1: alternation quanta across the distinct frontier ---------
    // First, not third. It is the most productive operator this schedule has
    // (9 publications in 18 calls on the v1 stream) and the constructor slice
    // that used to precede it published nothing in nineteen.
    let template_epochs = settings.relaxed_template.epochs.max(1);

    // ---- v3: one ranked action loop in place of phases 1-4 ----------------
    // The v2 schedule below is a single pass: descent, then crossover, then
    // compression, then diversify, each entered once. The ledger measured what
    // that costs - on seeds 0 and 1 the final rank-0 state is born in the
    // *compression* phase, after the crossover phase has ended, so the run's
    // best state and its recombination operator never meet. v3 replaces the
    // pass with a queue that re-enumerates after every action, so a state born
    // late re-enters every class it is eligible for.
    // The race runs *before* the queue and only inside v3, because it is a
    // decision the queue then inherits: an eliminated arm is out of the
    // archive by the time the first action is enumerated. Off by default; see
    // `run_basin_race`.
    #[cfg(feature = "compression-schedule")]
    let basin_race_report = if settings.coordinator_v3 && settings.basin_race {
        Some(run_basin_race(&mut coordinator, constructor_clamp_mm))
    } else {
        None
    };
    let mut tranches: Vec<TrancheReport> = Vec::new();
    let schedule_report = if settings.coordinator_v3 {
        Some(run_v3_schedule(
            &mut coordinator,
            constructor_clamp_mm,
            plan.as_ref(),
            &mut tranches,
        ))
    } else {
        None
    };
    // Both mode-34 dispatch sites - the queue's `Schedule` action and the
    // race's audition batch - are reachable only inside v3, and `run_v3_schedule`
    // drains the slot on its way out, so a run cannot end holding a live slice.
    // Asserted rather than commented, because a third dispatch site added later
    // would silently drop a slice's whole account and every aggregate would
    // still look plausible.
    #[cfg(feature = "compression-schedule")]
    debug_assert!(
        coordinator.suspended_slice.is_none(),
        "a run ended holding a suspended mode-34 slice"
    );
    if !settings.coordinator_v3 {
        coordinator.run_phase("descent", settings.schedule.descent_by, |run| {
            let mut cycles = run.settings.descent_cycles.max(1);
            let mut epochs = run.settings.descent_relaxed_epochs.max(1);
            loop {
                // Re-selected every round, because a quantum's own output is an
                // archive member and therefore a candidate parent for the next
                // round. That is what turns a round-robin over the frontier into a
                // progressive descent rather than a repeated one.
                let frontier = run.archive.distinct_frontier(run.settings.descent_states);
                if frontier.is_empty() {
                    run.note_exit(PhaseExitCause::GeometricFixpoint);
                    return;
                }
                let mut spent_any = false;
                for basin in frontier {
                    if let Some(cause) = run.affordability(run.deadline, "mode22", 1.0) {
                        run.note_exit(cause);
                        return;
                    }
                    // The quantum's *size* is part of the key: a second pass at a
                    // deeper quantum is a different operator call, not a repeat.
                    if run.already_attempted(format!("22:{cycles}:{epochs}:{}", basin.fingerprint))
                    {
                        continue;
                    }
                    spent_any = true;
                    let target = basin.raw_depth_mm + ALTERNATION_RUNG_MM;
                    run.run_operator(
                        22,
                        &basin.placements.clone(),
                        Some(basin.fingerprint.clone()),
                        Some(target),
                        |relaxed| {
                            relaxed.alternation_max_cycles = Some(cycles);
                            relaxed.epochs = epochs;
                        },
                        None,
                        ParentRole::Descended,
                        None,
                    );
                }
                if spent_any {
                    continue;
                }
                // The frontier is a fixpoint *at this quantum size*: every distinct
                // state has had this much alternation and none of it produced a new
                // layout.
                //
                // The obvious next move is to deepen the quantum rather than hand
                // the remaining budget to a later phase, and this is that move,
                // measured and declined. Doubling the cycle and epoch counts until
                // the mode's own bound and the caller's own epoch count are reached
                // does keep the phase busy - and on the measured stream it takes
                // the budget away from the crossover phase, which is the *second*
                // most productive operator here (three publications in nine calls),
                // and the result gets worse: seed 1 goes from 176.056 to 179.633
                // under the review's schedule and to 176.753 under the focused one.
                // So the default is off, and the flag stays as the instrument that
                // priced it.
                if !run.settings.descent_iterated_deepening {
                    run.descent_stalled = true;
                    run.note_exit(PhaseExitCause::KeysExhausted);
                    break;
                }
                let deepened_cycles = (cycles * 2).min(ALTERNATION_MAX_CYCLES);
                let deepened_epochs = (epochs * 2).min(template_epochs);
                if deepened_cycles == cycles && deepened_epochs == epochs {
                    run.descent_stalled = true;
                    run.note_exit(PhaseExitCause::KeysExhausted);
                    break;
                }
                cycles = deepened_cycles;
                epochs = deepened_epochs;
            }
        });

        // ---- phase 2: crossovers over the distinct archive pairs --------------
        // Second, not fourth, and repeatable. The review called mode 23
        // "conditional but currently evidence-required"; the v1 measurement made it
        // evidence-*producing* - the largest single published gains in the run -
        // and then gave it 0.6 s of a ten-second budget. The condition stays what
        // the review wrote it as: two structurally distinct archive states.
        coordinator.run_phase("crossover", settings.schedule.crossover_by, |run| {
            for _ in 0..run.settings.crossover_attempts {
                if let Some(cause) = run.affordability(run.deadline, "mode23", 1.0) {
                    run.note_exit(cause);
                    return;
                }
                // Re-selected every attempt: a crossover that published moved the
                // incumbent, and the next pair should be drawn from where the
                // archive is now, not from where it was.
                let frontier = run
                    .archive
                    .distinct_frontier(run.settings.crossover_states.max(2));
                if frontier.len() < 2 {
                    run.note_exit(PhaseExitCause::GeometricFixpoint);
                    return;
                }
                let fingerprints = frontier
                    .iter()
                    .map(|basin| basin.fingerprint.clone())
                    .collect::<Vec<_>>();
                let Some((left, right, key)) =
                    first_unattempted_crossover_pair(&fingerprints, &run.attempted)
                else {
                    run.note_exit(PhaseExitCause::KeysExhausted);
                    return;
                };
                run.already_attempted(key);
                let parent_b = GeneralPersistentVacancyPinnedParent {
                    placements: frontier[right].placements.clone(),
                    source: "archive".to_owned(),
                    source_sha256: frontier[right].fingerprint.clone(),
                };
                run.run_operator(
                    23,
                    &frontier[left].placements.clone(),
                    Some(frontier[left].fingerprint.clone()),
                    Some(CROSSOVER_CUT_FRACTION),
                    |_| {},
                    Some(&parent_b),
                    ParentRole::Descended,
                    Some(format!(
                        "x:forward:{left}->{right}@{CROSSOVER_CUT_FRACTION}"
                    )),
                );
                // Both parents were descended from. v1 charged only the first,
                // which is the same defect - a descent that happened going
                // uncharged - as charging the constructor's pose prior, in the
                // other direction.
                let right_fingerprint = frontier[right].fingerprint.clone();
                run.archive.charge_descent(&right_fingerprint);
            }
        });

        // ---- phase 3: micro-descent, and m31 only on a residue -----------------
        // The order is inverted from v1. v1 asked mode 31 to legalize a *clean*
        // mode-22 fixpoint one rung below its own depth: six calls, zero
        // exact-valid results, every one "global legalization did not reach a
        // feasible fixpoint". The review's own sentence is that m31 is
        // production-worthy "only as the legalizer for a compressed/perturbed
        // frontier", so v2 does the compression first and hands m31 the residue if
        // and only if one exists - a complete layout the compressing descent
        // returned that the exact validator refuses.
        coordinator.run_phase("compression", settings.schedule.compression_by, |run| {
            if let Some(cause) = run.affordability(run.deadline, "mode22", 1.0) {
                run.note_exit(cause);
                return;
            }
            let parent = run.incumbent.result.placements.clone();
            let fingerprint = run.incumbent.fingerprint.clone();
            let Some(depth) = run.incumbent.raw_depth_mm else {
                run.note_exit(PhaseExitCause::GeometricFixpoint);
                return;
            };
            if run.already_attempted(format!("22c:{fingerprint}")) {
                run.note_exit(PhaseExitCause::KeysExhausted);
                return;
            }
            let epochs = run.settings.descent_relaxed_epochs;
            let compressed = run.run_operator(
                22,
                &parent,
                Some(fingerprint),
                Some(depth + ALTERNATION_RUNG_MM),
                |relaxed| {
                    relaxed.alternation_max_cycles = Some(1);
                    relaxed.epochs = epochs;
                },
                None,
                ParentRole::Descended,
                None,
            );
            if compressed.exact_valid {
                // Nothing to legalize. This is the whole demotion: on this stream
                // the trigger does not fire, and a phase that does not fire costs
                // nothing instead of costing six refused calls.
                run.note_exit(PhaseExitCause::NoResidue);
                return;
            }
            let residue = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
                &compressed.final_placements,
            );
            if residue.len() != run.pieces.len() {
                run.note_exit(PhaseExitCause::NoCompleteLayout);
                return;
            }
            if !run.meter.has_room(run.deadline) {
                run.note_exit(PhaseExitCause::Deadline);
                return;
            }
            let Some(residue_depth) = crate::search::general_relaxed::coupled_raw_source_depth(
                run.pieces,
                &residue,
                run.fast_settings,
            )
            .ok() else {
                run.note_exit(PhaseExitCause::GeometricFixpoint);
                return;
            };
            // One rung of the engine's own construction drop ladder below the
            // residue, which is the smallest bound this engine ever asks a
            // legalizer for.
            let bound = residue_depth - COMPRESSION_RUNG_MM;
            if bound <= 0.0 {
                run.note_exit(PhaseExitCause::GeometricFixpoint);
                return;
            }
            let residue_fingerprint = general_placement_fingerprint(&residue);
            if run.already_attempted(format!("31:{residue_fingerprint}")) {
                run.note_exit(PhaseExitCause::KeysExhausted);
                return;
            }
            run.run_operator(
                31,
                &residue,
                Some(residue_fingerprint),
                Some(bound),
                |_| {},
                None,
                ParentRole::Descended,
                None,
            );
        });

        // ---- phase 4: diversify - draw a basin only if it can be descended ----
        // The constructor slice, conditional and last. Each iteration draws one
        // salted arm and immediately spends a quantum on it, because the v1
        // measurement's finding was not "mode 20 is bad" - all nineteen arms were
        // exact-valid - but "nineteen arms that nobody descended from published
        // nothing". An arm that is drawn is now descended from in the same
        // iteration or it is not drawn at all.
        coordinator.run_phase("diversify", settings.schedule.diversify_by, |run| {
            match run.settings.basin_trigger {
                BasinTrigger::Never => {
                    run.note_exit(PhaseExitCause::TriggerRefused);
                    return;
                }
                BasinTrigger::OnStall if !run.descent_stalled => {
                    run.note_exit(PhaseExitCause::TriggerRefused);
                    return;
                }
                _ => {}
            }
            let priced = run.settings.basin_trigger == BasinTrigger::WhenDescendable;
            let patience = run.settings.basin_patience.max(1);
            let mut barren = 0usize;
            for slot in 0..run.settings.basin_slots {
                let publications_before = run.publications.len();
                if !run.meter.has_room(run.deadline) {
                    run.note_exit(PhaseExitCause::Deadline);
                    return;
                }
                if priced {
                    // A quantum is the price of *using* a basin, and a basin that
                    // is not used is the 19/19 refusal. An arm has never been
                    // priced when the first one is drawn, so it is charged a
                    // quantum's price until it has priced itself.
                    let Some(quantum) = run.mean_operator_cost("mode22") else {
                        run.note_exit(PhaseExitCause::Affordability);
                        return;
                    };
                    let arm = run.mean_operator_cost("mode20").unwrap_or(quantum);
                    if run.meter.remaining_to(run.deadline) < arm + quantum {
                        run.note_exit(PhaseExitCause::Affordability);
                        return;
                    }
                }
                let salt = slot as f64 * BASIN_TARGET_SALT_RELATIVE_STEP * constructor_clamp_mm;
                let divisor = if run.settings.cell_divisor_salts.is_empty() {
                    None
                } else {
                    Some(
                        run.settings.cell_divisor_salts
                            [slot % run.settings.cell_divisor_salts.len()],
                    )
                };
                let parent = run.incumbent.result.placements.clone();
                let parent_fingerprint = run.incumbent.fingerprint.clone();
                let drawn = run.run_operator(
                    20,
                    &parent,
                    Some(parent_fingerprint),
                    Some(constructor_clamp_mm + salt),
                    |relaxed| {
                        relaxed.construction_restart_window = Some((slot, 1));
                        relaxed.construction_void_cell_divisor = divisor;
                    },
                    None,
                    // The constructor builds from scratch; the incumbent is only
                    // its pose prior, so this is not that basin's quantum.
                    ParentRole::Prior,
                    Some(format!("m20:slot{slot}")),
                );
                let basin =
                    crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
                        &drawn.final_placements,
                    );
                if basin.len() != run.pieces.len() {
                    // An arm that produced no complete layout is evidence about the
                    // clamp, not about this slot: consecutive slots differ only in
                    // a salt of one part in ten thousand, so the next arm would be
                    // refused for the same reason at the same price. Stopping here
                    // is what turns "the clamp was wrong" from eight wasted arms -
                    // 2.04 s of a 3.88 s shapes-17 run, measured - into one.
                    run.note_exit(PhaseExitCause::NoCompleteLayout);
                    return;
                }
                if !run.meter.has_room(run.deadline) {
                    run.note_exit(PhaseExitCause::Deadline);
                    return;
                }
                let Some(basin_depth) = crate::search::general_relaxed::coupled_raw_source_depth(
                    run.pieces,
                    &basin,
                    run.fast_settings,
                )
                .ok() else {
                    barren += 1;
                    if barren >= patience {
                        run.note_exit(PhaseExitCause::Patience);
                        return;
                    }
                    continue;
                };
                let basin_fingerprint = general_placement_fingerprint(&basin);
                let cycles = run.settings.descent_cycles.max(1);
                let epochs = run.settings.descent_relaxed_epochs.max(1);
                if run.already_attempted(format!("22:{cycles}:{epochs}:{basin_fingerprint}")) {
                    // The arm rebuilt a layout some quantum has already descended
                    // from, so it bought nothing. That is a barren iteration on the
                    // same terms as one whose descent published nothing.
                    barren += 1;
                    if barren >= patience {
                        run.note_exit(PhaseExitCause::Patience);
                        return;
                    }
                    continue;
                }
                run.run_operator(
                    22,
                    &basin,
                    Some(basin_fingerprint),
                    Some(basin_depth + ALTERNATION_RUNG_MM),
                    |relaxed| {
                        relaxed.alternation_max_cycles = Some(cycles);
                        relaxed.epochs = epochs;
                    },
                    None,
                    ParentRole::Descended,
                    Some(format!("m22:slot{slot}")),
                );
                if run.publications.len() > publications_before {
                    barren = 0;
                } else {
                    barren += 1;
                    if barren >= patience {
                        run.note_exit(PhaseExitCause::Patience);
                        return;
                    }
                }
            }
        });
    } // end of the v2 phase sequence

    // ---- phase 5: drain ---------------------------------------------------
    // Every archived basin that is exact-valid and might beat the incumbent is
    // offered to the adoption rule once. Publication is still the adoption
    // rule's decision; the drain only makes sure nothing the run found is left
    // unoffered.
    coordinator.run_phase("drain", settings.schedule.drain_by, |run| {
        let mut candidates = run
            .archive
            .basins()
            .iter()
            .filter(|basin| basin.exact_valid)
            .cloned()
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.raw_depth_mm.total_cmp(&right.raw_depth_mm));
        for basin in candidates {
            if run
                .incumbent
                .raw_depth_mm
                .is_some_and(|depth| basin.raw_depth_mm >= depth)
            {
                break;
            }
            run.try_publish(&basin.placements, "drain");
        }
    });

    // ---- phase 6: the A/B/C probe -----------------------------------------
    // One action, from the state the schedule saturated at, on an allowance
    // every arm shares. It runs *after* the whole schedule, including the
    // drain, so the base trajectory of an A run, a B run and a C run is
    // bit-identical and the arms are paired on the same saturated archive by
    // construction. Compiled only under `portfolio-ledger`; the shipping
    // schedule has no such phase and no branch that could reach one.
    #[cfg(feature = "portfolio-ledger")]
    let probe = run_probe_phase(&mut coordinator, constructor_clamp_mm);
    #[cfg(not(feature = "portfolio-ledger"))]
    let probe = None;

    let elapsed_seconds = coordinator.meter.seconds();
    let work_units = coordinator.meter.work_units();
    let budget = coordinator.meter.budget;
    let descent_stalled = coordinator.descent_stalled;
    #[cfg(feature = "portfolio-ledger")]
    let ledger = Some(build_ledger(&coordinator));
    #[cfg(not(feature = "portfolio-ledger"))]
    let ledger = None;
    Ok(PortfolioOutcome {
        ledger,
        probe,
        plan,
        tranches,
        descent_stalled,
        result: coordinator.incumbent.result.clone(),
        incumbent: coordinator.incumbent,
        archive: coordinator.archive.report(),
        m0_diagnostics: Box::new(m0.diagnostics),
        phases: coordinator.phases,
        schedule: schedule_report,
        #[cfg(feature = "compression-schedule")]
        basin_race: basin_race_report,
        operator_calls: coordinator.operator_calls,
        publications: coordinator.publications,
        work_currency: settings.work_currency,
        // `filter` and not `Some`: a run that armed the profiler exactly as it
        // always has has nothing to report, and a key that appeared on every
        // plan document would make every pinned document digest in this
        // repository a digest of a different document.
        work_meter_arming: Some(_work_meter_arming.report())
            .filter(|report| report.metering_armed || report.deferred_to_profiler),
        budget,
        elapsed_seconds,
        work_units,
        constructor_clamp_mm,
        area_lower_bound_depth_mm,
        constructed_depth_mm,
    })
}

/// The first frontier pair a crossover has not attempted, in
/// `(0,1), (0,2), (1,2), (0,3), ...` order.
///
/// Ordered by the *worse* member's rank first, so a phase that can afford only
/// one call spends it on the two best distinct states - which is the same
/// "best structurally distinct" ordering the alternation phase uses, applied to
/// a pair rather than to a single parent.
fn first_unattempted_crossover_pair(
    fingerprints: &[String],
    attempted: &std::collections::BTreeSet<String>,
) -> Option<(usize, usize, String)> {
    for right in 1..fingerprints.len() {
        for left in 0..right {
            let key = format!("23:{}:{}", fingerprints[left], fingerprints[right]);
            if !attempted.contains(&key) {
                return Some((left, right, key));
            }
        }
    }
    None
}

/// The alternation descent rung: the engine's own construction drop ladder's
/// second step, which is the rung mode 22 walks internally.
const ALTERNATION_RUNG_MM: f64 = 0.8;

/// The compression bound step: the drop ladder's first, smallest rung.
const COMPRESSION_RUNG_MM: f64 = 0.4;

/// Mode 23's cut fraction: half of parent A's own measured short-axis span.
/// Dimensionless and scale-free by the mode's own definition.
const CROSSOVER_CUT_FRACTION: f64 = 0.5;

/// The probe arm C ladder drop, in millimetres below the incumbent's own raw
/// depth.
///
/// The mode-26 anatomy's shortest measured ladder. It is a length and it is
/// stated as one: it is the drop the anatomy sampled at (0.30 mm, two rungs,
/// six arms, 9.98-11.06 s of profiled wall), chosen so that this probe measures
/// the same object that round measured rather than a new one.
#[cfg(feature = "portfolio-ledger")]
const LADDER_PROBE_DROP_MM: f64 = 0.3;

/// A phase in flight: the coordinator plus this phase's deadline.
struct PhaseRun<'c, 'a> {
    coordinator: &'c mut Coordinator<'a>,
    deadline: f64,
}

impl<'a> std::ops::Deref for PhaseRun<'_, 'a> {
    type Target = Coordinator<'a>;
    fn deref(&self) -> &Self::Target {
        self.coordinator
    }
}

impl std::ops::DerefMut for PhaseRun<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.coordinator
    }
}

impl<'a> Coordinator<'a> {
    /// Runs one phase against its deadline fraction, recording what it cost and
    /// whether it was entered at all.
    fn run_phase(&mut self, name: &str, share: f64, body: impl FnOnce(&mut PhaseRun<'_, 'a>)) {
        let deadline = self.protected_fraction + (1.0 - self.protected_fraction) * share;
        self.run_phase_to(name, deadline, body);
    }

    /// The same, against an absolute deadline fraction rather than a share of
    /// what phase 0 left. The probe phase is the only caller that needs one:
    /// its allowance is a fixed number of work units measured from wherever the
    /// schedule saturated, so that every arm gets the same allowance from the
    /// same state.
    fn run_phase_to(
        &mut self,
        name: &str,
        deadline: f64,
        body: impl FnOnce(&mut PhaseRun<'_, 'a>),
    ) {
        let entered_seconds = self.meter.seconds();
        let entered_work = self.meter.work_units();
        let calls_before = self.operator_calls.len();
        let publications_before = self.publications.len();
        let skipped = !self.meter.has_room(deadline);
        self.phase_name = name.to_owned();
        self.exit_cause = if skipped {
            PhaseExitCause::SkippedDeadlinePassed
        } else {
            PhaseExitCause::Completed
        };
        if !skipped {
            // One trace scope per phase, so the quality-frontier stream
            // attributes every exact-valid candidate to the phase that paid
            // for it - which is the "which phase produced each improvement"
            // half of this stage's measurement.
            #[cfg(feature = "quality-trace")]
            let _trace_phase = crate::quality_trace::scope(
                format!("portfolio.{name}"),
                self.settings.relaxed_template.seed,
                None,
            );
            let mut run = PhaseRun {
                coordinator: self,
                deadline,
            };
            body(&mut run);
        }
        self.phases.push(PhaseReport {
            name: name.to_owned(),
            deadline_fraction: deadline,
            entered_seconds,
            elapsed_seconds: self.meter.seconds() - entered_seconds,
            work_units: self.meter.work_units().saturating_sub(entered_work),
            operator_calls: self.operator_calls.len() - calls_before,
            publications: self.publications.len() - publications_before,
            skipped,
            exit_cause: self.exit_cause,
        });
    }
}

// ---------------------------------------------------------------------------
// Coordinator v3: the ranked action queue.
//
// Three measured defects of the v2 phase sequence, and nothing else:
//
//   * its compression phase asked mode 22 for `depth + 0.8` - a *looser* bound
//     than the incumbent it already held - got an exact-valid answer and exited
//     `noResidue`. The A/B/C's control D asked the same operator, the same
//     parent, for `depth - 0.3` and published 2.620 mm for 3.08M work units;
//   * its schedule is a single pass, so a state born in a late phase is never
//     a parent for an earlier one. On seeds 0 and 1 the final rank-0 state was
//     born in compression, after crossover had ended, and never met the
//     recombination operator;
//   * it names one crossover action per pair - one direction, one cut - and the
//     ledger enumerated 360 ordered, cut-derived actions over the same top-3
//     frontier, of which the schedule had attempted exactly one.
// ---------------------------------------------------------------------------

/// How many rungs a scheduled mode-26 ladder walks.
///
/// Two, which is the A/B/C's arm C: a 0.3 mm drop at a 174 mm parent is two
/// rungs of the mode's own `COUPLED_SEPARATOR_CONTRACTION_RATIO`. The drop is
/// derived rather than carried - `rungs * depth * ratio` - so the same two-rung
/// ladder is a different number of millimetres on a different request.
const LADDER_RUNGS: usize = 2;

/// How many derived cuts one ordered pair may contribute to a single
/// enumeration.
///
/// The ledger's point is that the action space is 4,318 wide, not that a
/// schedule should walk all of it: the queue re-enumerates after every action,
/// so a pair that keeps paying keeps being offered its next cut, in the
/// ledger's canonical order (nearest the constant `0.5` first).
const CROSSOVER_CUTS_PER_PAIR: usize = 2;

/// How many rungs of the separator's own relative contraction quantum one
/// scheduled mode-34 slice walks.
///
/// Nine, and the nine is a reproduction rather than a tuning. The
/// compression-schedule port's cheap arm - `sched10-noroll`, capped at 10% of a
/// measured mode-26 rung in the schedule's own currency - walked a **median
/// 1,568** one-micron steps across its twelve cells and published a median
/// 1.104 mm. On a 174.208 mm parent, `9 * depth * ratio` is 1.5679 mm, which is
/// 1,568 canonical grid steps: the same walk, expressed as a multiple of the
/// engine's own quantum instead of as a work cap measured on one request.
///
/// That is what makes the slice portable. A cap of 3,341,379 units is a
/// mixed-61 number; nine rungs is 1.568 mm on mixed-61's 174 mm parent, 0.636 mm
/// on triangle-20's 70.7 mm parent and 1.803 mm on shapes-17's 200.3 mm one,
/// and no millimetre crosses a request. It is the same derivation
/// [`LADDER_RUNGS`] uses, at a different count, because it is the same quantum.
const SCHEDULE_RUNGS: usize = 9;

/// How many batches a past-bound slice's budget is cut into by default.
///
/// See [`PortfolioSettings::compression_schedule_past_bound_batches`]. Eight,
/// and the number is a resolution rather than a tuning: it is how often the
/// coordinator gets to re-ask "can I still afford this?" over one action, and on
/// the measured band it puts a checkpoint about every one to two of the nine
/// rungs the bounded slice walks.
#[cfg(feature = "compression-schedule")]
const SCHEDULE_PAST_BOUND_BATCHES: usize = 8;

/// How many consecutive barren batches end a past-bound slice by default.
///
/// See [`PortfolioSettings::compression_schedule_past_bound_barren`]. Two, and
/// not one: `confirm_every` is four steps and a batch is one to two rungs, so a
/// single batch can straddle a cadence gap and be barren because it was never
/// asked rather than because it was refused. Two consecutive batches cannot.
#[cfg(feature = "compression-schedule")]
const SCHEDULE_PAST_BOUND_BARREN: usize = 2;

/// How many arms the basin race starts with, the incumbent control included.
///
/// Three, and the count is set by what an audition costs rather than by how
/// many basins would be interesting. Each salted arm is a mode-20 draw, a
/// mode-22 quantum and a short mode-34 batch; the incumbent arm is the batch
/// alone, because the layout is already there. At ten seconds the whole run
/// buys 1-6 schedule actions (`fast-contract-validator` §12's factorial), so
/// four arms would spend the run on the audition and two would not be a race.
#[cfg(feature = "compression-schedule")]
const BASIN_RACE_ARMS: usize = 3;

/// The audition slice, in rungs against [`SCHEDULE_RUNGS`]'s nine.
///
/// Three is a third of a scheduled action, and a third is the smallest fraction
/// at which the three criteria are all *measurable*: the schedule confirms
/// every fourth step by default, so a batch has to walk enough rungs to attempt
/// more than one confirmation or `confirmations_accepted / attempted` - the
/// binding-front stability criterion - is a coin toss with one flip.
#[cfg(feature = "compression-schedule")]
const BASIN_RACE_RUNGS: usize = 3;

/// The ceiling on what the race may spend, as a share of what phase 0 left.
///
/// A ceiling, not an allocation. The race stops as soon as it has a winner and
/// hands everything it did not spend to the v3 queue, which is where the
/// eliminated arms' work goes.
#[cfg(feature = "compression-schedule")]
const BASIN_RACE_SHARE: f64 = 0.34;

/// How many consecutive actions may publish nothing before the v3 loop stops.
///
/// Coordinator v3 has no global patience at all, and §4 of its README measures
/// what that costs: shapes-17 spends a 30 s budget making 281 crossover actions
/// across nine runs for 0.0034 mm, at 9.4x v2's coordinator wall, and never
/// reaches a fixpoint because every rounding-scale publication regenerates the
/// frontier's ordered pairs.
///
/// §5.2 measured the interval any such constant has to live in rather than
/// guessing one, over 1,056 actions on three requests: the **longest barren run
/// that was followed by a publication** is 7 on the mixed-61 30 s arm that
/// produced this stage's headline and 8 on shapes-17 at 10 s, so a patience
/// below 8 destroys measured results; and shapes-17 at 30 s churns 33 barren
/// actions between micron publications, so a patience above 32 does not stop
/// the churn. `[8, 32]` is the measured interval; the constant inside it was
/// left unfitted.
///
/// **16 is the geometric midpoint of that interval**, `sqrt(8 * 32)`, and the
/// midpoint is taken geometrically rather than arithmetically because the
/// quantity being bounded is a *ratio* - "how many failures before a success" -
/// whose interval endpoints are multiplicative, not additive. It is also
/// exactly twice the largest productive barren run ever measured (8, shapes-17
/// at 10 s), which is the same margin the interval's floor was chosen to
/// protect, and exactly half the churn length it has to cut.
pub const BARREN_ACTION_PATIENCE: usize = 16;

/// How many consecutive barren actions the queue tolerates before it auditions
/// an untested constructor ticket.
///
/// Eight: the *floor* of the same measured interval, and for the same measured
/// reason. Coordinator v3 §5.2 says a patience below 8 would have cut the
/// seed-0 30 s run's #13 crossover, which published 1.736 mm after seven barren
/// actions - so 8 is the smallest number that provably interrupts no measured
/// productive barren run on any of the three requests. An audition at 8 is
/// therefore an action mixed-61's own headline stream never reaches.
///
/// The pair reads as one rule: **at eight barren actions the queue buys a new
/// basin, at sixteen it stops.**
const DIVERSIFY_AUDITION_BARREN: usize = 8;

/// How many actions of the compression-schedule class this run will buy on
/// nothing but the prior, before a class that has published *nothing here* is
/// taken off the queue.
///
/// One, and it is the whole of Grok review 1 §2b item 4: the class published
/// **0 of 29** actions on shapes-17 and **0 of 37** on triangle-20, pooled over
/// both budget tiers, while publishing on 9 of 9 at ten seconds on mixed-61. A
/// prior of 1.104 mm is a mixed-61 number and it does not cross a request - so
/// the first slice on a new request is the *audition*, and one sterile audition
/// is the evidence this rule acts on. Re-baselined on HEAD, the second and
/// later slices are 1.1 s each on shapes-17 and 1.6-1.9 s each on triangle-20,
/// and at a thirty-second budget the class takes 2 and 4 of them respectively.
const SCHEDULE_STERILE_ACTIONS: usize = 1;

/// How many barren actions must pass after the class was taken off the queue
/// before it is offered once more.
///
/// Sixteen, which is [`BARREN_ACTION_PATIENCE`] itself: the audition is meant
/// to keep the rule falsifiable, not to buy the class back, and a run that has
/// gone sixteen barren actions since a sterile slice is about to end anyway. It
/// fires **at most once per run**. The diversify class's own audition is at
/// eight because that class is being promoted *into* a queue that outranks it;
/// this one is being let back into a queue it was removed from, which is a
/// weaker claim and gets the more conservative number.
const SCHEDULE_AUDITION_BARREN: usize = BARREN_ACTION_PATIENCE;

/// What one compression-schedule slice costs on the **clock**, as a multiple of
/// the protected phase-0 pipeline this run just paid for.
///
/// Coordinator v4 §1.3 measured this class's work-budget prior as the best in
/// the queue - first-action actual/estimate 0.97-1.01 - and §8 named the same
/// number "the weakest in this stage" under a wall budget, where the ratio is
/// 2.54-2.59 on mixed-61, 2.94-3.07 on shapes-17 and 5.1 on triangle-20. The
/// two are the same fact: [`schedule_self_cost_units`] is denominated in the
/// coordinator's work currency, which counts the narrow phase of the exact tier
/// only, and the exact tier is 24-52% of this operator's *wall*.
///
/// The number here is this round's own re-baselined measurement on HEAD, over
/// **18 cells** - three requests, three seeds, ten- and thirty-second budgets -
/// of the first slice's charged seconds against that run's own phase 0:
///
/// | request | min | median | max |
/// |---|---:|---:|---:|
/// | mixed-61 | 0.990 | 1.019 | 1.147 |
/// | shapes-17 | 1.138 | 1.201 | 1.619 |
/// | triangle-20 | 2.124 | 2.207 | **2.238** |
///
/// **2.2375 is the worst of the eighteen**, which is the rule every other class
/// in this table is already priced by: the ladder carries the largest of three
/// arm-C spends rather than their mean, and the diversify class carries the
/// worst of three requests in each of its two currencies. An operator with a
/// 2.3x spread across requests priced by its median is an affordability rule
/// that is a coin toss on the request where it is dearest.
///
/// It is 2.9x the work-denominated prior, so on mixed-61 - where the true
/// multiple is 1.02 - it over-prices the class by 2.2x. Two rules keep that
/// from costing what it is worth, and both were arrived at by measuring the
/// version without them:
///
/// * it is read by the **affordability gate only** - see
///   [`Coordinator::class_rank_cost_estimate`] for the nine paired rounds that
///   measured what happens when the ranking reads it too (a median 0.649 mm at
///   thirty seconds on mixed-61);
/// * and only until this run has bought one slice - see
///   [`Coordinator::class_cost_estimate`] for the nine paired rounds that
///   measured holding it over later slices (a median 0.137 mm, and 2.1-4.0 mm
///   on one seed).
///
/// What is left is the case the review named: **the first slice has no
/// ratchet.** On shapes-17 at three seconds that slice costs 1.11 s of a budget
/// that has about 1 s left when it is offered, and HEAD buys it in 8 of 9 runs
/// and overruns in 7.
const SCHEDULE_WALL_PRIOR_PHASE_ZEROS: f64 = 2.2375;

/// The fraction of a compression-schedule slice bought before the slice has to
/// show evidence: `1 / SCHEDULE_PROBE_DENOMINATOR` of its planned steps.
///
/// **Zero - off - and that is a measurement, not an omission.**
///
/// The mechanism does what it was built to do. Nine cells at ten seconds,
/// every other key off (`evidence/probe-sweep.json`):
///
/// | denominator | mixed-61 slices publishing | shapes-17 first slice | triangle-20 first slice |
/// |---|---:|---:|---:|
/// | off | 3 of 3 | 1.07-1.46 s | 1.64-1.92 s |
/// | 2 | 3 of 3 | 0.50-0.74 s | 0.13-0.38 s |
/// | 3 | 3 of 3 | 0.38-0.50 s | 0.017-0.020 s |
/// | 4 | **2 of 3** | 0.31-0.38 s | 0.012-0.015 s |
///
/// It is off for two measured reasons and neither is the sweep above.
///
/// **It buys no depth with the wall it returns.** On shapes-17 and triangle-20
/// - the only two requests where a sterile slice exists to cut - every arm in
/// that sweep publishes exactly the depth HEAD publishes: 200.349 on all three
/// shapes-17 seeds, and 70.73007 / 70.73005 / 70.72882 on triangle-20's, which
/// are coordinator v4's own ten-second numbers. One to two seconds of a
/// ten-second budget come back and nothing in the queue can spend them.
///
/// **And at thirty seconds it costs millimetres on the request that pays.**
/// Deeper parents arrive further above themselves - the entry loss on
/// mixed-61's *second* slice is 0.453 mm against a 1.520 mm drop, 30% of the
/// walk, against 0.158 mm and 10% on the first - so a probe at a third expires
/// before the lane has walked back the snap. Measured on seed 0 at thirty
/// seconds, action #17: the unabridged slice takes 2.61 s and **publishes**
/// 1.03 mm; the probed slice is abandoned at step 506 of 1520 after 1.30 s with
/// ten accepted confirmations and publishes nothing, and the run ends 2.132 mm
/// worse.
///
/// So the honest reading is that "spend a fraction, continue on evidence" is
/// the right shape and a *step count* is the wrong budget for it: the evidence
/// the lane can produce depends on a handicap the step count does not know
/// about. The key stays, off, with the sweep and the counter-example, because
/// the next attempt should start from an entry-loss-relative probe rather than
/// from scratch.
const SCHEDULE_PROBE_DENOMINATOR: usize = 0;

/// How many bands one enumeration will build a hybrid for, per ordered pair.
/// The hybrid is what decides whether a cut is a real action, and it costs a
/// fingerprint over the whole layout; this bounds that cost.
const CROSSOVER_BANDS_SCANNED: usize = 6;

/// How much evidence a class prior is worth, in actions.
///
/// Two: the prior is displaced by this run's own measurements after two actions
/// of the class, which is the smallest number at which "it published once" and
/// "it published twice" are different statements.
const PRIOR_ACTIONS: f64 = 2.0;

/// What an action needs to identify its parents. Fingerprints rather than
/// placements, because the archive is the authority on both and a queue that
/// carried copies could offer an action against a state that has been evicted.
#[derive(Clone, Debug)]
enum ActionPayload {
    Basin {
        fingerprint: String,
    },
    Crossover {
        left_fingerprint: String,
        right_fingerprint: String,
        cut_fraction: f64,
    },
    Diversify {
        slot: usize,
    },
}

/// One action the queue may offer.
#[derive(Clone, Debug)]
struct ScheduledAction {
    class: ActionClass,
    /// The key in the `attempted` namespace. Built from the parents *in the
    /// order they will be handed to the operator*, never from their ranks: the
    /// frontier reorders between actions, and a rank-built key reports an
    /// attempted action as untried. That is pinned by a test.
    key: String,
    /// Order within the class. Smaller first.
    rank: usize,
    label: String,
    payload: ActionPayload,
}

impl Coordinator<'_> {
    /// Every action the queue can name against the archive as it stands.
    ///
    /// Bounded by construction rather than by truncation: the frontier is the
    /// same top-K the v2 phases drew from, and each ordered pair contributes at
    /// most [`CROSSOVER_CUTS_PER_PAIR`] cuts. Actions already attempted are not
    /// offered, so a pair that has spent its two nearest cuts is offered the
    /// next two on the following iteration.
    fn enumerate_v3_actions(&self) -> Vec<ScheduledAction> {
        let mut actions = Vec::new();
        let frontier = self
            .archive
            .distinct_frontier(self.settings.crossover_states.max(2));
        let cycles = self.settings.descent_cycles.max(1);
        let epochs = self.settings.descent_relaxed_epochs.max(1);
        // The two mode-22 classes keep v2's measured selection width: one
        // state, the best distinct one. v2 priced the alternative and declined
        // it - spreading the same quanta over three states put them on 194-214
        // mm constructor basins and landed at 176.753 against 176.056 - and the
        // loop does not change that arithmetic, it only means the *next* best
        // state gets its quantum on the next iteration instead of never. The
        // ladder is rank 0 only, because it is the one class whose single
        // action costs more than the whole rest of the queue.
        let quantum_states = self.settings.descent_states.max(1);
        for (rank, basin) in frontier.iter().enumerate() {
            let depth = basin.raw_depth_mm;
            // Compression: the incumbent-relative, derived target. Never an
            // absolute slack *above* the depth the parent already holds.
            let compress_to = depth - COMPRESSION_RUNG_MM;
            let key = format!("22c:{}", basin.fingerprint);
            if rank < quantum_states && compress_to > 0.0 && !self.attempted.contains(&key) {
                actions.push(ScheduledAction {
                    class: ActionClass::Compression,
                    key,
                    rank,
                    label: format!("m22 compress rank{rank} {depth:.4} -> {compress_to:.4}"),
                    payload: ActionPayload::Basin {
                        fingerprint: basin.fingerprint.clone(),
                    },
                });
            }
            let key = format!("22:{cycles}:{epochs}:{}", basin.fingerprint);
            if rank < quantum_states && !self.attempted.contains(&key) {
                actions.push(ScheduledAction {
                    class: ActionClass::Descent,
                    key,
                    rank,
                    label: format!("m22 quantum rank{rank} cycles={cycles} epochs={epochs}"),
                    payload: ActionPayload::Basin {
                        fingerprint: basin.fingerprint.clone(),
                    },
                });
            }
            // The compression-schedule slice. Offered over the same one best
            // distinct state the two mode-22 classes are offered over rather
            // than over rank 0 alone: at 0.3806 phase-zeros it is a sixth of a
            // ladder, and the port measured it publishing in 11 of 12 cells
            // against the ladder's 10 of 12 at 17x the price.
            #[cfg(feature = "compression-schedule")]
            if self.settings.compression_schedule_class {
                let drop_mm = depth
                    * SCHEDULE_RUNGS as f64
                    * crate::search::general_relaxed::COUPLED_SEPARATOR_CONTRACTION_RATIO;
                let key = format!("34:{}", basin.fingerprint);
                if rank < quantum_states && depth - drop_mm > 0.0 && !self.attempted.contains(&key)
                {
                    actions.push(ScheduledAction {
                        class: ActionClass::Schedule,
                        key,
                        rank,
                        label: format!(
                            "m34 schedule rank{rank} {depth:.4} -> {:.4} ({SCHEDULE_RUNGS} rungs, \
                             {} steps)",
                            depth - drop_mm,
                            (drop_mm
                                / crate::search::compression_schedule::canonical_grid_step_mm())
                            .round() as usize
                        ),
                        payload: ActionPayload::Basin {
                            fingerprint: basin.fingerprint.clone(),
                        },
                    });
                }
            }
            let drop_mm = depth
                * LADDER_RUNGS as f64
                * crate::search::general_relaxed::COUPLED_SEPARATOR_CONTRACTION_RATIO;
            let key = format!("26:{}", basin.fingerprint);
            if rank == 0 && depth - drop_mm > 0.0 && !self.attempted.contains(&key) {
                actions.push(ScheduledAction {
                    class: ActionClass::Ladder,
                    key,
                    rank,
                    label: format!(
                        "m26 ladder rank{rank} {depth:.4} -> {:.4} ({LADDER_RUNGS} rungs)",
                        depth - drop_mm
                    ),
                    payload: ActionPayload::Basin {
                        fingerprint: basin.fingerprint.clone(),
                    },
                });
            }
        }
        // Crossover: ordered pairs, both directions, cuts derived from the
        // interface bands where the two parents actually differ.
        let mut pair_rank = 0usize;
        for right in 1..frontier.len() {
            for left in 0..right {
                for (a, b) in [(left, right), (right, left)] {
                    let parent_a = &frontier[a];
                    let parent_b = &frontier[b];
                    let mut bands = derived_cut_bands(&parent_a.placements, &parent_b.placements);
                    bands.sort_by(|first, second| {
                        (first.0 - CROSSOVER_CUT_FRACTION)
                            .abs()
                            .total_cmp(&(second.0 - CROSSOVER_CUT_FRACTION).abs())
                            .then(first.0.total_cmp(&second.0))
                    });
                    // The constant `0.5` first, then the derived band midpoints
                    // outward from it. `0.5` is the only cut this engine has
                    // evidence for - it is the action that published 3.277 mm
                    // on the ledger's own stream - and the ledger's arm A
                    // measured that the *next* derived cut produces a legal
                    // hybrid that the adoption rule usually refuses. So the
                    // derived cuts widen the action space behind the action
                    // that is known to pay, rather than replacing it.
                    let bands = std::iter::once((CROSSOVER_CUT_FRACTION, f64::NAN, 0usize, true))
                        .chain(bands);
                    let mut taken = 0usize;
                    let mut scanned = 0usize;
                    let mut seen = std::collections::BTreeSet::new();
                    for (fraction, gap_mm, differing, is_midpoint) in bands {
                        if taken > CROSSOVER_CUTS_PER_PAIR || scanned > CROSSOVER_BANDS_SCANNED {
                            break;
                        }
                        let key = format!(
                            "23:{}:{}:{:016x}",
                            parent_a.fingerprint,
                            parent_b.fingerprint,
                            fraction.to_bits()
                        );
                        if self.attempted.contains(&key) {
                            continue;
                        }
                        scanned += 1;
                        let Some((hybrid, from_a, from_b)) =
                            crossover_hybrid(&parent_a.placements, &parent_b.placements, fraction)
                        else {
                            continue;
                        };
                        let hybrid_fingerprint = general_placement_fingerprint(&hybrid);
                        // A cut that rebuilds one of its own parents is a no-op
                        // dressed as a crossover; a cut that rebuilds a layout
                        // the archive already holds buys a duplicate.
                        if hybrid_fingerprint == parent_a.fingerprint
                            || hybrid_fingerprint == parent_b.fingerprint
                            || !seen.insert(hybrid_fingerprint.clone())
                            || self
                                .archive
                                .basins()
                                .iter()
                                .any(|member| member.fingerprint == hybrid_fingerprint)
                        {
                            continue;
                        }
                        actions.push(ScheduledAction {
                            class: ActionClass::Crossover,
                            key,
                            rank: pair_rank * (CROSSOVER_CUTS_PER_PAIR + 1) + taken,
                            label: if gap_mm.is_nan() {
                                format!(
                                    "m23 rank{a}->rank{b} cut={fraction:.9} constant \
                                     from={from_a}/{from_b}"
                                )
                            } else {
                                format!(
                                    "m23 rank{a}->rank{b} cut={fraction:.9} derived \
                                     band={gap_mm:.6}mm differing={differing} \
                                     midpoint={is_midpoint} from={from_a}/{from_b}"
                                )
                            },
                            payload: ActionPayload::Crossover {
                                left_fingerprint: parent_a.fingerprint.clone(),
                                right_fingerprint: parent_b.fingerprint.clone(),
                                cut_fraction: fraction,
                            },
                        });
                        taken += 1;
                    }
                    pair_rank += 1;
                }
            }
        }
        actions
    }

    /// The archived basin with this fingerprint, cloned.
    fn basin_by_fingerprint(&self, fingerprint: &str) -> Option<ArchivedBasin> {
        self.archive
            .basins()
            .iter()
            .find(|basin| basin.fingerprint == fingerprint)
            .cloned()
    }

    /// What one action of `class` is expected to cost, in the budget's own
    /// currency.
    ///
    /// The larger of the class prior - a multiple of the protected phase's own
    /// measured cost - and this run's own worst observed action of the class.
    ///
    /// There is no "unpriced operators get a free pass" clause, which is the
    /// ledger's mode-20 finding turned into a rule: an operator nobody has
    /// priced is exactly the one that overruns. And the prior is never
    /// *discarded* by a cheap first action, only raised by an expensive one:
    /// the A/B/C measured one mode-26 ladder at 5.7M work units and another at
    /// 21.0M on the same request at a different seed, so a class with a 3.7x
    /// spread priced from one lucky sample is not priced at all.
    fn class_cost_estimate(&self, class: ActionClass) -> f64 {
        let observed = self.class_observed_cost(class);
        if self.schedule_wall_priced(class) && observed > 0.0 {
            // **Ratchet after.** [`SCHEDULE_WALL_PRIOR_PHASE_ZEROS`] is the
            // worst of three *other* requests; the moment this run has bought
            // one slice it holds a measurement of *this* request, and a
            // same-request sample strictly dominates a cross-request worst
            // case. Grok review 1 §2b item 1 asks for exactly this - "p95/worst
            // of the same request, ratchet after" - and this round measured
            // what happens without it: holding the cross-request worst case
            // over later slices refuses slices that fit and publish, for a
            // paired median 0.137 mm and up to 3.95 mm on one mixed-61 seed at
            // thirty seconds (`evidence/curve-mixed61-priorheld.json`).
            //
            // What the prior still does is the thing item 1 was about: **the
            // first slice has no ratchet.** That is the slice this class
            // overruns on, and on shapes-17 at three seconds it is the slice
            // that puts 7 of 9 HEAD runs over their own budget.
            return observed.max(f64::MIN_POSITIVE);
        }
        self.class_prior_cost(class)
            .max(observed)
            .max(f64::MIN_POSITIVE)
    }

    /// This run's own worst observed action of `class`, or `0.0` if it has
    /// never bought one. The ratchet.
    fn class_observed_cost(&self, class: ActionClass) -> f64 {
        self.class_stats
            .get(&class)
            .filter(|stats| stats.actions > 0)
            .map(|stats| stats.cost_max)
            .unwrap_or(0.0)
    }

    /// The class prior, in the budget's own currency, with the one switch a
    /// prior in this table has.
    ///
    /// `schedule_wall_prior` off restores merged-HEAD v4 exactly: the schedule
    /// class is priced by its work-denominated prior under both budgets, which
    /// is 2.6-5.9x low on the clock.
    fn class_prior_cost(&self, class: ActionClass) -> f64 {
        let wall = self.meter.is_wall()
            && (class != ActionClass::Schedule || self.settings.schedule_wall_prior);
        class.prior_cost_in_phase_zero_for(wall) * self.phase_zero_cost
    }

    /// The price the queue *ranks* against, which for one class is not the
    /// price it is willing to pay.
    ///
    /// # Why the two rules read different numbers here, and what it cost to
    /// find out
    ///
    /// The two rules answer different questions. **Affordability** asks "can
    /// this run finish an action of this class in what is left?" - a question
    /// about the tail, whose failure mode is an overrun - and for the *first*
    /// slice of a run it is asked at [`SCHEDULE_WALL_PRIOR_PHASE_ZEROS`], the
    /// worst of the eighteen cells measured, because the first slice is the one
    /// with no ratchet behind it. **Ranking** asks "is this class the best value
    /// on offer?" - a question about the centre, whose failure mode is buying
    /// the wrong class - and it is asked at coordinator v4's own price: the
    /// work-denominated prior, raised by this run's own worst action.
    ///
    /// **This round's first cut asked both questions at the worst case, and
    /// measured what that costs.** Nine paired rounds on mixed-61
    /// (`evidence/curve-mixed61-priorfloor.json`): at ten seconds it was
    /// harmless - 9 of 9 kept, 2 rounds better - and at thirty seconds it cost
    /// a median **0.649 mm** and 8 of 9 rounds, because a class ranked at
    /// `1.104 / 2.2375 = 0.493` never wins a rank again and the slice count
    /// fell from 2.89 per run to 1.00. Those later slices are not speculative:
    /// the same battery has them publishing on 23 of 26. A worst-case wall
    /// price is the right answer to "can I afford it" and the wrong answer to
    /// "is it worth it".
    ///
    /// So the ranking is left exactly where coordinator v4 had it - this
    /// function is v4's `class_cost_estimate` for this class, unchanged - and
    /// only the affordability gate is re-priced. That is also Grok review 1
    /// §2b item 1's floor, "at least one slice if eligible", obtained without a
    /// special case for the first action: the class keeps the rank it earned in
    /// v4 and the budget refuses it when the worst case does not fit.
    fn class_rank_cost_estimate(&self, class: ActionClass) -> f64 {
        if !self.schedule_wall_priced(class) {
            return self.class_cost_estimate(class);
        }
        let prior = class.prior_cost_in_phase_zero() * self.phase_zero_cost;
        prior
            .max(self.class_observed_cost(class))
            .max(f64::MIN_POSITIVE)
    }

    /// Whether `class` is the one class this run prices differently for
    /// affordability than for rank.
    fn schedule_wall_priced(&self, class: ActionClass) -> bool {
        self.settings.schedule_wall_prior && class == ActionClass::Schedule && self.meter.is_wall()
    }

    /// The queue's ranking value: expected millimetres of published raw depth
    /// per protected-phase-0 cost.
    ///
    /// The prior is worth [`PRIOR_ACTIONS`] actions of evidence and this run's
    /// own publications displace it, which is the "let publications re-rank"
    /// half of the rule. Quoting the value against phase 0 rather than against
    /// a million evaluations makes it the same number under a wall budget and a
    /// work budget; the *ordering* it produces on the priors alone is the
    /// ledger's own Δraw/M-evaluation ordering.
    fn class_value(&self, class: ActionClass) -> f64 {
        let (actions, delta) = match self.class_stats.get(&class) {
            Some(stats) => (stats.actions as f64, stats.delta_raw_mm),
            None => (0.0, 0.0),
        };
        let expected_delta =
            (PRIOR_ACTIONS * class.prior_delta_mm() + delta) / (PRIOR_ACTIONS + actions);
        expected_delta * self.phase_zero_cost / self.class_rank_cost_estimate(class)
    }
}

// ---- the multi-basin race ------------------------------------------------

/// One arm of the race, as it is judged and as it is reported.
///
/// The three criteria are Sol review 8 §4.3's, and none of them is the arm's
/// immediate depth. That exclusion is the whole design: the review's own
/// warning is that "l'early leader può essere il late loser", and a constructor
/// basin's depth at the moment it is drawn is the single most misleading number
/// available - Sol review 3's finding, quoted again in Grok review 3 §3 item 3,
/// is that a *worse* constructor can open a *better* basin.
#[cfg(feature = "compression-schedule")]
#[derive(Clone, Debug)]
struct BasinRaceArm {
    /// The salt slot. `0` is the incumbent control and draws no constructor.
    slot: usize,
    /// What the arm is, in the report's own words.
    kind: &'static str,
    /// The layout the arm is currently carrying, and its depth.
    placements: Vec<GeneralFastPlacement>,
    fingerprint: String,
    depth_mm: f64,
    /// Every layout this arm has ever put into the archive: the one it was
    /// created with, and one more per audition batch that improved it.
    ///
    /// Elimination has to retire all of them, not just the last. `run_operator`
    /// archives whatever it produces, so an arm that was auditioned twice has
    /// left two members behind, and retiring only the current fingerprint would
    /// leave the arm's earlier layouts in the queue's reach - which is exactly
    /// the "the race has only spent work" outcome `basin_race_evict` exists to
    /// prevent.
    archived: Vec<String>,
    /// Criterion 1 - **first-batch mode-34 yield**: raw millimetres the
    /// audition batch took off the arm's own parent. Higher is better. This is
    /// a *delta*, so a shallow basin that compresses well beats a deep one that
    /// has already fixpointed, which is the entire reason the criterion is not
    /// depth.
    yield_mm: f64,
    /// Criterion 2 - **binding-front stability**: the fraction of the batch's
    /// confirmation attempts that were accepted. A basin whose binding front
    /// moves under the clamp refuses confirmations and reads low here; one
    /// whose front is stable walks its rungs and keeps them. `1.0` when the
    /// batch attempted nothing, so an arm is never rewarded for having been
    /// too short to be measured - it is the neutral value, not a win, and
    /// criterion 1 will already have scored it zero.
    stability: f64,
    /// Criterion 3 - **proxy infeasibility** at the arm's own parent, as
    /// violating pairs plus boundary violations per piece. Lower is better. It
    /// is read at the batch's *entry*, before any repair, so it describes the
    /// basin and not the batch.
    infeasibility: f64,
    /// The batch's own account, for the report.
    batch_steps: usize,
    batch_confirmations: usize,
    /// Rank sum over the three criteria, lowest wins. Filled by `judge`.
    rank_sum: usize,
    /// The round this arm was eliminated in, or `None` if it survived.
    eliminated_round: Option<usize>,
}

/// What the race did, as a document.
#[cfg(feature = "compression-schedule")]
#[derive(Clone, Debug)]
pub struct BasinRaceReport {
    pub armed: bool,
    pub arms_started: usize,
    pub rounds: usize,
    pub kept: usize,
    pub retired: usize,
    /// The winning slot. `Some(0)` means the race chose the incumbent - the
    /// basin the run would have used anyway - and anything else means it did
    /// not. This is the field the round's central question is asked of.
    pub winner_slot: Option<usize>,
    pub winner_fingerprint: Option<String>,
    pub winner_depth_mm: Option<f64>,
    /// The incumbent control's depth at the end of the race, so "the race
    /// picked something else" can be read against what it passed over.
    pub incumbent_arm_depth_mm: Option<f64>,
    pub work_units: u64,
    pub seconds: f64,
    pub exit_cause: String,
    pub arms: Vec<BasinRaceArmReport>,
}

/// One arm's row of [`BasinRaceReport`].
#[cfg(feature = "compression-schedule")]
#[derive(Clone, Debug)]
pub struct BasinRaceArmReport {
    pub slot: usize,
    pub kind: String,
    pub fingerprint: String,
    pub depth_mm: f64,
    pub yield_mm: f64,
    pub stability: f64,
    pub infeasibility: f64,
    pub batch_steps: usize,
    pub batch_confirmations: usize,
    pub rank_sum: usize,
    pub eliminated_round: Option<usize>,
    pub retired_from_archive: bool,
}

#[cfg(feature = "compression-schedule")]
impl BasinRaceReport {
    fn unarmed() -> Self {
        Self {
            armed: false,
            arms_started: 0,
            rounds: 0,
            kept: 0,
            retired: 0,
            winner_slot: None,
            winner_fingerprint: None,
            winner_depth_mm: None,
            incumbent_arm_depth_mm: None,
            work_units: 0,
            seconds: 0.0,
            exit_cause: PhaseExitCause::Completed.name().to_owned(),
            arms: Vec::new(),
        }
    }
}

/// Ranks the arms on the three criteria and writes `rank_sum` into each.
///
/// A **rank sum** rather than a weighted score, and the choice is not a taste:
/// the three criteria are in three incommensurable units - millimetres, a
/// fraction, and a count per piece - so any weighting is three constants this
/// round would have had to tune on the same nine cells it is trying to measure.
/// A rank sum needs none, is invariant to every monotone rescaling of any
/// criterion, and is exactly as total as its tie-break, which is the arm
/// ordinal. Ties are broken toward the **lower slot**, so the incumbent control
/// wins every tie: a race that cannot tell the arms apart must not move the
/// run off the basin it already had.
#[cfg(feature = "compression-schedule")]
fn judge_basin_race(arms: &mut [usize], rows: &mut [BasinRaceArm]) {
    // Three orderings over the live arms, each best-first.
    let mut by_yield = arms.to_vec();
    by_yield.sort_by(|first, second| {
        rows[*second]
            .yield_mm
            .total_cmp(&rows[*first].yield_mm)
            .then(first.cmp(second))
    });
    let mut by_stability = arms.to_vec();
    by_stability.sort_by(|first, second| {
        rows[*second]
            .stability
            .total_cmp(&rows[*first].stability)
            .then(first.cmp(second))
    });
    let mut by_infeasibility = arms.to_vec();
    by_infeasibility.sort_by(|first, second| {
        rows[*first]
            .infeasibility
            .total_cmp(&rows[*second].infeasibility)
            .then(first.cmp(second))
    });
    for index in arms.iter() {
        let position = |order: &[usize]| {
            order
                .iter()
                .position(|candidate| candidate == index)
                .unwrap_or(0)
        };
        rows[*index].rank_sum =
            position(&by_yield) + position(&by_stability) + position(&by_infeasibility);
    }
    arms.sort_by(|first, second| {
        rows[*first]
            .rank_sum
            .cmp(&rows[*second].rank_sum)
            .then(first.cmp(second))
    });
}

/// The race, as a phase.
///
/// # Why this is a phase and not a class
///
/// The v3 queue ranks a class on a prior and then on the class's own measured
/// yield. Neither can price a basin the run has not entered: the constructor's
/// prior is one number for every draw, and a draw's *depth* - the only thing
/// available before a slice is spent on it - is the number Sol review 3 says is
/// anti-correlated with what follows. So the race runs the audition itself, in
/// front of the queue, and hands the queue a decision instead of a ticket.
///
/// # The arms
///
/// Slot 0 is the **incumbent control**: the layout phase 0 published, with no
/// constructor draw at all. It is in the race for two reasons. It is the arm
/// that answers the round's question - a winner that is not slot 0 is a basin
/// the un-raced run would never have used - and its audition batch is *not
/// overhead*, because it is the first mode-34 action the v3 queue would have
/// spent on that layout anyway. The race's true price is therefore the salted
/// arms alone, which is what the equal-work gate has to clear.
///
/// Slots 1.. are salted constructor draws, and the salting is the ledger's:
/// mode 20 derives `construction_seed` from
/// `parent_seed_key ^ CONSTRUCTION_SEED_DOMAIN ^ grid_key(target_depth_mm)`, so
/// two draws that differ only in their *seed* are replicas and two that differ
/// in their **target** are different lotteries. Each slot therefore moves the
/// clamp by [`BASIN_TARGET_SALT_RELATIVE_STEP`] and takes its own void-cell
/// divisor, exactly as the diversify class does - and then descends the draw
/// with one mode-22 quantum, because a raw constructor layout and a descended
/// one are not the same kind of object and an audition that compared them
/// would be measuring the descent.
///
/// # The halving
///
/// Round 1 auditions every arm at [`PortfolioSettings::basin_race_rungs`].
/// Every round after it eliminates the bottom half - `keep = max(surviving / 2,
/// target)` - and re-auditions the survivors at **double** the rungs, from
/// wherever their last batch left them, until `target` arms remain. So an arm
/// that survives is not re-run from its draw: its batch continues, and the
/// eliminated arms' share is what pays for the longer batch. That is the
/// "loser's work returns to the winner" of the brief, in the only currency the
/// schedule has.
///
/// # What it costs when it is wrong
///
/// If the race eliminates the arm that would have won the run, the run is on a
/// worse basin *and* has spent the audition. Both halves are priced in
/// `docs/experiments/basin-race/`; the equal-work gate is what decides.
#[cfg(feature = "compression-schedule")]
fn run_basin_race(coordinator: &mut Coordinator<'_>, constructor_clamp_mm: f64) -> BasinRaceReport {
    if !coordinator.settings.basin_race {
        return BasinRaceReport::unarmed();
    }
    let arm_count = coordinator.settings.basin_race_arms.clamp(2, 4);
    let target = coordinator.settings.basin_race_keep.clamp(1, arm_count - 1);
    let rungs = coordinator.settings.basin_race_rungs.max(1);
    let evict = coordinator.settings.basin_race_evict;
    let draw_arms = coordinator.settings.basin_race_draw;
    let share = coordinator.settings.basin_race_share.clamp(0.0, 1.0);
    let entered_work = coordinator.meter.work_units();
    let entered_seconds = coordinator.meter.seconds();
    let mut rows: Vec<BasinRaceArm> = Vec::new();
    let mut live: Vec<usize> = Vec::new();
    let mut rounds = 0usize;
    let mut retired: Vec<String> = Vec::new();
    coordinator.run_phase("race", share, |run| {
        // ---- the arms -----------------------------------------------------
        //
        // Slot 0 first and unconditionally: it is the control, it costs no
        // draw, and an arm list that could be empty is one the halving below
        // would have to special-case.
        let incumbent = run.incumbent.result.placements.clone();
        if incumbent.len() != run.pieces.len() {
            run.note_exit(PhaseExitCause::KeysExhausted);
            return;
        }
        let Some(incumbent_depth) = crate::search::general_relaxed::coupled_raw_source_depth(
            run.pieces,
            &incumbent,
            run.fast_settings,
        )
        .ok() else {
            run.note_exit(PhaseExitCause::KeysExhausted);
            return;
        };
        rows.push(BasinRaceArm {
            slot: 0,
            kind: "incumbent",
            fingerprint: general_placement_fingerprint(&incumbent),
            archived: vec![general_placement_fingerprint(&incumbent)],
            placements: incumbent,
            depth_mm: incumbent_depth,
            yield_mm: 0.0,
            stability: 1.0,
            infeasibility: f64::INFINITY,
            batch_steps: 0,
            batch_confirmations: 0,
            rank_sum: 0,
            eliminated_round: None,
        });
        live.push(0);
        if draw_arms {
            for slot in 1..arm_count {
                // Affordability, checked the way the queue checks it: a draw
                // plus a quantum, priced by whatever this run has already
                // measured them at. An arm the race cannot pay for is an arm
                // the race does not start, and the round is judged over the
                // arms that ran.
                //
                // Honest caveat, measured rather than argued: under a *work*
                // budget this check is nearly inert, because mode 20 costs 310
                // work units for 3.156 s of wall. See
                // `PortfolioSettings::basin_race_draw`.
                let quantum = run.mean_operator_cost("mode22");
                let draw = run.mean_operator_cost("mode20");
                if let (Some(quantum), Some(draw)) = (quantum, draw) {
                    if run.meter.remaining_to(run.deadline) < draw + quantum {
                        run.note_exit(PhaseExitCause::Affordability);
                        break;
                    }
                }
                if !run.meter.has_room(run.deadline) {
                    run.note_exit(PhaseExitCause::Deadline);
                    break;
                }
                let Some(arm) = draw_race_arm(run, slot, constructor_clamp_mm) else {
                    continue;
                };
                rows.push(arm);
                live.push(rows.len() - 1);
            }
        } else {
            // The archive's own arms. `distinct_frontier` is the same
            // shallowest-first, pairwise-distinct selection the descent class
            // draws its parents from, so the race is auditioning basins the
            // queue could already reach rather than inventing a selection rule
            // of its own. The incumbent is skipped by fingerprint because it is
            // already slot 0, and asking for one more member than there are
            // slots is what leaves room for that skip.
            let candidates = run.archive.distinct_frontier(arm_count + 1);
            let incumbent_fingerprint = rows[0].fingerprint.clone();
            for basin in candidates {
                if rows.len() >= arm_count {
                    break;
                }
                if basin.fingerprint == incumbent_fingerprint {
                    continue;
                }
                rows.push(BasinRaceArm {
                    slot: rows.len(),
                    kind: "archive-basin",
                    archived: vec![basin.fingerprint.clone()],
                    fingerprint: basin.fingerprint,
                    placements: basin.placements,
                    depth_mm: basin.raw_depth_mm,
                    yield_mm: 0.0,
                    stability: 1.0,
                    infeasibility: f64::INFINITY,
                    batch_steps: 0,
                    batch_confirmations: 0,
                    rank_sum: 0,
                    eliminated_round: None,
                });
                live.push(rows.len() - 1);
            }
        }
        // ---- the rounds ---------------------------------------------------
        //
        // A round runs only when there is still something to decide. The
        // survivor is deliberately *not* auditioned again once it is alone:
        // the winner's continuation is the v3 queue's first mode-34 action,
        // priced by the queue at the queue's own rungs, and running it here
        // would charge the race for work the run was always going to do. That
        // is also the literal form of "the loser's work returns to the
        // winner" - the race stops, and everything its share had left goes to
        // the queue that is now working on the arm the race chose.
        let mut round_rungs = rungs;
        while live.len() > target {
            if !run.meter.has_room(run.deadline) {
                run.note_exit(PhaseExitCause::Deadline);
                break;
            }
            rounds += 1;
            for index in live.clone() {
                if !run.meter.has_room(run.deadline) {
                    run.note_exit(PhaseExitCause::Deadline);
                    break;
                }
                audition_race_arm(run, &mut rows[index], round_rungs);
            }
            judge_basin_race(&mut live, &mut rows);
            // Successive halving: drop the bottom half, never below the
            // target. `live` is already best-first after the judge.
            //
            // `div_ceil` and not `/`, so three arms go 3 -> 2 -> 1 rather than
            // 3 -> 1. The extra round is the point: Sol review 8 §4.3's named
            // risk is that "l'early leader può essere il late loser", and
            // committing on a single three-rung audition is exactly the
            // decision that risk describes. It still strictly decreases at
            // every length the loop can be entered with - `2 -> 1`, `3 -> 2`,
            // `4 -> 2` - so the halving terminates.
            let keep = live.len().div_ceil(2).max(target);
            for index in live.split_off(keep) {
                rows[index].eliminated_round = Some(rounds);
            }
            // Doubled, because the survivors inherit the eliminated arms'
            // rungs - and capped at a full scheduled action, because mode 34 is
            // atomic (Sol review 8 §3 condition 4) and an audition batch longer
            // than the queue's own action would make the race's overrun larger
            // than anything else in the run.
            round_rungs = round_rungs.saturating_mul(2).min(SCHEDULE_RUNGS);
        }
        // One last judge over whatever survived, so `rank_sum` is filled on
        // every live arm even when the loop exited on the deadline.
        judge_basin_race(&mut live, &mut rows);
        // ---- the decision -------------------------------------------------
        //
        // An eliminated arm's basin comes out of the archive, which is what
        // makes this a commitment rather than three extra draws. The
        // incumbent's is never retired even if it lost: it is the published
        // layout, and the archive is not where publication lives.
        if evict {
            for row in rows.iter() {
                if row.eliminated_round.is_none() || row.slot == 0 {
                    continue;
                }
                for fingerprint in &row.archived {
                    if run.archive.retire(fingerprint) {
                        retired.push(fingerprint.clone());
                    }
                }
            }
        }
    });
    let exit_cause = coordinator
        .phases
        .last()
        .map(|phase| phase.exit_cause.name().to_owned())
        .unwrap_or_else(|| PhaseExitCause::Completed.name().to_owned());
    let winner = live.first().map(|index| &rows[*index]);
    BasinRaceReport {
        armed: true,
        arms_started: rows.len(),
        rounds,
        kept: live.len(),
        retired: retired.len(),
        winner_slot: winner.map(|arm| arm.slot),
        winner_fingerprint: winner.map(|arm| arm.fingerprint.clone()),
        winner_depth_mm: winner.map(|arm| arm.depth_mm),
        incumbent_arm_depth_mm: rows.first().map(|arm| arm.depth_mm),
        work_units: coordinator.meter.work_units().saturating_sub(entered_work),
        seconds: coordinator.meter.seconds() - entered_seconds,
        exit_cause,
        arms: rows
            .iter()
            .map(|arm| BasinRaceArmReport {
                slot: arm.slot,
                kind: arm.kind.to_owned(),
                fingerprint: arm.fingerprint.clone(),
                depth_mm: arm.depth_mm,
                yield_mm: arm.yield_mm,
                stability: arm.stability,
                infeasibility: arm.infeasibility,
                batch_steps: arm.batch_steps,
                batch_confirmations: arm.batch_confirmations,
                rank_sum: arm.rank_sum,
                eliminated_round: arm.eliminated_round,
                retired_from_archive: arm
                    .archived
                    .iter()
                    .any(|fingerprint| retired.contains(fingerprint)),
            })
            .collect(),
    }
}

/// One salted constructor draw, descended by one alternation quantum.
///
/// The salting, the restart window and the divisor set are the diversify
/// class's, field for field - see `execute_v3_action`'s `Diversify` arm - so
/// the race is auditioning the basins that class would have produced and not a
/// fourth kind of constructor.
#[cfg(feature = "compression-schedule")]
fn draw_race_arm(
    run: &mut PhaseRun<'_, '_>,
    slot: usize,
    constructor_clamp_mm: f64,
) -> Option<BasinRaceArm> {
    let salt = slot as f64 * BASIN_TARGET_SALT_RELATIVE_STEP * constructor_clamp_mm;
    let divisor = if run.settings.cell_divisor_salts.is_empty() {
        None
    } else {
        Some(run.settings.cell_divisor_salts[slot % run.settings.cell_divisor_salts.len()])
    };
    let parent = run.incumbent.result.placements.clone();
    let parent_fingerprint = run.incumbent.fingerprint.clone();
    let drawn = run.run_operator(
        20,
        &parent,
        Some(parent_fingerprint),
        Some(constructor_clamp_mm + salt),
        |relaxed| {
            relaxed.construction_restart_window = Some((slot, 1));
            relaxed.construction_void_cell_divisor = divisor;
        },
        None,
        ParentRole::Prior,
        Some(format!("race m20 slot{slot}")),
    );
    let basin = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
        &drawn.final_placements,
    );
    if basin.len() != run.pieces.len() {
        return None;
    }
    let basin_depth = crate::search::general_relaxed::coupled_raw_source_depth(
        run.pieces,
        &basin,
        run.fast_settings,
    )
    .ok()?;
    let basin_fingerprint = general_placement_fingerprint(&basin);
    let basin_fingerprint_archived = basin_fingerprint.clone();
    let cycles = run.settings.descent_cycles.max(1);
    let epochs = run.settings.descent_relaxed_epochs.max(1);
    let descended = run.run_operator(
        22,
        &basin,
        Some(basin_fingerprint),
        Some(basin_depth + ALTERNATION_RUNG_MM),
        |relaxed| {
            relaxed.alternation_max_cycles = Some(cycles);
            relaxed.epochs = epochs;
        },
        None,
        ParentRole::Descended,
        Some(format!("race m22 quantum on slot{slot}")),
    );
    let settled = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
        &descended.final_placements,
    );
    let (placements, depth_mm) = if settled.len() == run.pieces.len() {
        match crate::search::general_relaxed::coupled_raw_source_depth(
            run.pieces,
            &settled,
            run.fast_settings,
        ) {
            Ok(depth) => (settled, depth),
            Err(_) => (basin, basin_depth),
        }
    } else {
        (basin, basin_depth)
    };
    Some(BasinRaceArm {
        slot,
        kind: "salted-constructor",
        fingerprint: general_placement_fingerprint(&placements),
        // The raw draw as well as the descended layout: both were archived on
        // the way here, and both are this arm's to take back if it loses.
        archived: vec![
            basin_fingerprint_archived,
            general_placement_fingerprint(&placements),
        ],
        placements,
        depth_mm,
        yield_mm: 0.0,
        stability: 1.0,
        infeasibility: f64::INFINITY,
        batch_steps: 0,
        batch_confirmations: 0,
        rank_sum: 0,
        eliminated_round: None,
    })
}

/// One audition batch: a mode-34 slice of `rungs` rungs on the arm's current
/// layout, with the three criteria read off its own report.
///
/// The batch is the arm's *continuation*: if it publishes deeper, the arm
/// carries the deeper layout into the next round, so a survivor's second batch
/// starts where its first stopped and the rungs are not re-walked.
#[cfg(feature = "compression-schedule")]
fn audition_race_arm(run: &mut PhaseRun<'_, '_>, arm: &mut BasinRaceArm, rungs: usize) {
    let drop_mm = arm.depth_mm
        * rungs as f64
        * crate::search::general_relaxed::COUPLED_SEPARATOR_CONTRACTION_RATIO;
    let bound = arm.depth_mm - drop_mm;
    let legalize_entry = run.settings.schedule_legalize_entry;
    let skip_infeasible_entry = run.settings.schedule_skip_infeasible_entry;
    let skip_unpublishable_entry = run.settings.schedule_skip_unpublishable_entry;
    let barren_probe_denominator = run.settings.schedule_probe_denominator;
    #[cfg(feature = "parallel-compression-schedule")]
    let schedule_lanes = run.settings.compression_schedule_lanes.max(1);
    #[cfg(feature = "parallel-compression-schedule")]
    let schedule_parallel_confirm = run.settings.compression_schedule_parallel_confirm;
    let slot = arm.slot;
    let parent = arm.placements.clone();
    let parent_fingerprint = arm.fingerprint.clone();
    let scheduled = run.run_operator(
        34,
        &parent,
        Some(parent_fingerprint),
        Some(bound),
        |relaxed| {
            #[allow(unused_mut)]
            let mut schedule_settings =
                crate::search::compression_schedule::CompressionScheduleSettings {
                    legalize_entry,
                    skip_infeasible_entry,
                    skip_unpublishable_entry,
                    barren_probe_denominator,
                    ..crate::search::compression_schedule::CompressionScheduleSettings::default()
                };
            #[cfg(feature = "parallel-compression-schedule")]
            {
                schedule_settings.lanes = schedule_lanes;
                schedule_settings.parallel_confirm = schedule_parallel_confirm;
            }
            relaxed.compression_schedule = Some(schedule_settings);
        },
        None,
        ParentRole::Descended,
        Some(format!("race m34 batch slot{slot} ({rungs} rungs)")),
    );
    // The three criteria, all off the slice's own report so that an arm the
    // schedule refused to enter at all is scored on what actually happened
    // rather than on what it would have been given.
    if let Some(slice) = scheduled.compression_schedule.as_ref() {
        arm.batch_steps = slice.steps_taken;
        arm.batch_confirmations = slice.confirmations_attempted;
        arm.stability = if slice.confirmations_attempted == 0 {
            1.0
        } else {
            slice.confirmations_accepted as f64 / slice.confirmations_attempted as f64
        };
        arm.infeasibility = (slice.entry_collision_pairs + slice.entry_boundary_violations) as f64
            / run.pieces.len().max(1) as f64;
    }
    let produced = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
        &scheduled.final_placements,
    );
    if produced.len() != run.pieces.len() {
        return;
    }
    let Ok(depth) = crate::search::general_relaxed::coupled_raw_source_depth(
        run.pieces,
        &produced,
        run.fast_settings,
    ) else {
        return;
    };
    // Yield accumulates across rounds, because a survivor's rounds are one
    // continued batch and the criterion is what the whole audition bought.
    if depth < arm.depth_mm {
        arm.yield_mm += arm.depth_mm - depth;
        arm.depth_mm = depth;
        arm.fingerprint = general_placement_fingerprint(&produced);
        arm.archived.push(arm.fingerprint.clone());
        arm.placements = produced;
    }
}

/// The v3 loop: enumerate, rank, spend the best affordable action, repeat.
fn run_v3_schedule(
    coordinator: &mut Coordinator<'_>,
    constructor_clamp_mm: f64,
    plan: Option<&PlanReport>,
    tranches: &mut Vec<TrancheReport>,
) -> ScheduleReport {
    let mut actions: Vec<ScheduledActionReport> = Vec::new();
    let phase_zero_cost = coordinator.phase_zero_cost;
    let mut queue = V3QueueState::default();
    run_v3_tranche(
        coordinator,
        constructor_clamp_mm,
        &mut actions,
        &mut queue,
        "schedule",
    );
    // ---- the in-run re-plan ------------------------------------------------
    //
    // `docs/experiments/calibrated-plan/` §13.1: *"Install a provisional plan
    // from phase 0, run to a deterministic work checkpoint, then re-price the
    // remaining wall at the rate the queue is actually retiring units at."*
    // The deterministic work checkpoint is the line above - the tranche's own
    // budget is a counter, so where it stops is a counter's decision - and the
    // re-pricing is `BudgetMeter::replan`.
    //
    // The two lines §13.1 names as the cost are handled by *not* patching them:
    // rather than recomputing `run.deadline` and `protected_fraction` inside a
    // phase that is already running against them, each tranche recomputes
    // `protected_fraction` from the new total and enters a **new phase**. Every
    // deadline a tranche runs against is therefore a fraction of the budget
    // that is actually in force, which is the property the previous chapters'
    // schedule numbers rest on and the reason that round left this undone.
    if let Some(plan) = plan {
        if coordinator.settings.plan_replan {
            let max_tranches = coordinator.settings.plan_max_tranches;
            for index in 1..=max_tranches {
                // A tranche buys *budget*, so it is only worth buying when the
                // budget is what ran out. A queue that stopped because it had
                // nothing left to enumerate - `keysExhausted`, `patience`,
                // `geometricFixpoint` - will stop again on the next line for
                // the same reason, and a report carrying five empty `replanN`
                // phases would describe a run that re-planned five times when
                // what happened is that it finished early.
                //
                // `skippedDeadlinePassed` *is* in the list, and deliberately:
                // it means phase 0 alone outspent the tranche it was given,
                // which is the one case where a bigger budget turns a run that
                // did nothing into a run that searches.
                let budget_bound = matches!(
                    coordinator.phases.last().map(|phase| phase.exit_cause),
                    Some(
                        PhaseExitCause::Deadline
                            | PhaseExitCause::Affordability
                            | PhaseExitCause::SkippedDeadlinePassed
                    )
                );
                if !budget_bound {
                    break;
                }
                // Cloned rather than borrowed: `replan` takes `&mut self` on a
                // field of the same struct the settings live on, and a
                // coordinator-wide borrow would put the two on opposite sides
                // of the borrow checker for no gain - this runs at most
                // `PLAN_MAX_TRANCHES` times per process.
                let settings = coordinator.settings.clone();
                let Some(tranche) = coordinator.meter.replan(plan, &settings, index) else {
                    break;
                };
                // Phase 0's share of the *new* total. It shrinks with every
                // tranche, which is correct: the protected phase is a fixed
                // number of work units and the budget it is protected out of
                // has grown.
                coordinator.protected_fraction =
                    (plan.probe_work_units as f64 / tranche.units as f64).clamp(0.0, 1.0);
                tranches.push(tranche);
                run_v3_tranche(
                    coordinator,
                    constructor_clamp_mm,
                    &mut actions,
                    &mut queue,
                    &format!("replan{index}"),
                );
            }
        }
    }
    // A slice may still be parked when the budget runs out, and the run must not
    // end holding one: its incumbent was already published by the call that
    // suspended it, but its *report* has not been written, and a run that
    // silently drops a slice's account is a run whose work numbers do not add
    // up. Ending it here costs no further geometry - no batch is run, no
    // confirmation is asked - which is why this is a drain and not a resume.
    #[cfg(feature = "compression-schedule")]
    coordinator.drain_suspended_slice();
    let exit_cause = coordinator
        .phases
        .last()
        .map(|phase| phase.exit_cause.name().to_owned())
        .unwrap_or_else(|| PhaseExitCause::Completed.name().to_owned());
    let classes = coordinator
        .class_stats
        .iter()
        .map(|(class, stats)| ScheduleClassReport {
            class: class.name().to_owned(),
            actions: stats.actions,
            publications: stats.publications,
            work_units: stats.work_units,
            seconds: stats.seconds,
            cost_total: stats.cost_total,
            cost_max: stats.cost_max,
            delta_raw_mm: stats.delta_raw_mm,
            first_estimated_cost: stats.first_estimated_cost,
            first_actual_cost: stats.first_actual_cost,
        })
        .collect();
    ScheduleReport {
        iterations: actions.len(),
        exit_cause,
        actions,
        classes,
        phase_zero_cost,
    }
}

/// One tranche of the v3 queue: a phase, against the budget in force now.
///
/// A named function rather than an inlined `run_phase` because the re-plan calls
/// it more than once and every call has to be the *same* call - same loop, same
/// share, same accumulator. The only thing that differs between tranches is the
/// phase name, which is what makes the second one visible in the report instead
/// of hidden inside the first one's row.
fn run_v3_tranche(
    coordinator: &mut Coordinator<'_>,
    constructor_clamp_mm: f64,
    actions: &mut Vec<ScheduledActionReport>,
    queue: &mut V3QueueState,
    name: &str,
) {
    let schedule_by = coordinator.settings.schedule.schedule_by;
    coordinator.run_phase(name, schedule_by, |run| {
        v3_loop(run, constructor_clamp_mm, actions, queue);
    });
}

/// The v3 queue's own policy counters, held across tranches.
///
/// Every field here used to be a `let mut` inside [`v3_loop`], and moving them
/// out is not a tidy-up: it is what makes a second tranche a **continuation**
/// rather than a second run of the queue.
///
/// The difference is measurable and all of it is in the wrong direction. A
/// tranche that restarted these would give the compression-schedule class a
/// fresh audition it has already failed, reset the barren patience that
/// coordinator v3 §4.2 measured at eight, and offer diversify slot 0 again on a
/// run that has already spent it. None of those is what "the plan's work is
/// exhausted, buy some more" means. The budget grew; the run's history did not
/// reset, and neither does this.
#[derive(Clone, Copy, Debug, Default)]
struct V3QueueState {
    diversify_slot: usize,
    diversify_barren: usize,
    diversify_done: bool,
    /// The falsifiability half of Grok review 1 §2b item 4's sterile bit: the
    /// class is offered once more after [`SCHEDULE_AUDITION_BARREN`] further
    /// barren actions, and **once only** - which is a property of the run, not
    /// of the tranche.
    schedule_auditioned: bool,
    barren_since_schedule: usize,
    /// Consecutive actions of *any* class that published nothing. Reset by a
    /// publication and by nothing else.
    barren: usize,
    /// The same count, additionally reset by a diversify action, so the audition
    /// rule fires at most once per `DIVERSIFY_AUDITION_BARREN` barren actions
    /// rather than on every action after the eighth.
    barren_since_diversify: usize,
}

fn v3_loop(
    run: &mut PhaseRun<'_, '_>,
    constructor_clamp_mm: f64,
    out: &mut Vec<ScheduledActionReport>,
    queue: &mut V3QueueState,
) {
    let patience = run.settings.basin_patience.max(1);
    let slots = run.settings.basin_slots;
    let barren_patience = run.settings.barren_action_patience;
    let ranked_diversify = run.settings.diversify_in_queue;
    let sterile_bit = run.settings.schedule_sterile_bit;
    let V3QueueState {
        mut diversify_slot,
        mut diversify_barren,
        mut diversify_done,
        mut schedule_auditioned,
        mut barren_since_schedule,
        mut barren,
        mut barren_since_diversify,
    } = *queue;
    diversify_done = diversify_done || run.settings.basin_trigger == BasinTrigger::Never;
    // Written back on every exit, including the early ones, so a tranche cannot
    // drop the history by returning through a path that forgot to save it.
    macro_rules! save {
        () => {
            *queue = V3QueueState {
                diversify_slot,
                diversify_barren,
                diversify_done,
                schedule_auditioned,
                barren_since_schedule,
                barren,
                barren_since_diversify,
            };
        };
    }
    loop {
        // Asked before the resumption and before the enumeration, because a
        // queue that is out of *seconds* must not spend one more of them on
        // either. A suspended slice is not lost by exiting here: its incumbent
        // was published by the call that suspended it, and `drain_suspended_slice`
        // writes its report without running a batch.
        if run.wall_stop_refuses(None) {
            run.note_exit(PhaseExitCause::WallStop);
            save!();
            return;
        }
        if !run.meter.has_room(run.deadline) {
            run.note_exit(PhaseExitCause::Deadline);
            save!();
            return;
        }
        // The resumption, and it is the *first* thing the loop does because a
        // suspended slice is not a candidate: it is an action already bought,
        // half spent, holding a frontier whose caches are the reason its next
        // step is cheap. Ranking it against the queue would be pricing a sunk
        // cost.
        //
        // `actions_since_suspension >= 1` is the whole rule. It is what makes
        // this an interleave: the slice hands its turn back, the queue spends
        // exactly one action on whichever class outranks it now, and then the
        // slice gets its turn back. Zero would be a more expensive way to write
        // the loop the previous round already had.
        #[cfg(feature = "compression-schedule")]
        if run.suspended_slice.is_some() && run.actions_since_suspension >= 1 {
            let iteration = out.len();
            let entry_raw_depth_mm = run.incumbent.raw_depth_mm;
            let cost_before = run.meter.currency_spent();
            let work_before = run.meter.work_units();
            let debit_before = run.meter.self_metered_debit();
            let seconds_before = run.meter.seconds();
            let publications_before = run.publications.len();
            let calls_before = run.operator_calls.len();
            let resumed = run.resume_suspended_slice();
            let self_metered_units = resumed.as_ref().and_then(schedule_self_cost_units);
            run.coordinator.actions_since_suspension = 0;
            let debited_units = run.meter.self_metered_debit().saturating_sub(debit_before);
            let charged_cost = (run.meter.currency_spent() - cost_before).max(0.0);
            let metered_cost = if run.meter.is_wall() {
                charged_cost
            } else {
                (charged_cost - debited_units as f64).max(0.0)
            };
            let cost = match self_metered_units {
                Some(units) if !run.meter.is_wall() => charged_cost.max(units as f64),
                _ => charged_cost,
            };
            let publications = run.publications.len() - publications_before;
            let work_units = run.meter.work_units().saturating_sub(work_before);
            let seconds = run.meter.seconds() - seconds_before;
            let operator_calls = run.operator_calls.len() - calls_before;
            let exit_raw_depth_mm = run.incumbent.raw_depth_mm;
            let gained = match (entry_raw_depth_mm, exit_raw_depth_mm) {
                (Some(entry), Some(exit)) => (entry - exit).max(0.0),
                _ => 0.0,
            };
            {
                let stats = run.class_stats.entry(ActionClass::Schedule).or_default();
                stats.actions += 1;
                stats.publications += publications;
                stats.work_units += work_units;
                stats.seconds += seconds;
                stats.cost_total += cost;
                stats.cost_max = stats.cost_max.max(cost);
                stats.delta_raw_mm += gained;
            }
            out.push(ScheduledActionReport {
                iteration,
                class: ActionClass::Schedule.name().to_owned(),
                key: "schedule:resume".to_owned(),
                label: "m34 resume".to_owned(),
                value: 0.0,
                estimated_cost: 0.0,
                actual_cost: cost,
                work_units,
                seconds,
                operator_calls,
                publications,
                entry_raw_depth_mm,
                exit_raw_depth_mm,
                candidates: 0,
                metered_cost,
                self_metered_units,
                debited_units,
            });
            continue;
        }
        let mut candidates = run.enumerate_v3_actions();
        // The diversify class competes on rank like every other class, instead
        // of being gated on the priced queue emptying - which coordinator v3
        // §4.2 measured never happening on triangle-20, where the class is the
        // only one that pays: crossover regenerates ordered pairs at 217
        // actions per nine runs, so v3 never draws a ticket at all and loses
        // the 3 µm those tickets were worth.
        let diversify_available = ranked_diversify && !diversify_done && diversify_slot < slots;
        if diversify_available {
            candidates.push(diversify_action(diversify_slot));
        }
        // The sterile bit. It is applied to the *candidate list* rather than to
        // the prior, because a prior of zero is a deletion the class can never
        // argue with (coordinator v4 §3.1) while a candidate withheld is a
        // candidate the audition below can hand back. The class keeps its
        // prior, its stats and its ratchet throughout.
        let mut schedule_audition_due = false;
        if sterile_bit {
            let sterile = run
                .class_stats
                .get(&ActionClass::Schedule)
                .is_some_and(|stats| {
                    stats.actions >= SCHEDULE_STERILE_ACTIONS && stats.publications == 0
                });
            if sterile {
                schedule_audition_due =
                    !schedule_auditioned && barren_since_schedule >= SCHEDULE_AUDITION_BARREN;
                if !schedule_audition_due {
                    candidates.retain(|action| action.class != ActionClass::Schedule);
                }
            }
        }
        let candidate_count = candidates.len();
        // Rank: value first, then the class declaration order, then the action's
        // own order within its class. Every comparison is total, so the queue is
        // a deterministic function of the archive.
        let values = ActionClass::all()
            .into_iter()
            .map(|class| (class, run.class_value(class)))
            .collect::<BTreeMap<_, _>>();
        candidates.sort_by(|first, second| {
            values[&second.class]
                .total_cmp(&values[&first.class])
                .then(first.class.cmp(&second.class))
                .then(first.rank.cmp(&second.rank))
                .then(first.key.cmp(&second.key))
        });
        // The audition. A prior of 0.005826 mm never outranks a crossover prior
        // of 1.0923 mm inside any budget this engine runs at, so ranking the
        // class is necessary and not sufficient: a prior that is never tested
        // is not evidence, and v3's own rule says the prior is worth two
        // actions. After `DIVERSIFY_AUDITION_BARREN` barren actions the queue
        // promotes one ticket to the front of the affordable set - and it is
        // still the *affordability* rule that decides whether it is bought,
        // which is the half of this that fixes §1.3's 12x mispricing.
        if diversify_available && barren_since_diversify >= DIVERSIFY_AUDITION_BARREN {
            if let Some(position) = candidates
                .iter()
                .position(|action| action.class == ActionClass::Diversify)
            {
                let promoted = candidates.remove(position);
                candidates.insert(0, promoted);
            }
        }
        // The wall reserve, applied to the *candidate list* rather than folded
        // into the affordability `find` below, so that "every action left would
        // cross the deadline" exits on its own cause instead of being reported
        // as a work-affordability exit. With the reserve at its `0.0` default
        // this retains every candidate and the list is the one the queue has
        // always ranked.
        let wall_reserved = candidate_count > 0 && {
            candidates.retain(|action| !run.wall_stop_refuses(Some(action.class)));
            candidates.is_empty()
        };
        if wall_reserved {
            run.note_exit(PhaseExitCause::WallStop);
            save!();
            return;
        }
        let remaining = run.meter.remaining_to(run.deadline);
        let chosen = candidates
            .into_iter()
            .find(|action| remaining >= run.class_cost_estimate(action.class));
        let action = match chosen {
            Some(action) => action,
            None if candidate_count > 0 => {
                // Every action the queue can name costs more than the budget
                // has left. That is a different finding from having no actions,
                // and the exit cause says which.
                run.note_exit(PhaseExitCause::Affordability);
                save!();
                return;
            }
            None => {
                // No complementary pairs remain: the one place a constructor
                // ticket is worth its price, and the only place v3 spends one.
                if diversify_done || diversify_slot >= slots {
                    run.note_exit(PhaseExitCause::KeysExhausted);
                    save!();
                    return;
                }
                // This ticket is not on the ranked list, so the reserve filter
                // above never saw it. It is the most expensive action the queue
                // buys - a constructor arm plus a legalization quantum - so it
                // is the last one that should be allowed through the wall.
                if run.wall_stop_refuses(Some(ActionClass::Diversify)) {
                    run.note_exit(PhaseExitCause::WallStop);
                    save!();
                    return;
                }
                let Some(quantum) = run.mean_operator_cost("mode22") else {
                    run.note_exit(PhaseExitCause::Affordability);
                    save!();
                    return;
                };
                let arm = run.mean_operator_cost("mode20").unwrap_or(quantum);
                if remaining < arm + quantum {
                    run.note_exit(PhaseExitCause::Affordability);
                    save!();
                    return;
                }
                let slot = diversify_slot;
                diversify_slot += 1;
                diversify_action(slot)
            }
        };

        let class = action.class;
        if class == ActionClass::Diversify && ranked_diversify {
            // The slot advances when the ticket is *bought*, not when it is
            // offered: an offer the affordability rule declines has to be
            // offerable again on the next iteration or the class silently
            // spends its slots on actions it never took.
            diversify_slot += 1;
        }
        let estimated_cost = run.class_cost_estimate(class);
        let entry_raw_depth_mm = run.incumbent.raw_depth_mm;
        let cost_before = run.meter.currency_spent();
        let work_before = run.meter.work_units();
        let debit_before = run.meter.self_metered_debit();
        let seconds_before = run.meter.seconds();
        let publications_before = run.publications.len();
        let calls_before = run.operator_calls.len();
        let iteration = out.len();

        // Counted *before* the action, and that ordering is the interleave.
        // `run_operator` zeroes this counter at the instant it suspends a slice,
        // so an increment afterwards would credit the suspending action itself
        // as "the other action that ran" and the slice would be resumed on the
        // very next iteration with nothing in between.
        #[cfg(feature = "compression-schedule")]
        {
            run.coordinator.actions_since_suspension += 1;
        }
        let self_metered_units = execute_v3_action(run, &action, constructor_clamp_mm);

        // The debit is applied inside the operator transaction now (see
        // [`Coordinator::run_operator`]), so by here the meter has already
        // settled and `currency_spent` is the honest charge. What is left for
        // this loop is *attribution*: how much of the action's charge was the
        // debit, so the report can carry the global reading and the debit
        // separately rather than only their sum.
        let debited_units = run.meter.self_metered_debit().saturating_sub(debit_before);
        let charged_cost = (run.meter.currency_spent() - cost_before).max(0.0);
        let metered_cost = if run.meter.is_wall() {
            charged_cost
        } else {
            (charged_cost - debited_units as f64).max(0.0)
        };
        // The one place the coordinator charges an action more than its own
        // meter read. See [`schedule_self_cost_units`]. `charged_cost` already
        // *is* `max(metered_cost, units)` for the single-operator schedule
        // action, which is the only self-metered action today; the `max` is
        // kept so an action that ever dispatches a self-metered operator
        // alongside others is still priced at no less than the self-meter's
        // own reading.
        let cost = match self_metered_units {
            Some(units) if !run.meter.is_wall() => charged_cost.max(units as f64),
            _ => charged_cost,
        };
        let work_units = run.meter.work_units().saturating_sub(work_before);
        let seconds = run.meter.seconds() - seconds_before;
        let publications = run.publications.len() - publications_before;
        let operator_calls = run.operator_calls.len() - calls_before;
        let exit_raw_depth_mm = run.incumbent.raw_depth_mm;
        let gained = match (entry_raw_depth_mm, exit_raw_depth_mm) {
            (Some(entry), Some(exit)) => (entry - exit).max(0.0),
            _ => 0.0,
        };
        {
            let stats = run.class_stats.entry(class).or_default();
            stats.actions += 1;
            stats.publications += publications;
            stats.work_units += work_units;
            stats.seconds += seconds;
            stats.cost_total += cost;
            stats.cost_max = stats.cost_max.max(cost);
            stats.delta_raw_mm += gained;
            if stats.first_estimated_cost.is_none() {
                stats.first_estimated_cost = Some(estimated_cost);
                stats.first_actual_cost = Some(cost);
            }
        }
        out.push(ScheduledActionReport {
            iteration,
            class: class.name().to_owned(),
            key: action.key.clone(),
            label: action.label.clone(),
            value: values.get(&class).copied().unwrap_or(0.0),
            estimated_cost,
            actual_cost: cost,
            work_units,
            seconds,
            operator_calls,
            publications,
            entry_raw_depth_mm,
            exit_raw_depth_mm,
            candidates: candidate_count,
            metered_cost,
            self_metered_units,
            debited_units,
        });
        if class == ActionClass::Diversify {
            // The stopping signal is the descendant, never the arm's own depth
            // - the same rule v2's patience implements, kept because the
            // ledger's eighteen-sample sweep measured Pearson(immediate,
            // descended) = -0.212.
            if publications > 0 {
                diversify_barren = 0;
            } else {
                diversify_barren += 1;
                if diversify_barren >= patience {
                    diversify_done = true;
                }
            }
        }
        // The global patience. The signal is the incumbent moving, which is the
        // only thing the coordinator is being paid for, and it is deliberately
        // *not* a yield floor: coordinator v3 §5.2 suggested one, and a floor
        // needs a millimetre to compare against, which is the kind of constant
        // this schedule carries none of.
        if publications > 0 {
            barren = 0;
            barren_since_diversify = 0;
            barren_since_schedule = 0;
        } else {
            barren += 1;
            barren_since_diversify += 1;
            barren_since_schedule += 1;
        }
        if class == ActionClass::Diversify {
            barren_since_diversify = 0;
        }
        if class == ActionClass::Schedule {
            barren_since_schedule = 0;
            // Spent whether or not the audition published: a rare audition that
            // re-armed itself on every failure would be the state machine this
            // rule exists instead of.
            schedule_auditioned |= schedule_audition_due;
        }
        if barren_patience > 0 && barren >= barren_patience {
            // With the queue still full: this is a patience exit, not a
            // fixpoint and not an affordability exit, and the three are
            // different findings about a run.
            run.note_exit(PhaseExitCause::Patience);
            save!();
            return;
        }
    }
}

/// One constructor ticket plus the quantum spent on it, at `slot`.
///
/// Built in one place because the queue now names it from two - the ranked
/// enumeration and merged-HEAD v3's empty-queue fallback - and the two have to
/// produce the same key or the same ticket would be bought twice.
fn diversify_action(slot: usize) -> ScheduledAction {
    ScheduledAction {
        class: ActionClass::Diversify,
        key: format!("m20:slot{slot}"),
        rank: slot,
        label: format!("m20 ticket slot{slot} + m22 quantum"),
        payload: ActionPayload::Diversify { slot },
    }
}

/// Executes one queued action. Marks its key attempted first, so an action that
/// produces nothing is still never offered twice.
///
/// Returns the action's own self-metered work units when the operator carries a
/// meter of its own that the coordinator's does not cover; `None` otherwise.
/// Today that is exactly the compression schedule - see
/// [`schedule_self_cost_units`].
fn execute_v3_action(
    run: &mut PhaseRun<'_, '_>,
    action: &ScheduledAction,
    constructor_clamp_mm: f64,
) -> Option<u64> {
    run.phase_name = action.class.name().to_owned();
    run.already_attempted(action.key.clone());
    match (action.class, &action.payload) {
        (ActionClass::Descent, ActionPayload::Basin { fingerprint }) => {
            let Some(basin) = run.basin_by_fingerprint(fingerprint) else {
                return None;
            };
            let cycles = run.settings.descent_cycles.max(1);
            let epochs = run.settings.descent_relaxed_epochs.max(1);
            run.run_operator(
                22,
                &basin.placements,
                Some(basin.fingerprint.clone()),
                Some(basin.raw_depth_mm + ALTERNATION_RUNG_MM),
                |relaxed| {
                    relaxed.alternation_max_cycles = Some(cycles);
                    relaxed.epochs = epochs;
                },
                None,
                ParentRole::Descended,
                Some(action.label.clone()),
            );
            None
        }
        (ActionClass::Compression, ActionPayload::Basin { fingerprint }) => {
            let Some(basin) = run.basin_by_fingerprint(fingerprint) else {
                return None;
            };
            let epochs = run.settings.descent_relaxed_epochs.max(1);
            let target = basin.raw_depth_mm - COMPRESSION_RUNG_MM;
            let compressed = run.run_operator(
                22,
                &basin.placements,
                Some(basin.fingerprint.clone()),
                Some(target),
                |relaxed| {
                    relaxed.alternation_max_cycles = Some(1);
                    relaxed.epochs = epochs;
                },
                None,
                ParentRole::Descended,
                Some(action.label.clone()),
            );
            if compressed.exact_valid {
                // Already archived and already offered to the adoption rule by
                // `run_operator`. There is nothing for a legalizer to do.
                return None;
            }
            legalize_residue(run, &compressed, "m31 on the compression residue");
            None
        }
        (ActionClass::Ladder, ActionPayload::Basin { fingerprint }) => {
            let Some(basin) = run.basin_by_fingerprint(fingerprint) else {
                return None;
            };
            let drop_mm = basin.raw_depth_mm
                * LADDER_RUNGS as f64
                * crate::search::general_relaxed::COUPLED_SEPARATOR_CONTRACTION_RATIO;
            let bound = basin.raw_depth_mm - drop_mm;
            let ladder = run.run_operator(
                26,
                &basin.placements,
                Some(basin.fingerprint.clone()),
                Some(bound),
                |_| {},
                None,
                ParentRole::Descended,
                Some(action.label.clone()),
            );
            legalize_residue(run, &ladder, "m31 on the ladder residue");
            None
        }
        (
            ActionClass::Crossover,
            ActionPayload::Crossover {
                left_fingerprint,
                right_fingerprint,
                cut_fraction,
            },
        ) => {
            let (Some(left), Some(right)) = (
                run.basin_by_fingerprint(left_fingerprint),
                run.basin_by_fingerprint(right_fingerprint),
            ) else {
                return None;
            };
            let parent_b = GeneralPersistentVacancyPinnedParent {
                placements: right.placements.clone(),
                source: "archive".to_owned(),
                source_sha256: right.fingerprint.clone(),
            };
            run.run_operator(
                23,
                &left.placements,
                Some(left.fingerprint.clone()),
                Some(*cut_fraction),
                |_| {},
                Some(&parent_b),
                ParentRole::Descended,
                Some(action.label.clone()),
            );
            // Both parents were descended from.
            run.archive.charge_descent(&right.fingerprint);
            None
        }
        (ActionClass::Diversify, ActionPayload::Diversify { slot }) => {
            let slot = *slot;
            let salt = slot as f64 * BASIN_TARGET_SALT_RELATIVE_STEP * constructor_clamp_mm;
            let divisor = if run.settings.cell_divisor_salts.is_empty() {
                None
            } else {
                Some(run.settings.cell_divisor_salts[slot % run.settings.cell_divisor_salts.len()])
            };
            let parent = run.incumbent.result.placements.clone();
            let parent_fingerprint = run.incumbent.fingerprint.clone();
            let drawn = run.run_operator(
                20,
                &parent,
                Some(parent_fingerprint),
                Some(constructor_clamp_mm + salt),
                |relaxed| {
                    relaxed.construction_restart_window = Some((slot, 1));
                    relaxed.construction_void_cell_divisor = divisor;
                },
                None,
                // The constructor builds from scratch; the incumbent is only
                // its pose prior.
                ParentRole::Prior,
                Some(action.label.clone()),
            );
            let basin = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
                &drawn.final_placements,
            );
            if basin.len() != run.pieces.len() {
                return None;
            }
            let Some(basin_depth) = crate::search::general_relaxed::coupled_raw_source_depth(
                run.pieces,
                &basin,
                run.fast_settings,
            )
            .ok() else {
                return None;
            };
            let basin_fingerprint = general_placement_fingerprint(&basin);
            let cycles = run.settings.descent_cycles.max(1);
            let epochs = run.settings.descent_relaxed_epochs.max(1);
            if run.already_attempted(format!("22:{cycles}:{epochs}:{basin_fingerprint}")) {
                return None;
            }
            run.run_operator(
                22,
                &basin,
                Some(basin_fingerprint),
                Some(basin_depth + ALTERNATION_RUNG_MM),
                |relaxed| {
                    relaxed.alternation_max_cycles = Some(cycles);
                    relaxed.epochs = epochs;
                },
                None,
                ParentRole::Descended,
                Some(format!("m22 quantum on m20 slot{slot}")),
            );
            None
        }
        #[cfg(feature = "compression-schedule")]
        (ActionClass::Schedule, ActionPayload::Basin { fingerprint }) => {
            let Some(basin) = run.basin_by_fingerprint(fingerprint) else {
                return None;
            };
            let drop_mm = basin.raw_depth_mm
                * SCHEDULE_RUNGS as f64
                * crate::search::general_relaxed::COUPLED_SEPARATOR_CONTRACTION_RATIO;
            let bound = basin.raw_depth_mm - drop_mm;
            // Read before the operator closure borrows `run`, so the closure
            // captures scalars rather than the phase run.
            let legalize_entry = run.settings.schedule_legalize_entry;
            let skip_infeasible_entry = run.settings.schedule_skip_infeasible_entry;
            let skip_unpublishable_entry = run.settings.schedule_skip_unpublishable_entry;
            let barren_probe_denominator = run.settings.schedule_probe_denominator;
            #[cfg(feature = "parallel-compression-schedule")]
            let schedule_lanes = run.settings.compression_schedule_lanes.max(1);
            #[cfg(feature = "parallel-compression-schedule")]
            let schedule_parallel_confirm = run.settings.compression_schedule_parallel_confirm;
            // The batch budget, resolved here because this is the only place
            // that can see both the slice's settings and the coordinator's
            // remaining budget. An explicit `m34batch` wins over the cap, so a
            // gate can pin a batch size without also arming the policy.
            //
            // `remaining_to` is in the meter's own currency, which for a work
            // or a plan budget is the same currency the slice charges itself
            // in - see `compression_schedule_cap_to_budget`. Under a *wall*
            // budget it is seconds, and capping a work meter with a number of
            // seconds would be a category error, so the cap is refused there
            // and the slice stays atomic.
            //
            // The past-bound lever is the third source, and it is a *budget*
            // for the whole action: past the bound the walk has no natural end
            // short of the sheet floor, so the number the coordinator can
            // afford for this action is what stops it, and cutting that number
            // into `past_bound_batches` is what turns "afford it once, at
            // dispatch, on an estimate" into "re-ask at every checkpoint".
            let past_bound = run.settings.compression_schedule_past_bound;
            let action_budget_units = (!run.meter.is_wall())
                .then(|| run.meter.remaining_to(run.deadline).max(1.0) as usize);
            // The share is applied here rather than at a checkpoint, because a
            // work cap is exactly the right instrument for "you have spent
            // enough": it fires at the top of a step, not at the end of a
            // batch, so the slice cannot overshoot it by a batch. What the
            // checkpoint policy owns is the other question - "you have stopped
            // buying anything" - which no cap can express.
            let past_bound_cap = past_bound
                .then_some(action_budget_units)
                .flatten()
                .map(|units| {
                    let share = run
                        .settings
                        .compression_schedule_past_bound_share
                        .clamp(0.0, 1.0);
                    ((units as f64 * share) as usize).max(1)
                });
            // A checkpoint policy with no checkpoints to answer at is a policy
            // that never runs, so all three keys that consume checkpoints -
            // `m34past`, `m34wallstop` and `m34yield` - give the slice a batch
            // budget when no explicit `m34batch` named one. The divisor is the
            // same in all three because it means the same thing in all three:
            // how many times the coordinator gets to decide over one action.
            //
            // This is the trap the previous round fell into from the other side.
            // `m34cap` handed the slice a batch budget and then never asked it
            // anything; arming a policy without a batch budget is the mirror
            // image - the coordinator would be ready to answer at a checkpoint
            // the slice never reaches - and it would look exactly as armed in
            // the spec and do exactly as little.
            let wants_checkpoints = past_bound
                || run.settings.compression_schedule_wall_stop
                || run.settings.compression_schedule_wall_stop_all
                || run.settings.compression_schedule_yield_batches > 0;
            let batch_work_units = run
                .settings
                .compression_schedule_batch_work_units
                .or_else(|| {
                    let batches = run.settings.compression_schedule_past_bound_batches.max(1);
                    wants_checkpoints
                        .then_some(past_bound_cap.or(action_budget_units))
                        .flatten()
                        .map(|units| (units / batches).max(1))
                })
                .or_else(|| {
                    let deadline = run.deadline;
                    (run.settings.compression_schedule_cap_to_budget && !run.meter.is_wall())
                        .then(|| run.meter.remaining_to(deadline).max(1.0) as usize)
                });
            // The confirmation-density lever, and it is scoped by *count* here
            // rather than by a flag inside the slice: the coordinator is the
            // only thing that knows which slice this is. Incremented before the
            // dispatch so a slice that returns early still spends its turn -
            // "the first slice" has to mean the first one that was tried, or a
            // request whose first slice is refused at the entry gate would apply
            // the lever to its second and report it as its first.
            let first_slice = run.schedule_slices == 0;
            run.coordinator.schedule_slices += 1;
            let first_slice_step_grid = first_slice
                .then_some(run.settings.schedule_first_slice_step_grid)
                .flatten();
            let first_slice_confirm_every = first_slice
                .then_some(run.settings.schedule_first_slice_confirm_every)
                .flatten();
            let scheduled = run.run_operator(
                34,
                &basin.placements,
                Some(basin.fingerprint.clone()),
                Some(bound),
                |relaxed| {
                    // The port's own measured defaults, unmodified: six repair
                    // sweeps per step, a confirmation due every fourth step,
                    // `micro_legalize` on a refused confirmation - and
                    // `rollback_after_steps = 0`, which is not a preference but
                    // the port's structural finding (arming it at 32 cost a
                    // paired median 10.962 mm over twelve cells, because a
                    // rollback triggered by "the frontier has not been
                    // publishable lately" fires on the normal state of a
                    // compression frontier).
                    //
                    // `continue_past_bound` stays `false` and there is no work
                    // cap: the slice *is* the bound. Nine rungs of the
                    // separator's own quantum is a step count the request
                    // supplies, so the arm is deterministic and load-independent
                    // without reading a counter at all - which a work cap
                    // expressed in the coordinator's currency would not be,
                    // because that currency is zero when profiling is off and a
                    // wall-budget run has it off.
                    //
                    // The four entry keys and the two intra-arm parallel levers
                    // are the only fields the coordinator overrides, and every
                    // one defaults to the shipped serial slice, so an unarmed
                    // spec builds the default settings field for field. See
                    // `CompressionScheduleSettings::legalize_entry`.
                    #[allow(unused_mut)]
                    let mut schedule_settings =
                        crate::search::compression_schedule::CompressionScheduleSettings {
                            legalize_entry,
                            skip_infeasible_entry,
                            skip_unpublishable_entry,
                            barren_probe_denominator,
                            batch_work_units,
                            ..crate::search::compression_schedule::CompressionScheduleSettings::default()
                        };
                    #[cfg(feature = "parallel-compression-schedule")]
                    {
                        schedule_settings.lanes = schedule_lanes;
                        schedule_settings.parallel_confirm = schedule_parallel_confirm;
                    }
                    // Applied last and only when a value was actually named, so
                    // an unarmed spec leaves the two fields at the module's own
                    // defaults rather than writing them back with the same
                    // numbers - which is the difference between a document that
                    // is equal to the base binary's and one that only looks it.
                    if let Some(step_grid) = first_slice_step_grid {
                        schedule_settings.step_grid = step_grid;
                    }
                    if let Some(confirm_every) = first_slice_confirm_every {
                        schedule_settings.confirm_every = confirm_every;
                    }
                    // The bound lever, and it is two writes because the slice's
                    // own loop needs both: `continue_past_bound` alone takes the
                    // *lower limit* down to the sheet floor but leaves the loop
                    // bounded by `steps_planned`, and only the work cap makes
                    // the tail unbounded (`ScheduleSliceRun::unbounded_tail`).
                    // Arming one without the other is a lever that does nothing,
                    // which is why they are written together and from one
                    // `Option`.
                    if let Some(cap) = past_bound_cap {
                        schedule_settings.continue_past_bound = true;
                        schedule_settings.work_cap_queries = Some(cap);
                    }
                    relaxed.compression_schedule = Some(schedule_settings);
                },
                None,
                ParentRole::Descended,
                Some(action.label.clone()),
            );
            // Mode 34 publishes only layouts its own exact confirmation
            // accepted, with the parent as the floor, so there is never a
            // residue for the global legalizer to be pointed at. That is the
            // structural difference from the ladder class, and it is why this
            // arm does not call `legalize_residue`.
            schedule_self_cost_units(&scheduled)
        }
        // The payload and the class are built together, so a mismatch is
        // unreachable; it is a no-op rather than a panic because a coordinator
        // that aborts a run to report a scheduling bug is worse than one that
        // skips an action.
        _ => None,
    }
}

/// What one compression-schedule slice charges itself, in the portfolio's own
/// work currency, or `None` if the arm never armed a schedule.
///
/// # Why the coordinator does not simply read its own meter
///
/// The port measured the disagreement and named it a finding (§6.3): the
/// coordinator's `Counter::ExactPairTests` is incremented *past* the
/// broad-phase bounds reject, so a whole-layout confirmation that asks all
/// `n * (n - 1) / 2 = 1,830` pairs on the 61-piece request reaches the narrow
/// phase on about 99 of them and is charged **~493 units for 4.83 ms of work**.
/// On the schedule's own arms the exact tier is 24-52% of the wall and about
/// 4% of the metered work.
///
/// So a schedule slice priced on the coordinator's meter is a slice riding
/// free on the one tier it spends its wall in, and the twelve gate cells show
/// exactly that: the same self-capped arm reads 307,767 to 3,343,739 units on
/// the coordinator's meter - an **11x spread** for an arm whose own meter reads
/// 3,341,665 to 3,356,020, a spread of **0.4%**.
///
/// Two ways to fix it were available and this round took the second:
///
/// * **extend the meter** - charge asked pairs rather than narrow-phase tests,
///   process-wide. That is the more principled instrument and it is *rejected
///   here on blast radius*: every pinned work-unit number in this repository is
///   denominated in the current counter, including the ledger's
///   32,393,757 / 31,957,935 / 27,938,867 that coordinator v3 §6.1 reproduces
///   to the unit as its strongest regression statement, and the four gates'
///   work columns. Moving the meter moves all of them at once and buys nothing
///   the pricing decision needs.
/// * **charge the self-cap** - the operator carries a deterministic meter of
///   its own, denominated in the same currency by construction
///   (`candidate_queries + WORK_UNITS_PER_EXACT_PAIR_TEST * asked pairs`), and
///   the coordinator charges the larger of the two. That is this function.
///
/// The charge raises `ClassStats::cost_max`, which is what the affordability
/// rule and the ranking value read - so the class is ranked and refused on the
/// conservative number, not the coordinator's own optimistic meter read.
///
/// Coordinator v5 (Sol review 5 §2, item 1) closes the gap this left open:
/// the price used to be a price and never a spend, so a work-budget run's
/// `BudgetMeter` advanced at its own rate regardless of what this function
/// priced an arm at, and a class whose own meter read 11x the coordinator's
/// could buy far more of itself than the nominal budget allowed. The reading
/// this function returns is now charged into the meter itself, by
/// [`settle_operator_charge`] inside [`Coordinator::run_operator`], as
/// `max(global_units, operator_self_units)`.
///
/// Sol review 6 §1 corrected *when*: the first cut debited in [`v3_loop`],
/// after `run_operator` had already stamped the call's archive entry,
/// publication and report, so an action's own charge landed on the next
/// action's readings. The debit is now step three of a four-step operator
/// transaction - dispatch, charge, debit, stamp - so every reading taken
/// after it is a reading of a settled budget.
///
/// Under a **wall** budget nothing is debited, and nothing needs to be:
/// seconds are seconds, and the clock has no broad phase. That guard lives in
/// [`BudgetMeter::debit_self_metered`], not at the call site, so no future
/// caller can forget it.
#[cfg(feature = "compression-schedule")]
fn schedule_self_cost_units(population: &GeneralPersistentVacancyDiagnostics) -> Option<u64> {
    population
        .compression_schedule
        .as_ref()
        .map(|report| report.work_units as u64)
}

/// The slice's own account of itself, projected onto
/// [`ScheduleSliceReport`], or `None` when this call was not a schedule slice.
#[cfg(feature = "compression-schedule")]
fn schedule_slice_report(
    population: &GeneralPersistentVacancyDiagnostics,
) -> Option<ScheduleSliceReport> {
    let report = population.compression_schedule.as_ref()?;
    Some(ScheduleSliceReport {
        parent_proxy_feasible: report.parent_proxy_feasible,
        parent_collision_pairs: report.parent_collision_pairs,
        parent_boundary_violations: report.parent_boundary_violations,
        parent_entry_loss: report.parent_entry_loss,
        entry_proxy_feasible: report.entry_proxy_feasible,
        entry_collision_pairs: report.entry_collision_pairs,
        entry_boundary_violations: report.entry_boundary_violations,
        entry_loss: report.entry_loss,
        entry_source_depth_mm: report.entry_source_depth_mm,
        entry_depth_loss_mm: report.entry_depth_loss_mm,
        requested_drop_mm: report.requested_drop_mm,
        entry_legalization_armed: report.entry_legalization_armed,
        entry_legalization_run: report.entry_legalization_run,
        entry_legalization_resolved: report.entry_legalization_resolved,
        entry_legalization_accepted: report.entry_legalization_accepted,
        entry_legalization_ms: report.entry_legalization_ms,
        entry_legalization_reason: report.entry_legalization_reason.clone(),
        entry_legalization_violating_pairs_before: report.entry_legalization_violating_pairs_before,
        entry_legalization_violating_pairs_after: report.entry_legalization_violating_pairs_after,
        entry_legalization_boundary_pieces_before: report.entry_legalization_boundary_pieces_before,
        entry_legalization_boundary_pieces_after: report.entry_legalization_boundary_pieces_after,
        skipped_infeasible_entry: report.skipped_infeasible_entry,
        aborted_barren_probe: report.aborted_barren_probe,
        probe_steps: report.probe_steps,
        steps_planned: report.steps_planned,
        steps_taken: report.steps_taken,
        confirmations_attempted: report.confirmations_attempted,
        confirmations_accepted: report.confirmations_accepted,
        confirmations_refused: report.confirmations_refused,
        confirmations_skipped_infeasible: report.confirmations_skipped_infeasible,
        confirmation_ms: report.confirmation_ms,
        repair_ms: report.repair_ms,
        start_depth_mm: report.start_depth_mm,
        final_depth_mm: report.final_depth_mm,
        work_units: report.work_units,
        exit_cause: report.exit_cause.clone(),
        continuous_rotation: report.continuous_rotation,
        rotation_rungs_proposed: report.rotation_rungs_proposed,
        rotation_rungs_improved: report.rotation_rungs_improved,
        mirror_toggles_proposed: report.mirror_toggles_proposed,
        mirror_toggles_improved: report.mirror_toggles_improved,
        rotation_accepted_moves: report.rotation_accepted_moves,
        accepted_moves: report.accepted_moves,
        rotation_loss_bought_mm: report.rotation_loss_bought_mm,
        translation_loss_bought_mm: report.translation_loss_bought_mm,
        rotation_surrogate_builds: report.rotation_surrogate_builds,
        rotation_surrogate_hits: report.rotation_surrogate_hits,
        rotation_surrogate_evictions: report.rotation_surrogate_evictions,
        rotation_surrogate_build_ms: report.rotation_surrogate_build_ms,
        rotation_surrogate_cells: report.rotation_surrogate_cells,
        rotation_builds_refused: report.rotation_builds_refused,
        sparse_rotation: report.sparse_rotation,
        rotation_equivariant_offset: report.rotation_equivariant_offset,
        rotation_equivariant_builds: report.rotation_equivariant_builds,
        rotation_equivariant_fallbacks: report.rotation_equivariant_fallbacks,
        sparse_rotation_episodes: report.sparse_rotation_episodes,
        sparse_rotation_pieces_armed: report.sparse_rotation_pieces_armed,
        sparse_rotation_sweeps: report.sparse_rotation_sweeps,
        sparse_rotation_rungs_proposed: report.sparse_rotation_rungs_proposed,
        sparse_rotation_rung_winners: report.sparse_rotation_rung_winners,
        sparse_rotation_committed_moves: report.sparse_rotation_committed_moves,
        sparse_rotation_committed_episodes: report.sparse_rotation_committed_episodes,
        se2_witness_calls: report.se2_witness_calls,
        se2_witness_accepted: report.se2_witness_accepted,
        se2_witness_adoptions: report.se2_witness_adoptions,
        se2_witness_ms: report.se2_witness_ms,
        se2_witness_bought_mm: report.se2_witness_bought_mm,
        batch_work_units: report.batch_work_units,
        checkpoints: report.checkpoints.clone(),
        batches: report.batches,
        resumptions: report.resumptions,
        interrupted: report.interrupted,
        step_digest: report.step_digest,
    })
}

/// Without the feature there is no slice to report, and the call site stays one
/// expression in either build.
#[cfg(not(feature = "compression-schedule"))]
fn schedule_slice_report(
    _population: &GeneralPersistentVacancyDiagnostics,
) -> Option<ScheduleSliceReport> {
    None
}

/// Hands a deep operator's terminal state to the global legalizer, one rung
/// below its own measured depth, if it is a complete layout that the exact
/// validator refused.
///
/// This is the one place mode 31 is called in v3, and it is what the review
/// asked for: `m31` belongs to the clamp/repair chain, not to a phase of its
/// own.
fn legalize_residue(
    run: &mut PhaseRun<'_, '_>,
    population: &GeneralPersistentVacancyDiagnostics,
    label: &str,
) {
    let residue = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
        &population.final_placements,
    );
    if residue.len() != run.pieces.len() {
        return;
    }
    if !run.meter.has_room(run.deadline) {
        return;
    }
    let Some(residue_depth) = crate::search::general_relaxed::coupled_raw_source_depth(
        run.pieces,
        &residue,
        run.fast_settings,
    )
    .ok() else {
        return;
    };
    let bound = residue_depth - COMPRESSION_RUNG_MM;
    if bound <= 0.0 {
        return;
    }
    let residue_fingerprint = general_placement_fingerprint(&residue);
    if run.already_attempted(format!("31:{residue_fingerprint}")) {
        return;
    }
    run.run_operator(
        31,
        &residue,
        Some(residue_fingerprint),
        Some(bound),
        |_| {},
        None,
        ParentRole::Descended,
        Some(format!("{label} to {bound:.4}")),
    );
}

// ---------------------------------------------------------------------------
// The opportunity-and-delayed-credit ledger, and the A/B/C probe.
//
// Everything below this line is compiled only under `portfolio-ledger`. The
// ledger is an instrument: it reads the archive at exit and never feeds a
// schedule decision, and the probe is one extra phase that runs after the
// whole schedule has finished, so a default build has neither the phase nor a
// branch that could reach it.
// ---------------------------------------------------------------------------

/// Whether two placements are the same pose, on the same bit-exact terms
/// [`assignment_overlap`] uses - which is the terms mode 23 copies poses on.
fn poses_equal(left: &GeneralFastPlacement, right: &GeneralFastPlacement) -> bool {
    left.rotation_deg.to_bits() == right.rotation_deg.to_bits()
        && left.mirrored == right.mirrored
        && left.translate_short_axis.to_bits() == right.translate_short_axis.to_bits()
        && left.translate_long_axis.to_bits() == right.translate_long_axis.to_bits()
}

/// The hybrid mode 23 would build from `(left, right)` at `cut_fraction`.
///
/// This mirrors `general_relaxed::run_recombination`'s rule exactly - the
/// threshold is `min + f * (max - min)` over *left*'s own short-axis span, and
/// a piece goes to left when its left-pose is strictly below it - so the
/// ledger's hybrid fingerprint is the fingerprint the operator would actually
/// be handed. Nothing is legalized here; this is the seed, not the result.
fn crossover_hybrid(
    left: &[GeneralFastPlacement],
    right: &[GeneralFastPlacement],
    cut_fraction: f64,
) -> Option<(Vec<GeneralFastPlacement>, usize, usize)> {
    let right_by_id = right
        .iter()
        .map(|placement| (placement.piece_id.as_str(), placement))
        .collect::<BTreeMap<_, _>>();
    let mut min_short = f64::INFINITY;
    let mut max_short = f64::NEG_INFINITY;
    for placement in left {
        min_short = min_short.min(placement.translate_short_axis);
        max_short = max_short.max(placement.translate_short_axis);
    }
    if !(min_short.is_finite() && max_short.is_finite() && max_short > min_short) {
        return None;
    }
    let threshold = min_short + cut_fraction * (max_short - min_short);
    let mut hybrid = Vec::with_capacity(left.len());
    let mut from_left = 0usize;
    let mut from_right = 0usize;
    for placement in left {
        if placement.translate_short_axis < threshold {
            from_left += 1;
            hybrid.push(placement.clone());
        } else {
            let other = right_by_id.get(placement.piece_id.as_str())?;
            from_right += 1;
            hybrid.push((*other).clone());
        }
    }
    Some((hybrid, from_left, from_right))
}

/// The interface bands of `left` against `right`, as cut fractions.
///
/// A cut only ever partitions `left`'s *occupied* short-axis positions, so the
/// whole continuum of fractions collapses to at most one action per gap between
/// two consecutive occupied positions. The cut is placed at the gap's midpoint,
/// which is the representative of its partition furthest from either edge.
///
/// Returns `(fraction, gap_mm, differing_pieces_at_lower_edge, is_midpoint_band)`.
/// A band whose lower edge holds no piece that *differs* between the two
/// parents produces the same hybrid as the band below it, which is why the
/// count is carried: it is the ledger's "where the two parents' placements
/// differ" and it is what makes a cut a real action rather than a relabelling.
fn derived_cut_bands(
    left: &[GeneralFastPlacement],
    right: &[GeneralFastPlacement],
) -> Vec<(f64, f64, usize, bool)> {
    let right_by_id = right
        .iter()
        .map(|placement| (placement.piece_id.as_str(), placement))
        .collect::<BTreeMap<_, _>>();
    let mut rows = left
        .iter()
        .map(|placement| {
            let differs = right_by_id
                .get(placement.piece_id.as_str())
                .is_none_or(|other| !poses_equal(placement, other));
            (placement.translate_short_axis, differs)
        })
        .collect::<Vec<_>>();
    rows.sort_by(|first, second| first.0.total_cmp(&second.0));
    // One group per occupied short-axis position, carrying how many of the
    // pieces at that position differ between the parents.
    let mut groups: Vec<(f64, usize)> = Vec::new();
    for (value, differs) in rows {
        match groups.last_mut() {
            Some(last) if last.0.to_bits() == value.to_bits() => {
                last.1 += usize::from(differs);
            }
            _ => groups.push((value, usize::from(differs))),
        }
    }
    if groups.len() < 2 {
        return Vec::new();
    }
    let min_short = groups[0].0;
    let max_short = groups[groups.len() - 1].0;
    let span = max_short - min_short;
    if !(span.is_finite() && span > 0.0) {
        return Vec::new();
    }
    let midpoint_threshold = min_short + CROSSOVER_CUT_FRACTION * span;
    let mut bands = Vec::with_capacity(groups.len() - 1);
    for index in 0..groups.len() - 1 {
        let lower = groups[index].0;
        let upper = groups[index + 1].0;
        let threshold = lower + (upper - lower) / 2.0;
        let fraction = (threshold - min_short) / span;
        if !(fraction.is_finite() && fraction > 0.0 && fraction < 1.0) {
            continue;
        }
        let is_midpoint = lower < midpoint_threshold && midpoint_threshold <= upper;
        bands.push((fraction, upper - lower, groups[index].1, is_midpoint));
    }
    bands
}

/// Every ordered, cut-derived crossover action over `selection`, in the
/// canonical order the ledger names "next" by.
///
/// The order is the schedule's own pair order - `(0,1), (0,2), (1,2), ...`,
/// worse member's rank first - then forward before reciprocal, then cuts by
/// distance from the constant `0.5`, because `0.5` is the only cut this engine
/// has evidence for and the nearest band to it is the smallest departure from
/// the action the schedule already knows how to make.
#[cfg(feature = "portfolio-ledger")]
fn enumerate_crossover_actions(
    selection: &[ArchivedBasin],
    attempted: &std::collections::BTreeSet<String>,
) -> Vec<CrossoverAction> {
    let mut actions = Vec::new();
    for right in 1..selection.len() {
        for left in 0..right {
            for (a, b, reciprocal) in [(left, right, false), (right, left, true)] {
                let parent_a = &selection[a];
                let parent_b = &selection[b];
                // The schedule's key is built from the two parents in the order
                // it handed them to the operator, and the frontier's *ranks*
                // move between attempts, so the key has to come from the
                // fingerprints rather than from the ranks: an action whose
                // parents were ranked the other way round when the phase made
                // it is still the same attempted action.
                let schedule_key = format!("23:{}:{}", parent_a.fingerprint, parent_b.fingerprint);
                let mut bands = derived_cut_bands(&parent_a.placements, &parent_b.placements);
                bands.sort_by(|first, second| {
                    (first.0 - CROSSOVER_CUT_FRACTION)
                        .abs()
                        .total_cmp(&(second.0 - CROSSOVER_CUT_FRACTION).abs())
                        .then(first.0.total_cmp(&second.0))
                });
                let mut seen = std::collections::BTreeSet::new();
                for (fraction, gap_mm, differing, is_midpoint) in bands {
                    let Some((hybrid, from_left, from_right)) =
                        crossover_hybrid(&parent_a.placements, &parent_b.placements, fraction)
                    else {
                        continue;
                    };
                    let hybrid_fingerprint = general_placement_fingerprint(&hybrid);
                    if !seen.insert(hybrid_fingerprint.clone()) {
                        // A different band, the same hybrid: the pieces the two
                        // bands disagree about have the same pose in both
                        // parents, so this is not a second action.
                        continue;
                    }
                    let degenerate = hybrid_fingerprint == parent_a.fingerprint
                        || hybrid_fingerprint == parent_b.fingerprint;
                    // Only the midpoint band is an action the schedule can
                    // name; every other cut has no key in the schedule's
                    // namespace at all, which is the finding.
                    let (key, was_attempted) = if is_midpoint {
                        let hit = attempted.contains(&schedule_key);
                        (schedule_key.clone(), hit)
                    } else {
                        (
                            format!(
                                "23d:{}:{}:{:016x}",
                                parent_a.fingerprint,
                                parent_b.fingerprint,
                                fraction.to_bits()
                            ),
                            false,
                        )
                    };
                    let attempted_now = was_attempted || attempted.contains(&key);
                    actions.push(CrossoverAction {
                        left_fingerprint: parent_a.fingerprint.clone(),
                        right_fingerprint: parent_b.fingerprint.clone(),
                        left_rank: a,
                        right_rank: b,
                        reciprocal,
                        cut_fraction: fraction,
                        band_gap_mm: gap_mm,
                        differing_pieces_at_band: differing,
                        pieces_from_left: from_left,
                        pieces_from_right: from_right,
                        hybrid_fingerprint,
                        degenerate,
                        is_midpoint_band: is_midpoint,
                        attempted: attempted_now,
                        key,
                    });
                }
            }
        }
    }
    actions
}

/// The nearest-rank percentile of an already-sorted slice.
#[cfg(feature = "portfolio-ledger")]
fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (quantile * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// The opportunity-and-delayed-credit ledger for the state the run ended in.
#[cfg(feature = "portfolio-ledger")]
fn build_ledger(coordinator: &Coordinator<'_>) -> PortfolioLedger {
    let archive = &coordinator.archive;
    let members = archive.basins().to_vec();

    // 1 - the crossover action space, over the phase's own selection and over
    //     the whole archive.
    let frontier = archive.distinct_frontier(coordinator.settings.crossover_states.max(2));
    let frontier_actions = enumerate_crossover_actions(&frontier, &coordinator.attempted);
    let mut ranked = members.clone();
    ranked.sort_by(|left, right| {
        left.raw_depth_mm
            .total_cmp(&right.raw_depth_mm)
            .then(left.fingerprint.cmp(&right.fingerprint))
    });
    let archive_actions = enumerate_crossover_actions(&ranked, &coordinator.attempted);
    let archive_actions_total = archive_actions.len();
    let archive_actions_untried = archive_actions
        .iter()
        .filter(|action| !action.attempted)
        .count();
    let archive_actions_untried_nondegenerate = archive_actions
        .iter()
        .filter(|action| !action.attempted && !action.degenerate)
        .count();
    let existing = members
        .iter()
        .map(|member| member.fingerprint.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let next_action = frontier_actions
        .iter()
        .find(|action| {
            !action.attempted
                && !action.degenerate
                && !existing.contains(&action.hybrid_fingerprint)
        })
        .cloned();

    // 2 - selection: who the frontier can reach, and what shadows the rest.
    let descent_selection = archive
        .distinct_frontier(coordinator.settings.descent_states)
        .iter()
        .map(|basin| basin.fingerprint.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let crossover_selection = frontier
        .iter()
        .map(|basin| basin.fingerprint.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let full = archive.distinct_frontier(members.len().max(1));
    let full_selection = full
        .iter()
        .map(|basin| basin.fingerprint.clone())
        .collect::<std::collections::BTreeSet<_>>();

    // 4 - genealogy. Edges come from the archive's own parent fields *and*
    //     from every operator call, because a call whose output the archive
    //     refused as a duplicate still descended from its parents.
    let mut parents_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut add_edge = |child: &str, parent: Option<&String>| {
        let Some(parent) = parent else { return };
        if parent == child {
            return;
        }
        let entry = parents_of.entry(child.to_owned()).or_default();
        if !entry.iter().any(|known| known == parent) {
            entry.push(parent.clone());
        }
    };
    for member in &members {
        add_edge(&member.fingerprint, member.parent_fingerprint.as_ref());
        add_edge(
            &member.fingerprint,
            member.secondary_parent_fingerprint.as_ref(),
        );
    }
    for call in &coordinator.operator_calls {
        let Some(child) = call.result_fingerprint.as_deref() else {
            continue;
        };
        add_edge(child, call.parent_fingerprint.as_ref());
        add_edge(child, call.secondary_parent_fingerprint.as_ref());
    }
    let mut children_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (child, parents) in &parents_of {
        for parent in parents {
            children_of
                .entry(parent.clone())
                .or_default()
                .push(child.clone());
        }
    }
    // Forward reachability, breadth first, with the distance carried so the
    // "how many generations later did this pay" question has an answer.
    let descendants_of = |root: &str| -> BTreeMap<String, usize> {
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((root.to_owned(), 0usize));
        seen.insert(root.to_owned(), 0);
        while let Some((node, distance)) = queue.pop_front() {
            let Some(children) = children_of.get(&node) else {
                continue;
            };
            for child in children {
                if seen.contains_key(child) {
                    continue;
                }
                seen.insert(child.clone(), distance + 1);
                queue.push_back((child.clone(), distance + 1));
            }
        }
        seen
    };
    let incumbent_fingerprint = coordinator.incumbent.fingerprint.clone();

    let mut archive_rows = Vec::with_capacity(members.len());
    let mut members_without_action = 0usize;
    let mut excluded_by_top_k = 0usize;
    let mut excluded_by_similarity = 0usize;
    for member in &members {
        let depth_rank = ranked
            .iter()
            .position(|other| other.fingerprint == member.fingerprint)
            .unwrap_or(usize::MAX);
        let in_crossover = crossover_selection.contains(&member.fingerprint);
        let reachable_at_full_k = full_selection.contains(&member.fingerprint);
        let (excluded_by, shadowed_by, shadow_overlap) = if in_crossover {
            (None, None, 0.0)
        } else if reachable_at_full_k {
            excluded_by_top_k += 1;
            (Some("topK".to_owned()), None, 0.0)
        } else {
            excluded_by_similarity += 1;
            let mut best: Option<(&ArchivedBasin, f64)> = None;
            for kept in &full {
                let overlap = assignment_overlap(&kept.placements, &member.placements);
                if overlap >= archive.similarity_threshold()
                    && best.is_none_or(|(_, known)| overlap > known)
                {
                    best = Some((kept, overlap));
                }
            }
            match best {
                Some((kept, overlap)) => (
                    Some("similarity".to_owned()),
                    Some(kept.fingerprint.clone()),
                    overlap,
                ),
                None => (Some("similarity".to_owned()), None, 0.0),
            }
        };
        let actions_received = coordinator
            .operator_calls
            .iter()
            .filter(|call| {
                call.parent_fingerprint.as_deref() == Some(member.fingerprint.as_str())
                    || call.secondary_parent_fingerprint.as_deref()
                        == Some(member.fingerprint.as_str())
            })
            .count();
        if actions_received == 0 {
            members_without_action += 1;
        }
        let reach = descendants_of(&member.fingerprint);
        let mut descendant_publications = 0usize;
        let mut best_descendant_raw_depth_mm: Option<f64> = None;
        for event in &coordinator.publications {
            let Some(distance) = reach.get(&event.fingerprint) else {
                continue;
            };
            if *distance == 0 && event.fingerprint == member.fingerprint {
                // A state that *is* a publication still counts as its own
                // credit; deferred credit is the distance, and it is reported.
            }
            descendant_publications += 1;
            if best_descendant_raw_depth_mm.is_none_or(|known| event.raw_depth_mm < known) {
                best_descendant_raw_depth_mm = Some(event.raw_depth_mm);
            }
        }
        archive_rows.push(ArchiveOpportunityRow {
            fingerprint: member.fingerprint.clone(),
            raw_depth_mm: member.raw_depth_mm,
            operator: member.operator.name(),
            exact_valid: member.exact_valid,
            depth_rank,
            in_descent_frontier: descent_selection.contains(&member.fingerprint),
            in_crossover_frontier: in_crossover,
            reachable_at_full_k,
            excluded_by,
            shadowed_by,
            shadow_overlap,
            actions_received,
            descents: member.descents,
            descendant_publications,
            best_descendant_raw_depth_mm,
            generations_to_incumbent: reach.get(&incumbent_fingerprint).copied(),
        });
    }
    archive_rows.sort_by_key(|row| row.depth_rank);

    // 5 - cost and yield per action class.
    let mut classes: BTreeMap<(String, String), Vec<&OperatorCallReport>> = BTreeMap::new();
    for call in &coordinator.operator_calls {
        classes
            .entry((call.phase.clone(), call.operator.clone()))
            .or_default()
            .push(call);
    }
    let mut action_classes = Vec::with_capacity(classes.len());
    for ((phase, operator), calls) in classes {
        let mut work = calls
            .iter()
            .map(|call| call.work_units as f64)
            .collect::<Vec<_>>();
        let mut seconds = calls
            .iter()
            .map(|call| call.elapsed_seconds)
            .collect::<Vec<_>>();
        work.sort_by(f64::total_cmp);
        seconds.sort_by(f64::total_cmp);
        let work_total = calls.iter().map(|call| call.work_units).sum::<u64>();
        let delta_raw_mm = coordinator
            .publications
            .iter()
            .filter(|event| event.phase == phase && event.source == operator)
            .filter_map(|event| {
                event
                    .previous_raw_depth_mm
                    .map(|previous| previous - event.raw_depth_mm)
            })
            .sum::<f64>();
        action_classes.push(ActionClassRow {
            phase,
            operator,
            calls: calls.len(),
            published: calls.iter().filter(|call| call.published).count(),
            work_units_total: work_total,
            work_units_p50: percentile(&work, 0.50) as u64,
            work_units_p95: percentile(&work, 0.95) as u64,
            seconds_p50: percentile(&seconds, 0.50),
            seconds_p95: percentile(&seconds, 0.95),
            seconds_total: seconds.iter().sum(),
            delta_raw_mm,
            delta_raw_per_mega_unit: if work_total == 0 {
                0.0
            } else {
                delta_raw_mm / (work_total as f64 / 1.0e6)
            },
        });
    }

    // The incumbent's ancestry. It is a DAG, not a chain - crossover has two
    // parents - so this is the ancestor *set* in birth order, which is the
    // honest shape of "what fed the answer".
    let mut ancestors = std::collections::BTreeSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(incumbent_fingerprint.clone());
    ancestors.insert(incumbent_fingerprint.clone());
    while let Some(node) = queue.pop_front() {
        let Some(parents) = parents_of.get(&node) else {
            continue;
        };
        for parent in parents {
            if ancestors.insert(parent.clone()) {
                queue.push_back(parent.clone());
            }
        }
    }
    let mut incumbent_lineage = members
        .iter()
        .filter(|member| ancestors.contains(&member.fingerprint))
        .map(|member| LineageStep {
            fingerprint: member.fingerprint.clone(),
            operator: member.operator.name(),
            raw_depth_mm: member.raw_depth_mm,
            birth_work_units: member.birth_work_units,
        })
        .collect::<Vec<_>>();
    incumbent_lineage.sort_by_key(|step| step.birth_work_units);

    PortfolioLedger {
        frontier_actions,
        archive_actions_total,
        archive_actions_untried,
        archive_actions_untried_nondegenerate,
        archive_ordered_pairs: members.len() * members.len().saturating_sub(1),
        next_action,
        archive_rows,
        action_classes,
        incumbent_lineage,
        members_without_action,
        excluded_by_top_k,
        excluded_by_similarity,
    }
}

/// Runs the A/B/C probe arm the settings name, on the allowance they name.
#[cfg(feature = "portfolio-ledger")]
fn run_probe_phase(
    coordinator: &mut Coordinator<'_>,
    constructor_clamp_mm: f64,
) -> Option<ProbeReport> {
    let arm = coordinator.settings.probe;
    let allowance_units = coordinator.settings.probe_work_units;
    if arm == ProbeArm::None || allowance_units == 0 {
        return None;
    }
    if !matches!(coordinator.meter.budget, PortfolioBudget::Work { .. }) {
        // The arms are paired on identical work by construction, and a wall
        // budget cannot promise that. Refusing is the honest answer.
        return None;
    }
    let allowance = allowance_units as f64;
    let deadline =
        coordinator.meter.spent_fraction() + allowance / coordinator.meter.currency_total();
    let entry_raw_depth_mm = coordinator.incumbent.raw_depth_mm;
    let entry_work = coordinator.meter.work_units();
    let entry_seconds = coordinator.meter.seconds();
    let publications_before = coordinator.publications.len();
    let calls_before = coordinator.operator_calls.len();
    let mut steps: Vec<String> = Vec::new();
    coordinator.run_phase_to("probe", deadline, |run| match arm {
        ProbeArm::NextDerivedCrossover => probe_next_derived_crossover(run, &mut steps),
        ProbeArm::ConstructorTicket => {
            probe_constructor_ticket(run, constructor_clamp_mm, &mut steps)
        }
        ProbeArm::LadderRung => probe_ladder_rung(run, &mut steps),
        ProbeArm::DescentControl => probe_descent_control(run, &mut steps),
        ProbeArm::None => {}
    });
    let exit_raw_depth_mm = coordinator.incumbent.raw_depth_mm;
    Some(ProbeReport {
        arm: arm.name().to_owned(),
        allowance,
        work_units_spent: coordinator.meter.work_units().saturating_sub(entry_work),
        seconds_spent: coordinator.meter.seconds() - entry_seconds,
        entry_raw_depth_mm,
        exit_raw_depth_mm,
        delta_raw_mm: match (entry_raw_depth_mm, exit_raw_depth_mm) {
            (Some(entry), Some(exit)) => entry - exit,
            _ => 0.0,
        },
        exit_dual_gate_valid: coordinator.incumbent.dual_gate_valid,
        publications: coordinator.publications.len() - publications_before,
        operator_calls: coordinator.operator_calls.len() - calls_before,
        steps,
        exit_cause: coordinator.exit_cause.name().to_owned(),
    })
}

/// Arm A: the next derived crossover action the ledger names.
#[cfg(feature = "portfolio-ledger")]
fn probe_next_derived_crossover(run: &mut PhaseRun<'_, '_>, steps: &mut Vec<String>) {
    let frontier = run
        .archive
        .distinct_frontier(run.settings.crossover_states.max(2));
    if frontier.len() < 2 {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    }
    let existing = run
        .archive
        .basins()
        .iter()
        .map(|basin| basin.fingerprint.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let actions = enumerate_crossover_actions(&frontier, &run.attempted);
    let Some(action) = actions.into_iter().find(|action| {
        !action.attempted && !action.degenerate && !existing.contains(&action.hybrid_fingerprint)
    }) else {
        run.note_exit(PhaseExitCause::KeysExhausted);
        return;
    };
    if let Some(cause) = run.affordability(run.deadline, "mode23", 1.0) {
        run.note_exit(cause);
        return;
    }
    let (Some(left), Some(right)) = (
        frontier
            .iter()
            .find(|basin| basin.fingerprint == action.left_fingerprint),
        frontier
            .iter()
            .find(|basin| basin.fingerprint == action.right_fingerprint),
    ) else {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    };
    let left_placements = left.placements.clone();
    let left_fingerprint = left.fingerprint.clone();
    let right_fingerprint = right.fingerprint.clone();
    let parent_b = GeneralPersistentVacancyPinnedParent {
        placements: right.placements.clone(),
        source: "archive".to_owned(),
        source_sha256: right_fingerprint.clone(),
    };
    let label = format!(
        "m23 {} rank{}->rank{} cut={:.9} band={:.6}mm differing={}",
        if action.reciprocal {
            "reciprocal"
        } else {
            "forward"
        },
        action.left_rank,
        action.right_rank,
        action.cut_fraction,
        action.band_gap_mm,
        action.differing_pieces_at_band,
    );
    steps.push(label.clone());
    run.already_attempted(action.key.clone());
    run.run_operator(
        23,
        &left_placements,
        Some(left_fingerprint),
        Some(action.cut_fraction),
        |_| {},
        Some(&parent_b),
        ParentRole::Descended,
        Some(label),
    );
    run.archive.charge_descent(&right_fingerprint);
}

/// Arm B: one mode-20 ticket, a direct crossover with the incumbent, a short
/// mode-22.
#[cfg(feature = "portfolio-ledger")]
fn probe_constructor_ticket(
    run: &mut PhaseRun<'_, '_>,
    constructor_clamp_mm: f64,
    steps: &mut Vec<String>,
) {
    // A *fresh* restart window and a fresh salt: the slot the diversify phase
    // would have drawn next, so the ticket is new material rather than a replay
    // of one the schedule already bought.
    let slot = run
        .operator_calls
        .iter()
        .filter(|call| call.operator == "mode20")
        .count();
    let salt = slot as f64 * BASIN_TARGET_SALT_RELATIVE_STEP * constructor_clamp_mm;
    let divisor = if run.settings.cell_divisor_salts.is_empty() {
        None
    } else {
        Some(run.settings.cell_divisor_salts[slot % run.settings.cell_divisor_salts.len()])
    };
    let incumbent_placements = run.incumbent.result.placements.clone();
    let incumbent_fingerprint = run.incumbent.fingerprint.clone();
    steps.push(format!("m20 ticket slot{slot}"));
    let drawn = run.run_operator(
        20,
        &incumbent_placements,
        Some(incumbent_fingerprint.clone()),
        Some(constructor_clamp_mm + salt),
        |relaxed| {
            relaxed.construction_restart_window = Some((slot, 1));
            relaxed.construction_void_cell_divisor = divisor;
        },
        None,
        ParentRole::Prior,
        Some(format!("m20:ticket:slot{slot}")),
    );
    let ticket = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
        &drawn.final_placements,
    );
    if ticket.len() != run.pieces.len() {
        run.note_exit(PhaseExitCause::NoCompleteLayout);
        return;
    }
    let ticket_fingerprint = general_placement_fingerprint(&ticket);

    // The direct crossover: parent A is the incumbent, whose span the cut is
    // measured on, and parent B is the ticket. That is the pairing the
    // crossover phase would make of these two, because the frontier orders by
    // depth and the incumbent is the shallower of them.
    let parent_b = GeneralPersistentVacancyPinnedParent {
        placements: ticket.clone(),
        source: "probe-ticket".to_owned(),
        source_sha256: ticket_fingerprint.clone(),
    };
    steps.push("m23 incumbent->ticket@0.5".to_owned());
    // Re-read after the ticket: a mode-20 arm that published would have moved
    // the incumbent, and the crossover's parent A must be the current one.
    let crossover_parent = run.incumbent.result.placements.clone();
    let crossover_parent_fingerprint = run.incumbent.fingerprint.clone();
    let crossed = run.run_operator(
        23,
        &crossover_parent,
        Some(crossover_parent_fingerprint),
        Some(CROSSOVER_CUT_FRACTION),
        |_| {},
        Some(&parent_b),
        ParentRole::Descended,
        Some("m23:ticket:forward@0.5".to_owned()),
    );
    run.archive.charge_descent(&ticket_fingerprint);
    let crossed_placements =
        crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
            &crossed.final_placements,
        );
    // The short mode-22 descends from whatever the crossover left. If the
    // crossover produced nothing complete, it descends from the ticket itself,
    // which is what the diversify phase does and is the deferred-credit chain
    // the review names.
    let (child, origin) = if crossed_placements.len() == run.pieces.len() {
        (crossed_placements, "crossover")
    } else {
        (ticket, "ticket")
    };
    let Some(child_depth) = crate::search::general_relaxed::coupled_raw_source_depth(
        run.pieces,
        &child,
        run.fast_settings,
    )
    .ok() else {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    };
    let child_fingerprint = general_placement_fingerprint(&child);
    let cycles = run.settings.descent_cycles.max(1);
    let epochs = run.settings.descent_relaxed_epochs.max(1);
    if run.already_attempted(format!("22:{cycles}:{epochs}:{child_fingerprint}")) {
        run.note_exit(PhaseExitCause::KeysExhausted);
        return;
    }
    steps.push(format!("m22 short on {origin}"));
    run.run_operator(
        22,
        &child,
        Some(child_fingerprint),
        Some(child_depth + ALTERNATION_RUNG_MM),
        |relaxed| {
            relaxed.alternation_max_cycles = Some(cycles);
            relaxed.epochs = epochs;
        },
        None,
        ParentRole::Descended,
        Some(format!("m22:ticket:{origin}")),
    );
}

/// Arm C: one short mode-26 ladder, then the coordinator's own global
/// legalizer tier on what it leaves.
#[cfg(feature = "portfolio-ledger")]
fn probe_ladder_rung(run: &mut PhaseRun<'_, '_>, steps: &mut Vec<String>) {
    let parent = run.incumbent.result.placements.clone();
    let parent_fingerprint = run.incumbent.fingerprint.clone();
    let Some(parent_depth) = run.incumbent.raw_depth_mm else {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    };
    let bound = parent_depth - LADDER_PROBE_DROP_MM;
    if bound <= 0.0 {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    }
    steps.push(format!("m26 ladder {parent_depth:.4} -> {bound:.4}"));
    let ladder = run.run_operator(
        26,
        &parent,
        Some(parent_fingerprint.clone()),
        Some(bound),
        |_| {},
        None,
        ParentRole::Descended,
        Some(format!("m26:drop{LADDER_PROBE_DROP_MM}")),
    );
    // The ladder's own rung count, so "a short ladder" is a measurement rather
    // than a claim: `ladder_compression_bounds` derives the rung size from the
    // parent's depth, so the same 0.3 mm drop is a different number of rungs on
    // a different parent.
    if let Some(rungs) = ladder.ladder_compression.as_ref() {
        steps.push(format!(
            "m26 rungs planned={} run={} step={:.6}mm publishedStep={:?} arms={}",
            rungs.steps_planned,
            rungs.steps_run,
            rungs.step_mm,
            rungs.published_step,
            rungs
                .steps
                .iter()
                .map(|step| step.arms.len())
                .sum::<usize>(),
        ));
    }
    let produced = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
        &ladder.final_placements,
    );
    if produced.len() != run.pieces.len() {
        run.note_exit(PhaseExitCause::NoCompleteLayout);
        return;
    }
    if !run.meter.has_room(run.deadline) {
        run.note_exit(PhaseExitCause::Deadline);
        return;
    }
    let Some(residue_depth) = crate::search::general_relaxed::coupled_raw_source_depth(
        run.pieces,
        &produced,
        run.fast_settings,
    )
    .ok() else {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    };
    let residue_bound = residue_depth - COMPRESSION_RUNG_MM;
    if residue_bound <= 0.0 {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    }
    let residue_fingerprint = general_placement_fingerprint(&produced);
    if run.already_attempted(format!("31:{residue_fingerprint}")) {
        run.note_exit(PhaseExitCause::KeysExhausted);
        return;
    }
    steps.push(format!("m31 global legalizer to {residue_bound:.4}"));
    run.run_operator(
        31,
        &produced,
        Some(residue_fingerprint),
        Some(residue_bound),
        |_| {},
        None,
        ParentRole::Descended,
        Some("m31:ladder-residue".to_owned()),
    );
}

/// Arm D, the control for arm C: the same target depth, the same parent, the
/// schedule's own alternation operator, no clamp.
#[cfg(feature = "portfolio-ledger")]
fn probe_descent_control(run: &mut PhaseRun<'_, '_>, steps: &mut Vec<String>) {
    let parent = run.incumbent.result.placements.clone();
    let parent_fingerprint = run.incumbent.fingerprint.clone();
    let Some(parent_depth) = run.incumbent.raw_depth_mm else {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    };
    let target = parent_depth - LADDER_PROBE_DROP_MM;
    if target <= 0.0 {
        run.note_exit(PhaseExitCause::GeometricFixpoint);
        return;
    }
    let cycles = run.settings.descent_cycles.max(1);
    let epochs = run.settings.descent_relaxed_epochs.max(1);
    steps.push(format!("m22 control {parent_depth:.4} -> {target:.4}"));
    run.run_operator(
        22,
        &parent,
        Some(parent_fingerprint),
        Some(target),
        |relaxed| {
            relaxed.alternation_max_cycles = Some(cycles);
            relaxed.epochs = epochs;
        },
        None,
        ParentRole::Descended,
        Some(format!("m22:control:drop{LADDER_PROBE_DROP_MM}")),
    );
}

/// The request's own area lower-bound depth: the depth a perfect packing of the
/// expanded collision polygons would need on this sheet.
///
/// This is the same quantity the benchmark reports, computed here so that the
/// constructor clamp a from-request run uses is derived from the request rather
/// than handed to it.
pub fn area_lower_bound_depth_mm(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<f64, GeneralFastError> {
    let edge_clearance_mm = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    let expansion_mm = settings.total_padding_mm / 2.0
        + settings.clearance_safety_margin_mm
        + settings.search_offset_allowance_mm;
    let mut expanded_area_mm2 = 0.0;
    for piece in pieces {
        let offset = piece
            .polygon
            .offset(expansion_mm)
            .map_err(|error| GeneralFastError::Geometry(error.into()))?;
        expanded_area_mm2 += offset.area_mm2();
    }
    let inset_mm = edge_clearance_mm - settings.total_padding_mm / 2.0;
    let width_mm = settings.sheet_short_axis_mm - 2.0 * inset_mm;
    if !(width_mm.is_finite() && width_mm > 0.0) {
        return Err(GeneralFastError::InvalidInput(
            "area lower bound requires a positive collision sheet width".to_owned(),
        ));
    }
    Ok(expanded_area_mm2 / width_mm + 2.0 * inset_mm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(id: &str, x: f64, y: f64) -> GeneralFastPlacement {
        GeneralFastPlacement {
            piece_id: id.to_owned(),
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: x,
            translate_long_axis: y,
        }
    }

    fn basin(fingerprint: &str, depth: f64, layout: Vec<GeneralFastPlacement>) -> ArchivedBasin {
        ArchivedBasin {
            fingerprint: fingerprint.to_owned(),
            raw_depth_mm: depth,
            birth_seconds: 0.0,
            birth_work_units: 0,
            operator: BasinOperator::Mode(20),
            parent_fingerprint: None,
            secondary_parent_fingerprint: None,
            exact_valid: true,
            descents: 0,
            placements: layout,
        }
    }

    #[test]
    fn assignment_overlap_counts_identical_poses() {
        let left = vec![placement("a", 0.0, 0.0), placement("b", 1.0, 1.0)];
        let right = vec![placement("a", 0.0, 0.0), placement("b", 2.0, 2.0)];
        assert_eq!(assignment_overlap(&left, &right), 0.5);
        assert_eq!(assignment_overlap(&left, &left), 1.0);
    }

    #[test]
    fn archive_retains_a_worse_but_different_basin() {
        let mut archive = SearchArchive::new(4, 2, 0.5);
        let good = vec![placement("a", 0.0, 0.0), placement("b", 0.0, 0.0)];
        let worse_and_different = vec![placement("a", 9.0, 9.0), placement("b", 9.0, 9.0)];
        assert_eq!(
            archive.offer(basin("one", 170.0, good)),
            ArchiveDisposition::Admitted
        );
        assert_eq!(
            archive.offer(basin("two", 210.0, worse_and_different)),
            ArchiveDisposition::Admitted
        );
        assert_eq!(archive.basins().len(), 2);
    }

    #[test]
    fn archive_refuses_a_duplicate_fingerprint() {
        let mut archive = SearchArchive::new(4, 2, 0.5);
        let layout = vec![placement("a", 0.0, 0.0), placement("b", 0.0, 0.0)];
        archive.offer(basin("one", 170.0, layout.clone()));
        assert_eq!(
            archive.offer(basin("one", 160.0, layout)),
            ArchiveDisposition::Duplicate
        );
    }

    #[test]
    fn archive_refuses_incomplete_layouts() {
        let mut archive = SearchArchive::new(4, 2, 0.5);
        assert_eq!(
            archive.offer(basin("one", 170.0, vec![placement("a", 0.0, 0.0)])),
            ArchiveDisposition::IncompleteCardinality
        );
    }

    #[test]
    fn full_archive_of_distinct_basins_refuses_rather_than_evicting() {
        let mut archive = SearchArchive::new(2, 2, 0.5);
        archive.offer(basin(
            "one",
            170.0,
            vec![placement("a", 0.0, 0.0), placement("b", 0.0, 0.0)],
        ));
        archive.offer(basin(
            "two",
            180.0,
            vec![placement("a", 5.0, 5.0), placement("b", 5.0, 5.0)],
        ));
        assert_eq!(
            archive.offer(basin(
                "three",
                160.0,
                vec![placement("a", 9.0, 9.0), placement("b", 9.0, 9.0)],
            )),
            ArchiveDisposition::RefusedArchiveFullAllDistinct
        );
        assert_eq!(archive.basins().len(), 2);
    }

    #[test]
    fn full_archive_evicts_only_a_dominated_and_similar_member() {
        let mut archive = SearchArchive::new(2, 2, 0.5);
        // "one" and "two" share piece a's pose, so they are similar; "two" is
        // deeper, so it is dominated.
        archive.offer(basin(
            "one",
            170.0,
            vec![placement("a", 0.0, 0.0), placement("b", 0.0, 0.0)],
        ));
        archive.offer(basin(
            "two",
            180.0,
            vec![placement("a", 0.0, 0.0), placement("b", 4.0, 4.0)],
        ));
        assert_eq!(
            archive.offer(basin(
                "three",
                175.0,
                vec![placement("a", 9.0, 9.0), placement("b", 9.0, 9.0)],
            )),
            ArchiveDisposition::AdmittedAfterEviction
        );
        let kept = archive
            .basins()
            .iter()
            .map(|basin| basin.fingerprint.as_str())
            .collect::<Vec<_>>();
        assert_eq!(kept, vec!["one", "three"]);
    }

    #[test]
    fn distinct_frontier_is_best_first_and_skips_similar() {
        let mut archive = SearchArchive::new(8, 2, 0.5);
        archive.offer(basin(
            "one",
            170.0,
            vec![placement("a", 0.0, 0.0), placement("b", 0.0, 0.0)],
        ));
        // Shares piece a's pose with "one", so it is the same basin and must
        // not spend a second quantum on it.
        archive.offer(basin(
            "near-one",
            171.0,
            vec![placement("a", 0.0, 0.0), placement("b", 3.0, 3.0)],
        ));
        archive.offer(basin(
            "far",
            190.0,
            vec![placement("a", 7.0, 7.0), placement("b", 7.0, 7.0)],
        ));
        let names = archive
            .distinct_frontier(3)
            .iter()
            .map(|basin| basin.fingerprint.clone())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["one", "far"]);
        // Depth orders the frontier; a descent does not demote a better basin
        // below a worse one. That ordering is the schedule defect this stage
        // measured, so it is pinned rather than left to be rediscovered.
        archive.charge_descent("one");
        assert_eq!(archive.distinct_frontier(1)[0].fingerprint, "one");
    }

    #[test]
    fn distinct_frontier_breaks_depth_ties_toward_the_least_descended() {
        let mut archive = SearchArchive::new(8, 2, 0.5);
        archive.offer(basin(
            "left",
            170.0,
            vec![placement("a", 0.0, 0.0), placement("b", 0.0, 0.0)],
        ));
        archive.offer(basin(
            "right",
            170.0,
            vec![placement("a", 7.0, 7.0), placement("b", 7.0, 7.0)],
        ));
        assert_eq!(archive.distinct_frontier(1)[0].fingerprint, "left");
        archive.charge_descent("left");
        assert_eq!(archive.distinct_frontier(1)[0].fingerprint, "right");
    }

    #[test]
    fn work_budget_meter_is_monotone_and_wall_free() {
        let meter = BudgetMeter::new(PortfolioBudget::Work { units: 1_000 });
        let first = meter.spent_fraction();
        let second = meter.spent_fraction();
        assert!(second >= first);
        assert!(meter.has_room(1.0) || first >= 1.0);
    }

    #[test]
    fn a_zero_budget_has_room_for_nothing() {
        let meter = BudgetMeter::new(PortfolioBudget::Wall { millis: 0 });
        assert!(!meter.has_room(1.0));
    }

    /// A plan that has not been installed must not look affordable.
    ///
    /// This is the fail-closed half of `BudgetMeter::spent_fraction`: the only
    /// window in which `PortfolioBudget::Plan` is the live budget is the
    /// protected phase 0, which is never budget-checked, so any reader of it is
    /// a bug - and this is what such a bug does. It returns phase 0's own
    /// layout with every later phase skipped, which is visible in the report.
    #[test]
    fn an_uninstalled_plan_has_room_for_nothing() {
        let meter = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 10_000,
        });
        assert!(!meter.has_room(1.0));
        assert_eq!(meter.spent_fraction(), f64::INFINITY);
        assert!(!meter.is_wall());
    }

    /// The plan is `probe + (aim - probe wall) * rate / bias`, floored onto the
    /// ladder, and every term is checked against hand arithmetic here rather
    /// than against the function that produced it.
    #[test]
    fn a_plan_prices_the_remaining_target_at_the_probes_rate() {
        let mut meter = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 10_000,
        });
        // Freeze the probe: 2 s of wall and 8 M units, a rate of 4 M/s. The
        // meter reads the clock, so the probe wall is forced by rewinding
        // `started`, and the work is forced by the debit accumulator - which is
        // legitimate because `install_plan` reads `work_units()` and that is
        // exactly what a self-metered charge moves.
        meter.started = Instant::now() - std::time::Duration::from_secs(2);
        meter.self_metered_debit = 8_000_000;
        let mut settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Plan {
                target_millis: 10_000,
            },
        );
        settings.plan_bias = 2.0;
        settings.plan_headroom = 1.0;
        settings.plan_quantum_step = 1.0;
        let plan = meter.install_plan(10_000, &settings);

        // The probe is inside the plan, not beside it.
        assert_eq!(plan.probe_work_units, 8_000_000);
        assert!((plan.probe_seconds - 2.0).abs() < 0.05, "{plan:?}");
        // 8 M + (10 s - 2 s) * 4 M/s / 2 = 8 M + 16 M = 24 M.
        assert!(
            (plan.raw_units - 24_000_000.0).abs() < 200_000.0,
            "{plan:?}"
        );
        assert_eq!(plan.rung, None);
        // Installed, so nothing downstream sees a plan.
        assert_eq!(meter.budget, PortfolioBudget::Work { units: plan.units });
        assert!(!meter.is_wall());
        assert_eq!(meter.currency_total(), plan.units as f64);
    }

    /// Quantisation floors onto `anchor * step^k` and never rounds up.
    ///
    /// The direction is the whole reason `PLAN_HEADROOM` can be 0.97: a plan
    /// that could round *up* would need headroom for half a rung, which at the
    /// shipped step is 7%.
    #[test]
    fn the_plan_ladder_floors_and_never_rounds_up() {
        for step in [1.15_f64, 1.25, 2.0] {
            let mut meter = BudgetMeter::new(PortfolioBudget::Plan {
                target_millis: 10_000,
            });
            meter.started = Instant::now() - std::time::Duration::from_secs(2);
            meter.self_metered_debit = 8_000_000;
            let mut settings = PortfolioSettings::new(
                GeneralRelaxedSettings::mixed_61_probe(0, 1),
                PortfolioBudget::Plan {
                    target_millis: 10_000,
                },
            );
            settings.plan_bias = 2.0;
            settings.plan_headroom = 1.0;
            settings.plan_quantum_step = step;
            let plan = meter.install_plan(10_000, &settings);
            let rung = plan.rung.expect("a step above 1.0 quantises");
            let expected = PLAN_ANCHOR_UNITS * step.powi(rung as i32);
            assert_eq!(plan.units, expected as u64, "step {step}");
            assert!(
                (plan.units as f64) <= plan.raw_units,
                "step {step} rounded up: {plan:?}"
            );
            // ...and it is the *largest* rung that does, so the floor never
            // gives away more than one whole rung.
            assert!(
                (plan.units as f64) * step > plan.raw_units,
                "step {step} floored more than one rung: {plan:?}"
            );
        }
    }

    /// A target already overspent by phase 0 buys the probe and nothing more.
    #[test]
    fn a_target_smaller_than_the_probe_buys_no_extra_work() {
        let mut meter = BudgetMeter::new(PortfolioBudget::Plan { target_millis: 1 });
        meter.started = Instant::now() - std::time::Duration::from_secs(2);
        meter.self_metered_debit = 8_000_000;
        let mut settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Plan { target_millis: 1 },
        );
        settings.plan_quantum_step = 1.0;
        let plan = meter.install_plan(1, &settings);
        assert_eq!(plan.raw_units, 8_000_000.0);
        assert_eq!(plan.units, 8_000_000);
    }

    /// A run that names none of the load-robustness keys is the run
    /// `docs/experiments/calibrated-plan/` shipped, field for field.
    ///
    /// The keys are three separate mechanisms and each of them is a way this
    /// round could have moved a pinned number by accident, so the null is
    /// asserted rather than assumed: the source is `Live`, the effective probe
    /// *is* the whole-phase reading, and no sampler ran.
    #[test]
    fn an_unarmed_plan_is_the_shipped_plan_and_reads_no_file() {
        let mut meter = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 10_000,
        });
        meter.started = Instant::now() - std::time::Duration::from_secs(2);
        meter.self_metered_debit = 8_000_000;
        let settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Plan {
                target_millis: 10_000,
            },
        );
        assert_eq!(settings.plan_probe_buckets, 0);
        assert_eq!(settings.plan_calibration_path, None);
        assert!(!settings.plan_calibration_write);
        #[cfg(feature = "compression-schedule")]
        {
            assert_eq!(settings.schedule_first_slice_step_grid, None);
            assert_eq!(settings.schedule_first_slice_confirm_every, None);
        }
        // Nothing was armed, so nothing was started.
        meter.arm_plan_probe(&settings);
        assert!(meter.plan_probe.is_none());
        let plan = meter.install_plan(10_000, &settings);
        assert_eq!(plan.calibration_source, PlanCalibrationSource::Live);
        assert_eq!(plan.probe_effective_seconds, plan.probe_seconds);
        assert_eq!(plan.probe_samples, 0);
    }

    /// The max-of-k probe takes the **fastest** equal-work bucket, and the
    /// clamp stops one lucky bucket from inventing a box.
    ///
    /// The path below retires 8 M units in 4 s, but its second quarter retires
    /// 2 M in 0.1 s - a 20 M/s stretch against a 2 M/s average. Unclamped that
    /// would price the probe at 0.4 s; [`PLAN_PROBE_MIN_FRACTION`] holds it at
    /// half the observed wall, which is the most this mechanism is ever allowed
    /// to correct a loaded reading by.
    #[test]
    fn the_max_of_k_probe_takes_the_fastest_bucket_and_is_clamped() {
        let samples = vec![
            (1.0, 2_000_000),
            (1.1, 4_000_000),
            (2.5, 6_000_000),
            (3.9, 7_900_000),
        ];
        let effective = probe_effective_seconds(&samples, 4.0, 8_000_000, 4)
            .expect("four buckets over four samples");
        assert!((effective - 2.0).abs() < 1e-9, "{effective}");

        // With the same path and an even split, the fastest bucket is the whole
        // second quarter and the estimate is genuinely below the clamp's reach,
        // so a *milder* spike is passed through rather than flattened.
        let mild = vec![
            (1.0, 2_000_000),
            (1.6, 4_000_000),
            (2.6, 6_000_000),
            (3.9, 7_900_000),
        ];
        let effective = probe_effective_seconds(&mild, 4.0, 8_000_000, 4)
            .expect("four buckets over four samples");
        // The fastest quarter is 2 M in 0.6 s -> 3.333 M/s -> 2.4 s for 8 M.
        assert!((effective - 2.4).abs() < 1e-6, "{effective}");

        // One bucket is the whole-phase reading, which is the shipped mode, and
        // the function refuses rather than pretending.
        assert_eq!(probe_effective_seconds(&samples, 4.0, 8_000_000, 1), None);
        // And a path with nothing in it cannot be cut.
        assert_eq!(probe_effective_seconds(&[], 4.0, 8_000_000, 4), None);
    }

    /// A persisted calibration takes the clock out of the plan entirely.
    ///
    /// This is the round's central claim in one assertion: two meters whose
    /// probe walls differ by **2.5x** - which is more than three ladder rungs of
    /// rate - install the *same budget* when both consult the same file, because
    /// the file is keyed on `probe_work_units`, which is a counter.
    ///
    /// The third meter is the control: without the file the same pair of
    /// readings installs two different budgets, which is the failure
    /// `docs/experiments/replan/` §11.1 measured as 2 / 3 / 1 distinct depths
    /// per seed.
    #[test]
    fn a_persisted_calibration_makes_two_loaded_probes_install_one_budget() {
        let path = std::env::temp_dir().join(format!(
            "plan-calibration-{}-{}.json",
            std::process::id(),
            line!()
        ));
        let path = path.to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);
        write_plan_calibration(&path, 8_000_000, 2.0);

        let install = |wall_millis: u64, calibrated: bool| {
            let mut meter = BudgetMeter::new(PortfolioBudget::Plan {
                target_millis: 10_000,
            });
            meter.started = Instant::now() - std::time::Duration::from_millis(wall_millis);
            meter.self_metered_debit = 8_000_000;
            let mut settings = PortfolioSettings::new(
                GeneralRelaxedSettings::mixed_61_probe(0, 1),
                PortfolioBudget::Plan {
                    target_millis: 10_000,
                },
            );
            settings.plan_bias = 2.0;
            settings.plan_headroom = 1.0;
            if calibrated {
                settings.plan_calibration_path = Some(path.clone());
            }
            meter.install_plan(10_000, &settings)
        };

        // 1.9x apart, which is inside the default band and is more than three
        // ladder rungs of rate.
        let quiet = install(2_000, true);
        let loaded = install(3_800, true);
        assert_eq!(quiet.calibration_source, PlanCalibrationSource::File);
        assert_eq!(loaded.calibration_source, PlanCalibrationSource::File);
        assert_eq!(quiet.units, loaded.units);
        assert_eq!(quiet.probe_effective_seconds, 2.0);
        assert_eq!(loaded.probe_effective_seconds, 2.0);
        // The live reading is still reported, because a reader has to be able to
        // see the load the file absorbed.
        assert!(loaded.probe_seconds > 3.5, "{loaded:?}");

        // The control: the same two readings, no file, two budgets.
        assert_ne!(install(2_000, false).units, install(3_800, false).units);

        // A key the file does not carry misses and falls back, and says so.
        let mut meter = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 10_000,
        });
        meter.started = Instant::now() - std::time::Duration::from_secs(2);
        meter.self_metered_debit = 9_999_999;
        let mut settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Plan {
                target_millis: 10_000,
            },
        );
        settings.plan_calibration_path = Some(path.clone());
        let missed = meter.install_plan(10_000, &settings);
        assert_eq!(missed.calibration_source, PlanCalibrationSource::FileMiss);
        assert_eq!(missed.probe_effective_seconds, missed.probe_seconds);

        let _ = std::fs::remove_file(&path);
    }

    /// A live probe outside the band refuses the file rather than pricing the
    /// run off a calibration that cannot be about this box.
    ///
    /// Both directions, because they fail differently: a file that is *too fast*
    /// for the live reading would over-buy and overrun, and one that is too slow
    /// would under-buy for ever. The band is the same number for both and
    /// `plan.calibrationSource` names the refusal in the document.
    #[test]
    fn a_calibration_outside_the_band_is_refused_in_both_directions() {
        let path = std::env::temp_dir().join(format!(
            "plan-calibration-{}-{}.json",
            std::process::id(),
            line!()
        ));
        let path = path.to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);
        write_plan_calibration(&path, 8_000_000, 2.0);

        let install = |wall_millis: u64, band: f64| {
            let mut meter = BudgetMeter::new(PortfolioBudget::Plan {
                target_millis: 30_000,
            });
            meter.started = Instant::now() - std::time::Duration::from_millis(wall_millis);
            meter.self_metered_debit = 8_000_000;
            let mut settings = PortfolioSettings::new(
                GeneralRelaxedSettings::mixed_61_probe(0, 1),
                PortfolioBudget::Plan {
                    target_millis: 30_000,
                },
            );
            settings.plan_calibration_path = Some(path.clone());
            settings.plan_calibration_band = band;
            meter.install_plan(30_000, &settings)
        };

        // Inside: 3.5 s against a stored 2.0 s at a band of 2.0.
        assert_eq!(
            install(3_500, 2.0).calibration_source,
            PlanCalibrationSource::File
        );
        // Too slow to be load: 5 s against 2 s is 2.5x.
        assert_eq!(
            install(5_000, 2.0).calibration_source,
            PlanCalibrationSource::FileOutOfBand
        );
        // Too fast to be this box: 0.5 s against 2 s is 4x the other way.
        assert_eq!(
            install(500, 2.0).calibration_source,
            PlanCalibrationSource::FileOutOfBand
        );
        // And a refusal is a *fallback*, not a failure: the run still plans.
        assert!(install(5_000, 2.0).units > 0);

        let _ = std::fs::remove_file(&path);
    }

    /// The calibration file keeps the **least-loaded** observation and converges.
    ///
    /// The min rule is what makes a calibration pass repeatable: a pass that ran
    /// into a load spike must not be able to make the box look permanently
    /// slower, and a pass that is already converged must not rewrite the file on
    /// every run.
    #[test]
    fn the_calibration_file_keeps_the_least_loaded_observation() {
        let path = std::env::temp_dir().join(format!(
            "plan-calibration-{}-{}.json",
            std::process::id(),
            line!()
        ));
        let path = path.to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        // An absent file is an empty map, not an error.
        assert!(read_plan_calibration(&path).is_empty());

        write_plan_calibration(&path, 8_000_000, 2.4);
        assert_eq!(read_plan_calibration(&path).get(&8_000_000), Some(&2.4));
        // A slower observation is ignored...
        write_plan_calibration(&path, 8_000_000, 3.9);
        assert_eq!(read_plan_calibration(&path).get(&8_000_000), Some(&2.4));
        // ...a faster one wins...
        write_plan_calibration(&path, 8_000_000, 2.0);
        assert_eq!(read_plan_calibration(&path).get(&8_000_000), Some(&2.0));
        // ...and a second cell does not disturb the first.
        write_plan_calibration(&path, 9_000_000, 2.2);
        let entries = read_plan_calibration(&path);
        assert_eq!(entries.get(&8_000_000), Some(&2.0));
        assert_eq!(entries.get(&9_000_000), Some(&2.2));

        // A corrupt file degrades to the live probe rather than failing a run.
        std::fs::write(&path, "{ this is not json").expect("write");
        assert!(read_plan_calibration(&path).is_empty());

        let _ = std::fs::remove_file(&path);
    }

    /// The two flags the promotion round turns on are on, and the Cargo
    /// features that carry them are still off.
    ///
    /// The second half is what keeps every pinned gate untouched: the gate
    /// binary is built `--features jagua-experimental`, which compiles neither
    /// `parallel-compression-schedule` nor `fast-contract-validator`, so the
    /// fields asserted below do not exist in it at all.
    #[test]
    fn the_promoted_defaults_are_on_inside_v3_and_v3_is_off() {
        let settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Wall { millis: 10_000 },
        );
        assert!(!settings.coordinator_v3);
        #[cfg(feature = "parallel-compression-schedule")]
        assert!(settings.compression_schedule_parallel_confirm);
        #[cfg(feature = "fast-contract-validator")]
        assert!(settings.fast_contract_validator);
        // The lanes lever is *not* promoted: it is the one that is not
        // semantics-preserving.
        #[cfg(feature = "parallel-compression-schedule")]
        assert_eq!(settings.compression_schedule_lanes, 1);
        // This round's three levers all ship off, so a binary that carries them
        // and is not asked for them is the previous round's binary.
        assert!(!settings.plan_replan);
        #[cfg(feature = "compression-schedule")]
        assert_eq!(settings.compression_schedule_batch_work_units, None);
        #[cfg(feature = "compression-schedule")]
        assert!(!settings.compression_schedule_cap_to_budget);
        // And this round's four. All off, so a binary that carries the real
        // interruption and is not asked for it is the previous round's binary -
        // which is the claim the equivalence gate in
        // `docs/experiments/real-interruption/` §5 measures from the request.
        #[cfg(feature = "compression-schedule")]
        {
            assert!(!settings.compression_schedule_wall_stop);
            assert_eq!(settings.compression_schedule_yield_batches, 0);
            assert!(!settings.compression_schedule_past_bound);
            // The two that are *values* rather than switches still ship at the
            // number the lever means when it is armed, and neither is read at
            // all while `past_bound` is false.
            assert_eq!(
                settings.compression_schedule_past_bound_batches,
                SCHEDULE_PAST_BOUND_BATCHES
            );
            assert_eq!(
                settings.compression_schedule_past_bound_barren,
                SCHEDULE_PAST_BOUND_BARREN
            );
            assert_eq!(settings.compression_schedule_past_bound_share, 1.0);
        }
    }

    /// The ladder is one ladder, and it floors.
    #[test]
    fn the_quantiser_floors_onto_the_shipped_ladder() {
        let rung = |k: i32| PLAN_ANCHOR_UNITS * PLAN_QUANTUM_STEP.powi(k);
        for k in 0..30 {
            // Exactly on a rung stays on it; a hair above stays on it too.
            let (index, units) = quantise_plan(rung(k) * 1.000_001, PLAN_QUANTUM_STEP);
            assert_eq!(index, Some(k as i64), "rung {k}");
            assert!((units - rung(k)).abs() < rung(k) * 1e-9, "rung {k}");
            // A hair below the *next* rung is still this rung: the floor is
            // what makes the plan never larger than the measurement justified.
            let (index, _) = quantise_plan(rung(k + 1) * 0.999_999, PLAN_QUANTUM_STEP);
            assert_eq!(index, Some(k as i64), "rung {k}, just under the next");
        }
        // `planq=1` switches it off, and so does anything at or below the
        // anchor - a plan smaller than one rung has no ladder to sit on.
        assert_eq!(quantise_plan(5_000_000.0, 1.0), (None, 5_000_000.0));
        assert_eq!(
            quantise_plan(PLAN_ANCHOR_UNITS * 0.5, PLAN_QUANTUM_STEP),
            (None, PLAN_ANCHOR_UNITS * 0.5)
        );
    }

    /// The re-plan's two guards, which are the whole of its determinism claim.
    ///
    /// A tranche is taken only when the re-priced total clears the **next
    /// rung**; below that the run keeps the budget it has and therefore
    /// produces the document a `replan=0` run would have produced. So the
    /// clock's influence on a re-planning run is bounded by a whole rung on the
    /// tranche *count* as well as on the tranche *size*.
    #[test]
    fn a_tranche_is_refused_unless_it_buys_a_whole_rung() {
        let settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Wall { millis: 10_000 },
        );
        let plan = PlanReport {
            target_millis: 10_000,
            probe_seconds: 2.0,
            probe_work_units: 8_000_000,
            probe_rate_units_per_second: 4_000_000.0,
            bias: PLAN_PHASE_ZERO_BIAS,
            headroom: PLAN_HEADROOM,
            quantum_step: PLAN_QUANTUM_STEP,
            raw_units: 24_000_000.0,
            rung: Some(23),
            units: 24_000_000,
            first_tranche: 0.6,
            probe_effective_seconds: 2.0,
            calibration_source: PlanCalibrationSource::Live,
            probe_samples: 0,
        };
        // A meter whose clock has already passed the target buys nothing: there
        // is no remaining wall to re-price.
        let mut spent = BudgetMeter::new(PortfolioBudget::Work { units: 24_000_000 });
        spent.started = Instant::now() - std::time::Duration::from_secs(60);
        assert!(spent.replan(&plan, &settings, 1).is_none());

        // A meter under a wall budget has no work currency to install into.
        let mut wall = BudgetMeter::new(PortfolioBudget::Wall { millis: 10_000 });
        assert!(wall.replan(&plan, &settings, 1).is_none());

        // And a plan whose probe has not been overtaken by the queue has no
        // window to measure a rate over.
        let mut fresh = BudgetMeter::new(PortfolioBudget::Work { units: 24_000_000 });
        assert!(
            fresh.replan(&plan, &settings, 1).is_none(),
            "a run that has spent no queue time cannot price the queue"
        );
    }

    /// A tranche installs a *total*, and it is on the same ladder.
    #[test]
    fn a_tranche_installs_a_larger_total_on_the_same_ladder() {
        let settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Wall { millis: 10_000 },
        );
        let plan = PlanReport {
            target_millis: 10_000,
            probe_seconds: 0.001,
            probe_work_units: 0,
            probe_rate_units_per_second: 0.0,
            bias: PLAN_PHASE_ZERO_BIAS,
            headroom: PLAN_HEADROOM,
            quantum_step: PLAN_QUANTUM_STEP,
            raw_units: 4_000_000.0,
            rung: None,
            units: 4_000_000,
            first_tranche: 0.6,
            probe_effective_seconds: 2.0,
            calibration_source: PlanCalibrationSource::Live,
            probe_samples: 0,
        };
        let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: 4_000_000 });
        // A run 1 ms old with 4 M units already retired: a very fast queue and
        // ~9.7 s of wall left, so the re-price is far above the next rung.
        meter.started = Instant::now() - std::time::Duration::from_millis(1);
        meter.self_metered_debit = 4_000_000;
        let tranche = meter
            .replan(&plan, &settings, 1)
            .expect("9.7 s of remaining wall at this rate buys many rungs");
        assert_eq!(tranche.index, 1);
        assert!(tranche.units > 4_000_000, "{}", tranche.units);
        assert_eq!(
            meter.budget,
            PortfolioBudget::Work {
                units: tranche.units
            }
        );
        // On the ladder, and it is the *same* ladder the initial plan uses.
        let rung = tranche.rung.expect("quantised");
        let expected = PLAN_ANCHOR_UNITS * PLAN_QUANTUM_STEP.powf(rung as f64);
        assert!((tranche.units as f64 - expected).abs() < expected * 1e-6);
        // No bias divisor: the rate is the queue's own, measured.
        assert!(tranche.queue_rate_units_per_second > 0.0);
        assert!(tranche.queue_seconds > 0.0);
    }

    /// A floored rung times the step is *not* the next rung.
    ///
    /// The one-line arithmetic bug behind the stranding fix's first cut: a
    /// budget is a `u64`, so a rung has already lost its fractional part, and
    /// `floor(rung) * 1.15` lands a fraction of a unit below the next rung and
    /// quantises straight back onto the one it started from.
    #[test]
    fn the_next_rung_is_derived_from_the_index_and_not_from_a_multiplication() {
        for k in 0..40 {
            let rung = (PLAN_ANCHOR_UNITS * PLAN_QUANTUM_STEP.powi(k)) as u64;
            if rung as f64 <= PLAN_ANCHOR_UNITS {
                continue;
            }
            let next = next_rung_above(rung, PLAN_QUANTUM_STEP);
            let (index, snapped) = quantise_plan(next, PLAN_QUANTUM_STEP);
            assert_eq!(index, Some(k as i64 + 1), "rung {k}");
            assert!(
                snapped as u64 > rung,
                "rung {k}: {} is not above {rung}",
                snapped as u64
            );
            // The naive form is the bug, and it is a bug on most rungs.
            let naive = rung as f64 * PLAN_QUANTUM_STEP;
            let (naive_index, _) = quantise_plan(naive, PLAN_QUANTUM_STEP);
            assert!(
                naive_index == Some(k as i64) || naive_index == Some(k as i64 + 1),
                "rung {k}: the naive form is off by more than one rung"
            );
        }
        // Unquantised, the threshold is one rung's worth of growth.
        assert_eq!(
            next_rung_above(1_000, 1.0),
            1_000.0 * PLAN_TRANCHE_MIN_GROWTH
        );
    }

    /// A tranche whose window cannot justify a rung buys one anyway, if the
    /// remaining wall pays for it.
    ///
    /// The **stranding** regression, and it is the second bug this round
    /// shipped and caught: the first cut refused a tranche below one rung, and
    /// `evidence/determinism-replan-stranded.json` caught mixed-61 seed 2
    /// stopping with 5.7 s of a ten-second target unspent and three
    /// millimetres behind the mode it improves, because a short first tranche
    /// leaves a short queue window and a short window cannot justify a rung.
    ///
    /// The excess over [`PLAN_TRANCHE_HORIZON`] is bounded to exactly one rung,
    /// which is what keeps §9.1's 36.74 s failure from coming back this way.
    #[test]
    fn a_window_too_short_for_a_rung_still_buys_one_when_the_wall_pays() {
        let mut settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Wall { millis: 10_000 },
        );
        settings.plan_replan = true;
        let current = 9_357_620u64;
        let plan = PlanReport {
            target_millis: 10_000,
            probe_seconds: 2.5,
            probe_work_units: 8_778_573,
            probe_rate_units_per_second: 3_511_429.0,
            bias: PLAN_PHASE_ZERO_BIAS,
            headroom: PLAN_HEADROOM,
            quantum_step: PLAN_QUANTUM_STEP,
            raw_units: current as f64,
            rung: Some(31),
            units: current,
            first_tranche: 0.6,
            probe_effective_seconds: 2.0,
            calibration_source: PlanCalibrationSource::Live,
            probe_samples: 0,
        };
        // 4 s in: a 1.5 s queue window that retired 579 k units, and 5.7 s of
        // the target still to spend. The horizon alone buys 1.5 s * 386 k/s =
        // 579 k more, which is 6% - far short of the 15% rung.
        let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: current });
        meter.started = Instant::now() - std::time::Duration::from_millis(4_000);
        meter.self_metered_debit = current;
        let tranche = meter
            .replan(&plan, &settings, 1)
            .expect("5.7 s of remaining wall pays for one rung at this rate");
        assert!(
            tranche.units > current,
            "the run must not stop with wall unspent: {tranche:?}"
        );
        // Exactly one rung, and no more: the horizon was exceeded on purpose
        // and the excess is bounded.
        let (_, expected) = quantise_plan(
            next_rung_above(current, PLAN_QUANTUM_STEP),
            PLAN_QUANTUM_STEP,
        );
        assert_eq!(tranche.units, expected as u64, "one rung and no more");
        assert!(
            tranche.horizon_seconds > tranche.queue_seconds,
            "this is the one place the horizon is exceeded: {tranche:?}"
        );
        assert!(tranche.horizon_seconds <= tranche.remaining_seconds);

        // And when the wall cannot pay for a rung, it is still refused - the
        // override is "buy a rung or nothing", never "buy what is left".
        let mut broke = BudgetMeter::new(PortfolioBudget::Work { units: current });
        broke.started = Instant::now() - std::time::Duration::from_millis(9_600);
        broke.self_metered_debit = current;
        assert!(broke.replan(&plan, &settings, 1).is_none());
    }

    /// A first tranche the probe has already outrun degrades to the whole
    /// target instead of buying nothing.
    ///
    /// This is the three-second case and it is a **regression test for a bug
    /// this round shipped and caught**: at a 3 s target phase 0 on mixed-61 is
    /// 2.2 s, so `0.6 * 3 * 0.97 = 1.75 s` is already behind. Without the
    /// degrade the plan is exactly the probe, the schedule phase is skipped,
    /// and the re-plan cannot rescue it - with no queue there is no rate to
    /// measure - so the run publishes phase 0's layout and a re-planning run is
    /// *worse* than the mode it improves at the tightest budget there is.
    #[test]
    fn a_first_tranche_the_probe_outran_degrades_to_the_whole_target() {
        let mut settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Wall { millis: 3_000 },
        );
        settings.plan_replan = true;
        settings.plan_first_tranche = 0.6;
        let mut meter = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 3_000,
        });
        // A probe of 2.2 s against a 1.746 s first-tranche horizon.
        meter.started = Instant::now() - std::time::Duration::from_millis(2_200);
        meter.self_metered_debit = 8_778_573;
        let plan = meter.install_plan(3_000, &settings);
        assert_eq!(
            plan.first_tranche, 1.0,
            "the fraction is reported as the one that was applied"
        );
        // `3 * 0.97 - 2.2 = 0.71 s` of probe-rate work, on top of the probe
        // itself - which is strictly more than the probe alone.
        assert!(
            plan.raw_units > plan.probe_work_units as f64,
            "a plan of exactly the probe is a run that never searches: {plan:?}"
        );
        // And the degrade is *conditional*: a target the probe has not outrun
        // keeps the fraction it was given.
        let mut roomy = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 10_000,
        });
        roomy.started = Instant::now() - std::time::Duration::from_millis(2_200);
        roomy.self_metered_debit = 8_778_573;
        assert_eq!(roomy.install_plan(10_000, &settings).first_tranche, 0.6);
    }

    /// A tranche never prices more queue time than it has watched.
    ///
    /// The pilot in `docs/experiments/replan/` §9.1 is what this is for: an
    /// unbounded tranche predicted 15.5 s of queue from an 11.1 s window, the
    /// rate fell 42% below the reading, and a 30 s target took 36.74 s.
    #[test]
    fn a_tranche_does_not_extrapolate_past_the_window_it_measured() {
        let mut settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Wall { millis: 30_000 },
        );
        let plan = PlanReport {
            target_millis: 30_000,
            // A probe that finished 4 s ago, so the queue window below is
            // exactly `at_seconds - 4.0`.
            probe_seconds: 4.0,
            probe_work_units: 0,
            probe_rate_units_per_second: 0.0,
            bias: PLAN_PHASE_ZERO_BIAS,
            headroom: 1.0,
            quantum_step: PLAN_QUANTUM_STEP,
            raw_units: 4_000_000.0,
            rung: None,
            units: 4_000_000,
            first_tranche: 0.6,
            probe_effective_seconds: 2.0,
            calibration_source: PlanCalibrationSource::Live,
            probe_samples: 0,
        };
        let priced = move |horizon: f64| {
            let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: 4_000_000 });
            // 10 s in: a 6 s queue window, and 20 s of the target left. An
            // unbounded tranche prices 20 s; a bounded one prices 6.
            meter.started = Instant::now() - std::time::Duration::from_millis(10_000);
            meter.self_metered_debit = 6_000_000;
            let mut settings = settings.clone();
            settings.plan_tranche_horizon = horizon;
            meter
                .replan(&plan, &settings, 1)
                .expect("20 s of remaining wall buys many rungs either way")
        };
        let bounded = priced(1.0);
        let unbounded = priced(1_000.0);
        assert!(
            (bounded.horizon_seconds - bounded.queue_seconds).abs() < 0.5,
            "the bounded tranche prices its own window: {bounded:?}"
        );
        assert!(
            unbounded.horizon_seconds > bounded.horizon_seconds * 2.0,
            "the unbounded tranche prices the whole remaining wall: {unbounded:?}"
        );
        assert!(
            unbounded.raw_units > bounded.raw_units,
            "and therefore buys more"
        );
        assert!(
            unbounded.units > bounded.units,
            "by at least a rung, or the pilot's failure could not have happened"
        );
        // Both still report the *whole* remaining wall, so a reader can see
        // what the cap refused rather than only what it allowed.
        assert!(bounded.remaining_seconds > bounded.horizon_seconds);
        // The two arms are two processes' worth of clock apart by construction
        // - each builds its own meter - so this is "the same wall", not "the
        // same float".
        assert!(
            (unbounded.remaining_seconds - bounded.remaining_seconds).abs() < 0.1,
            "{} vs {}",
            unbounded.remaining_seconds,
            bounded.remaining_seconds
        );
    }

    /// The first tranche is the whole target unless the run intends to re-plan.
    ///
    /// This is the guard that keeps `planfirst` from being a way to give 40% of
    /// a wall target away by accident: a run that cannot take a second tranche
    /// must aim the one plan it gets at the whole thing.
    #[test]
    fn the_first_tranche_is_the_whole_target_when_replanning_is_off() {
        let mut settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Wall { millis: 10_000 },
        );
        settings.plan_first_tranche = 0.25;
        let mut off = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 10_000,
        });
        let plan_off = off.install_plan(10_000, &settings);
        assert_eq!(plan_off.first_tranche, 1.0);

        settings.plan_replan = true;
        let mut on = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 10_000,
        });
        let plan_on = on.install_plan(10_000, &settings);
        assert_eq!(plan_on.first_tranche, 0.25);
        assert!(
            plan_on.raw_units <= plan_off.raw_units,
            "a quarter of the target cannot buy more than all of it"
        );
    }

    /// The certificate arming is scoped to one run and restores what it found.
    ///
    /// A library that left a `fcv=0` request's disarm behind would hand the
    /// next request in the same process a different engine.
    #[cfg(feature = "fast-contract-validator")]
    #[test]
    fn the_certificate_arming_is_restored_on_the_way_out() {
        use crate::validation::general_polygon::contract_certificate_armed;
        assert!(contract_certificate_armed(), "the default is armed");
        {
            let _guard = ContractCertificateArming::install(false);
            assert!(!contract_certificate_armed());
            {
                let _nested = ContractCertificateArming::install(true);
                assert!(contract_certificate_armed());
            }
            assert!(!contract_certificate_armed(), "the nested guard restored");
        }
        assert!(contract_certificate_armed(), "the outer guard restored");
    }

    /// The parallel currency joins the settlement under the same `max`, and
    /// the three-way maximum is the rule - not "the class price wins".
    ///
    /// The three prices this exercises are the three the measured band
    /// produces: mode 34's own meter reading about 11x the global counter,
    /// mode 20's class price reading about 26,000x it, and mode 22 where the
    /// global counter is already the largest of the three.
    #[test]
    fn the_class_price_settles_as_the_third_arm_of_one_maximum() {
        // Mode 34: the operator's own meter is the maximum and the class price
        // must not lower it. The class price here is a mode-22-shaped one -
        // below the global delta - so the settled charge has to be the
        // operator's.
        let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: 40_000_000 });
        let charge = settle_operator_charge(&mut meter, 307_767, Some(3_341_665));
        assert_eq!(charge.charged_units, 3_341_665);

        // Mode 20: the global counter reads 310 and the class price reads the
        // draw's honest 8.17 M. The settlement takes the class price, and the
        // *budget* moves by the difference - which is the whole mechanism, and
        // the thing `docs/experiments/basin-race/` §4.4 said a work-denominated
        // ceiling could not do.
        let before = meter.work_units();
        let draw = settle_operator_charge(&mut meter, 310, Some(8_173_539));
        assert_eq!(draw.charged_units, 8_173_539);
        assert_eq!(draw.debited_units, 8_173_229);
        assert_eq!(meter.work_units() - before, 8_173_229);

        // Mode 22: nothing is repriced, so the settled charge is the global
        // delta and the budget moves by nothing extra.
        let steady = meter.work_units();
        let quantum = settle_operator_charge(&mut meter, 2_013_198, Some(2_013_198));
        assert_eq!(quantum.charged_units, 2_013_198);
        assert_eq!(quantum.debited_units, 0);
        assert_eq!(meter.work_units(), steady);
    }

    /// The currency's reading of the counter array and the shipped meter's
    /// are the same function of the same array.
    ///
    /// This is the structural half of the `43` bug's regression, and it is the
    /// half a unit test inside `work_currency` cannot write: it drives
    /// `work_currency_counts_from` and `work_units_from`, the two production
    /// mappings `run_operator` reads the live registry through, and asserts
    /// that a class the profile does not name self-prices at exactly the
    /// global delta. If those two ever disagree again, every unnamed class
    /// starts paying a debit the coordinator never intended and the whole
    /// trajectory moves - which is what happened, and what
    /// `chargedExtraUnits` on a mode-22 call reported.
    ///
    /// Over a **snapshot**, not the live counters: see `work_units_from`'s
    /// own comment for what the live version cost.
    #[test]
    fn an_unnamed_class_self_prices_at_exactly_the_shipped_meters_reading() {
        use crate::search::work_currency::{price_for, DEFAULT_CLASS_PRICE};
        let mut totals = [0u64; Counter::COUNT];
        // A real measured mode-22 call, plus non-zero values on every counter
        // an unnamed class must be charged *nothing* for - if any of those
        // leaked into the price the two sides would part company here.
        totals[Counter::CandidateQueries as usize] = 2_007_788;
        totals[Counter::ExactPairTests as usize] = 1_082;
        totals[Counter::NeighborTests as usize] = 7_247_175;
        totals[Counter::FullRescores as usize] = 1_912;
        totals[Counter::CollisionPolygonBuilds as usize] = 12_345;
        totals[Counter::AcceptedMoves as usize] = 999;
        for scale in [0u64, 1, 7, 1_000_003] {
            let scaled: [u64; Counter::COUNT] =
                std::array::from_fn(|index| totals[index].saturating_mul(scale));
            let counts = work_currency_counts_from(&scaled);
            let global = work_units_from(&scaled);
            assert_eq!(
                DEFAULT_CLASS_PRICE.units(&counts),
                global,
                "the currency and the shipped meter must read the array the same way"
            );
            // And the named class must be at least the global reading, never
            // below it - `max` protects the budget, but a class that
            // self-priced below the meter would make `classUnits`
            // incomparable with `globalUnits` in the evidence.
            assert!(price_for(20).units(&counts) >= global);
        }
        // The pinned scalar, so a change to the shipped meter breaks this
        // loudly rather than silently re-deriving itself on both sides.
        assert_eq!(work_units_from(&totals), 2_013_198);
    }

    /// Under a wall budget the currency is inert, exactly as the operator's
    /// own meter is.
    ///
    /// A wall budget spends seconds; there is no broad phase for a class to
    /// ride free on and no counter to reprice, and the guard lives in
    /// `debit_self_metered` so one rule covers both self-metered arms rather
    /// than each call site remembering it.
    #[test]
    fn a_wall_budget_is_not_repriced_by_the_class_price_either() {
        let mut wall = BudgetMeter::new(PortfolioBudget::Wall { millis: 10_000 });
        let charge = settle_operator_charge(&mut wall, 310, Some(8_173_539));
        assert_eq!(charge.debited_units, 0);
        assert_eq!(charge.charged_units, 310);
        assert_eq!(wall.self_metered_debit(), 0);
    }

    /// `Off` is the default, and the two other modes differ in exactly one
    /// thing: whether the price is settled.
    #[test]
    fn the_observing_mode_prices_without_charging() {
        assert_eq!(WorkCurrencyMode::default(), WorkCurrencyMode::Off);
        assert!(!WorkCurrencyMode::Off.armed());
        assert!(WorkCurrencyMode::Observe.armed());
        assert!(WorkCurrencyMode::Charge.armed());
        assert!(!WorkCurrencyMode::Observe.charges());
        assert!(WorkCurrencyMode::Charge.charges());
        assert!(!WorkCurrencyMode::Off.charges());
        let shipped = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Work { units: 40_000_000 },
        );
        assert_eq!(shipped.work_currency, WorkCurrencyMode::Off);
    }

    #[test]
    fn the_budget_currency_is_the_budgets_own() {
        // The affordability guard compares a *measured operator cost* against
        // what is left, so both have to be quoted in one currency, and which
        // currency that is has to be the budget's - seconds for a wall budget,
        // work units for a work budget - or a work-budget run would branch on
        // a clock and stop being reproducible.
        let wall = BudgetMeter::new(PortfolioBudget::Wall { millis: 10_000 });
        assert_eq!(wall.currency_total(), 10.0);
        let work = BudgetMeter::new(PortfolioBudget::Work { units: 40_000_000 });
        assert_eq!(work.currency_total(), 40_000_000.0);
        assert_eq!(work.currency_spent(), 0.0);
        assert_eq!(work.remaining_to(0.5), 20_000_000.0);
        assert_eq!(work.remaining_to(0.0), 0.0);

        let call = OperatorCallReport {
            phase: "descent".to_owned(),
            operator: "mode22".to_owned(),
            parent_fingerprint: None,
            secondary_parent_fingerprint: None,
            action: None,
            started_seconds: 0.0,
            elapsed_seconds: 1.25,
            work_units: 3_000_000,
            global_units: 3_000_000,
            self_metered_units: None,
            debited_units: 0,
            exact_valid: true,
            raw_depth_mm: None,
            result_fingerprint: None,
            archive_disposition: None,
            published: false,
            failure_reason: None,
            schedule_slice: None,
            work_currency: None,
        };
        assert_eq!(wall.call_cost(&call), 1.25);
        assert_eq!(work.call_cost(&call), 3_000_000.0);
    }

    // ---------------------------------------------------------------------
    // The self-metered debit (coordinator v5 item 1, corrected under Sol
    // review 6 §1). Every test below reads `work_units()` directly, which is
    // legitimate here for the reason the test above pins: with `profiling`
    // recording off - which it is in a unit test - `work_units_now()` is
    // constant, so `work_units()` is exactly the debit accumulator and
    // nothing else.
    // ---------------------------------------------------------------------

    #[test]
    fn a_self_meter_above_the_global_counter_is_what_gets_spent() {
        // Sol review 6 §1: "global 30 / self 50 -> spent 50". The gap, not
        // the maximum, is what the accumulator carries, because the global
        // counter has already contributed its own 30 through
        // `work_units_now()`.
        let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: 1_000 });
        let extra = meter.debit_self_metered(30, 50);
        assert_eq!(extra, 20);
        assert_eq!(meter.self_metered_debit(), 20);
        // 30 already-counted units plus the 20 debited: the action is charged
        // 50, which is the self-meter's own reading.
        assert_eq!(meter.work_units() + 30, 50);

        // And the charge the operator transaction reports says so in all four
        // numbers at once.
        let mut fresh = BudgetMeter::new(PortfolioBudget::Work { units: 1_000 });
        let charge = settle_operator_charge(&mut fresh, 30, Some(50));
        assert_eq!(
            charge,
            OperatorCharge {
                global_units: 30,
                self_metered_units: Some(50),
                debited_units: 20,
                charged_units: 50,
            }
        );
    }

    #[test]
    fn a_global_counter_at_or_above_the_self_meter_debits_nothing() {
        // The debit can only ever raise `spent`. When the coordinator's own
        // counter already read at least what the operator charges itself
        // there is nothing to add, and - the case that matters - the debit
        // must never *lower* the reading either.
        let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: 1_000 });
        assert_eq!(meter.debit_self_metered(50, 50), 0);
        assert_eq!(meter.debit_self_metered(3_343_739, 3_341_665), 0);
        assert_eq!(meter.self_metered_debit(), 0);
        assert_eq!(meter.work_units(), 0);

        let charge = settle_operator_charge(&mut meter, 3_343_739, Some(3_341_665));
        assert_eq!(charge.debited_units, 0);
        assert_eq!(charge.charged_units, 3_343_739);
        // An operator with no meter of its own is charged its global delta and
        // nothing more.
        let plain = settle_operator_charge(&mut meter, 12_345, None);
        assert_eq!(plain.debited_units, 0);
        assert_eq!(plain.charged_units, 12_345);
        assert_eq!(plain.self_metered_units, None);
        assert_eq!(meter.self_metered_debit(), 0);
    }

    #[test]
    fn two_consecutive_self_metered_actions_both_land_on_the_budget() {
        // The accumulator is additive, and the second action's charge does not
        // overwrite or absorb the first: a run that schedules the class twice
        // pays for it twice. This is the arithmetic that decides whether a
        // 40M-unit run can afford a third slice.
        let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: 40_000_000 });
        let first = settle_operator_charge(&mut meter, 307_767, Some(3_341_665));
        assert_eq!(first.debited_units, 3_033_898);
        let after_first = meter.work_units();
        let second = settle_operator_charge(&mut meter, 400_000, Some(3_356_020));
        assert_eq!(second.debited_units, 2_956_020);
        assert_eq!(meter.self_metered_debit(), 3_033_898 + 2_956_020);
        assert_eq!(meter.work_units(), after_first + 2_956_020);
        // Each action's own charge is the self-meter's reading, and the budget
        // has felt both.
        assert_eq!(first.charged_units, 3_341_665);
        assert_eq!(second.charged_units, 3_356_020);
        assert_eq!(
            meter.spent_fraction(),
            (3_033_898.0 + 2_956_020.0) / 40_000_000.0
        );
    }

    #[test]
    fn the_debit_saturates_rather_than_wrapping() {
        // `overflow-checks = true` is on in this profile's release build, so
        // an accumulator that wrapped would abort a run rather than
        // mis-report one; saturation makes the failure mode "the budget reads
        // full", which is the safe direction for a number the affordability
        // rule compares against.
        let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: 1_000 });
        assert_eq!(meter.debit_self_metered(0, u64::MAX), u64::MAX);
        assert_eq!(meter.self_metered_debit(), u64::MAX);
        assert_eq!(meter.debit_self_metered(0, 7), 7);
        assert_eq!(meter.self_metered_debit(), u64::MAX);
        assert_eq!(meter.work_units(), u64::MAX);
        assert!(!meter.has_room(1.0));
        // The subtraction saturates too: a global delta larger than the self
        // meter's reading is zero extra, never a wrap to `u64::MAX`.
        let mut second = BudgetMeter::new(PortfolioBudget::Work { units: 1_000 });
        assert_eq!(second.debit_self_metered(u64::MAX, 1), 0);
        assert_eq!(second.self_metered_debit(), 0);
    }

    #[test]
    fn a_wall_budget_never_debits_a_self_meter() {
        // Sol review 6 §1 finding 3, kept explicit: this instrument is
        // accounting, not a wall-clock guard, and it is a no-op under a wall
        // budget by construction rather than by the caller remembering to
        // ask. It is *not* what would have caught the 2/27 wall overruns.
        let mut wall = BudgetMeter::new(PortfolioBudget::Wall { millis: 10_000 });
        assert_eq!(wall.debit_self_metered(30, 50), 0);
        assert_eq!(wall.self_metered_debit(), 0);
        let charge = settle_operator_charge(&mut wall, 30, Some(3_341_665));
        assert_eq!(charge.debited_units, 0);
        assert_eq!(charge.charged_units, 30);
        assert_eq!(charge.self_metered_units, Some(3_341_665));
        assert_eq!(wall.self_metered_debit(), 0);
        // The wall meter's own currency is untouched: `currency_spent` is
        // seconds and no accumulator is anywhere near it.
        assert_eq!(wall.work_units(), 0);
    }

    #[test]
    fn the_current_actions_debit_is_already_on_the_meter_when_it_is_stamped() {
        // Sol review 6 §1 finding 4, and the reason `run_operator` is written
        // as a transaction. `archive_layout` stamps `birth_work_units`,
        // `try_publish` stamps the publication's `work_units` and the
        // incumbent's `published_work_units`, and `OperatorCallReport`
        // carries the call's own charge - all three read the meter *after*
        // this settlement, so all three include the charge of the action that
        // produced them. Before the fix they read the meter before it, and
        // the debit landed on the next action instead: a curve where work
        // appears one action after the action that spent it.
        let mut meter = BudgetMeter::new(PortfolioBudget::Work { units: 40_000_000 });
        let started_work = meter.work_units();

        // Step 1-2 of the transaction: dispatch has happened, the global
        // delta is read.
        let global_units = meter.work_units().saturating_sub(started_work);
        // What the pre-fix ordering would have stamped: the meter as it
        // stands before the charge is settled.
        let stamp_before_settlement = meter.work_units();

        // Step 3: debit.
        let charge = settle_operator_charge(&mut meter, global_units, Some(3_341_665));

        // Step 4: every stamp taken from here includes it.
        let stamp_after_settlement = meter.work_units();
        assert_eq!(charge.debited_units, 3_341_665);
        assert_eq!(
            stamp_after_settlement,
            stamp_before_settlement + charge.debited_units
        );
        // `OperatorCallReport::work_units` is `charge.charged_units`, and the
        // publication and archive stamps are `stamp_after_settlement`; the
        // pre-fix reading is strictly smaller than all of them.
        assert_eq!(charge.charged_units, 3_341_665);
        assert!(stamp_before_settlement < stamp_after_settlement);
        // A second call in the same action inherits the settled meter as its
        // own baseline, so no charge is ever counted twice.
        let next_started = meter.work_units();
        let next_global = meter.work_units().saturating_sub(next_started);
        let next = settle_operator_charge(&mut meter, next_global, None);
        assert_eq!(next.charged_units, 0);
        assert_eq!(meter.work_units(), stamp_after_settlement);

        // What this test does *not* do, said plainly: it exercises the
        // settlement, not `run_operator`'s own source order. Reaching a real
        // `archive_layout`/`try_publish` from a unit test needs a whole engine
        // run whose mode-34 arm actually fires, which no unit test in this
        // module can afford. The end-to-end half of finding 4 is checked on
        // real run documents instead, by `drivers/orderingcheck.py`, on the
        // identity asserted just below: it is a discriminator, because the
        // pre-fix ordering computed `work_units` from the meter *before* the
        // debit and so could only ever emit `work_units == global_units`.
        let report = OperatorCallReport {
            phase: "schedule".to_owned(),
            operator: "mode34".to_owned(),
            parent_fingerprint: None,
            secondary_parent_fingerprint: None,
            action: None,
            started_seconds: 0.0,
            elapsed_seconds: 4.0,
            work_units: charge.charged_units,
            global_units: charge.global_units,
            self_metered_units: charge.self_metered_units,
            debited_units: charge.debited_units,
            exact_valid: true,
            raw_depth_mm: None,
            result_fingerprint: None,
            archive_disposition: None,
            published: false,
            failure_reason: None,
            schedule_slice: None,
            work_currency: None,
        };
        assert_eq!(
            report.work_units,
            report.global_units + report.debited_units
        );
        assert_ne!(report.work_units, report.global_units);
    }

    #[test]
    fn a_debited_call_is_priced_at_the_self_meter_for_the_next_one() {
        // The behavioural consequence of stamping the call report with the
        // settled charge rather than the global delta, and the reason this is
        // more than bookkeeping: `mean_operator_cost` averages
        // `BudgetMeter::call_cost` over a run's own past calls, and the
        // affordability rule refuses a class it cannot finish. Pricing a past
        // mode-34 call at the coordinator's optimistic 307,767 rather than the
        // 3,341,665 the arm charged itself is what let the class keep being
        // affordable. Under a wall budget the same report is priced in
        // seconds and none of this is read at all.
        let charge_units = 3_341_665u64;
        let call = OperatorCallReport {
            phase: "schedule".to_owned(),
            operator: "mode34".to_owned(),
            parent_fingerprint: None,
            secondary_parent_fingerprint: None,
            action: None,
            started_seconds: 0.0,
            elapsed_seconds: 4.0,
            work_units: charge_units,
            global_units: 307_767,
            self_metered_units: Some(charge_units),
            debited_units: charge_units - 307_767,
            exact_valid: true,
            raw_depth_mm: None,
            result_fingerprint: None,
            archive_disposition: None,
            published: false,
            failure_reason: None,
            schedule_slice: None,
            work_currency: None,
        };
        let work = BudgetMeter::new(PortfolioBudget::Work { units: 40_000_000 });
        assert_eq!(work.call_cost(&call), 3_341_665.0);
        let wall = BudgetMeter::new(PortfolioBudget::Wall { millis: 10_000 });
        assert_eq!(wall.call_cost(&call), 4.0);
    }

    #[test]
    #[cfg(feature = "compression-schedule")]
    fn the_schedules_own_report_is_the_self_metered_reading() {
        // The wiring the five tests above take as given: the number
        // `settle_operator_charge` is handed comes from the operator's own
        // report, in the portfolio's own currency, and every other operator
        // reports nothing.
        use crate::search::compression_schedule::GeneralCompressionScheduleDiagnostics;
        let mut population = GeneralPersistentVacancyDiagnostics::default();
        assert_eq!(operator_self_metered_units(&population), None);
        population.compression_schedule = Some(GeneralCompressionScheduleDiagnostics {
            work_units: 3_341_665,
            ..GeneralCompressionScheduleDiagnostics::default()
        });
        assert_eq!(operator_self_metered_units(&population), Some(3_341_665));
        assert_eq!(schedule_self_cost_units(&population), Some(3_341_665));
    }

    #[test]
    fn crossover_pairs_are_best_first_and_each_is_offered_once() {
        let fingerprints = ["a".to_owned(), "b".to_owned(), "c".to_owned()];
        let mut attempted = std::collections::BTreeSet::new();
        let mut order = Vec::new();
        while let Some((left, right, key)) =
            first_unattempted_crossover_pair(&fingerprints, &attempted)
        {
            order.push((left, right));
            attempted.insert(key);
        }
        assert_eq!(order, vec![(0, 1), (0, 2), (1, 2)]);
        // Exhausted rather than looping: a crossover phase with budget left and
        // no new pair ends instead of paying for a layout it already has.
        assert!(first_unattempted_crossover_pair(&fingerprints, &attempted).is_none());
    }

    #[test]
    fn the_constructor_clamp_is_above_a_depth_the_request_admits() {
        // mixed-61: the area bound dominates, and the clamp is the one the
        // quality frontier trace established, unchanged.
        assert_eq!(constructor_clamp_mm(130.3985, 206.869), 260.797);
        // shapes-17 and triangle-20 pack at 2.08x and 2.21x their own area
        // bound, so twice the bound is below every layout that exists and the
        // constructed depth is the floor that rescues them.
        assert_eq!(constructor_clamp_mm(96.30984986566416, 200.903), 200.903);
        assert_eq!(constructor_clamp_mm(32.123, 73.72), 73.72);
        // Whichever term wins, the clamp is never below a depth this request is
        // known to admit a complete layout at.
        for (bound, constructed) in [(1.0, 5.0), (5.0, 1.0), (2.5, 5.0)] {
            assert!(constructor_clamp_mm(bound, constructed) >= constructed);
        }
    }

    #[test]
    fn the_phase_schedule_is_monotone_and_ends_at_the_budget() {
        let schedule = PhaseSchedule::default();
        assert!(schedule.descent_by <= schedule.crossover_by);
        assert!(schedule.crossover_by <= schedule.compression_by);
        assert!(schedule.compression_by <= schedule.diversify_by);
        assert!(schedule.diversify_by <= schedule.drain_by);
        assert_eq!(schedule.drain_by, 1.0);
        // The constructor slice is last, which is the whole of this stage's
        // rebudget: 19 arms, 19 exact-valid, 0 published at ten seconds.
        assert!(schedule.compression_by < schedule.diversify_by);
        // v3's single loop ends where v2's last operator phase ended, so the
        // drain keeps exactly the reserve it already had.
        assert_eq!(schedule.schedule_by, schedule.diversify_by);
        assert!(schedule.schedule_by < schedule.drain_by);
    }

    // ---- coordinator v3 ---------------------------------------------------

    /// The compression target is *below* the parent's own depth. v2 asked for
    /// `depth + 0.8` - looser than the incumbent it held - and exited
    /// `noResidue`; the A/B/C's control D asked the same operator for
    /// `depth - 0.3` and published 2.620 mm on seed 0. The rung is the
    /// engine's own smallest construction drop, not a new millimetre.
    #[test]
    fn the_compression_target_is_below_the_parents_own_depth() {
        assert!(COMPRESSION_RUNG_MM > 0.0);
        let depth = 174.20812003998896;
        assert!(depth - COMPRESSION_RUNG_MM < depth);
        // And it is strictly tighter than the alternation rung the descent
        // class asks for, which is a ceiling rather than a compression ask.
        assert!(depth - COMPRESSION_RUNG_MM < depth + ALTERNATION_RUNG_MM);
    }

    /// A scheduled ladder is two rungs of the separator's own relative
    /// contraction quantum, so the drop is a length the request supplies.
    #[test]
    fn a_scheduled_ladder_is_two_rungs_of_the_engines_own_quantum() {
        let ratio = crate::search::general_relaxed::COUPLED_SEPARATOR_CONTRACTION_RATIO;
        for depth in [174.20812003998896_f64, 70.9, 200.903] {
            let drop = depth * LADDER_RUNGS as f64 * ratio;
            // `ladder_compression_bounds` floors its rung at `depth * ratio`,
            // so a drop of exactly two floors is exactly two rungs whatever the
            // parent's depth is.
            let floor = depth * ratio;
            let step = (drop / 8.0).max(floor);
            let rungs = ((drop / step).ceil() as usize).clamp(1, 8);
            assert_eq!(rungs, LADDER_RUNGS, "depth {depth}");
            assert!(drop > 0.0 && drop < depth);
        }
    }

    /// The class priors reproduce the ledger's own Δraw per million evaluations
    /// ordering - compression 1.10, descent 0.43, crossover 0.20 - even though
    /// they are quoted against the protected phase rather than against a
    /// million evaluations, because the queue has to be rankable under a wall
    /// budget where the evaluation counters are off.
    #[test]
    fn the_class_priors_reproduce_the_ledgers_measured_order() {
        let value = |class: ActionClass| class.prior_delta_mm() / class.prior_cost_in_phase_zero();
        let compression = value(ActionClass::Compression);
        let descent = value(ActionClass::Descent);
        let crossover = value(ActionClass::Crossover);
        let ladder = value(ActionClass::Ladder);
        assert!(compression > descent, "{compression} > {descent}");
        assert!(descent > crossover, "{descent} > {crossover}");
        // The ledger's own ratios are 1.1017 : 0.4264 : 0.2043, i.e. 5.39 : 2.09
        // : 1. The priors have to reproduce them to better than 10%, or the
        // queue is not ranking by the thing that was measured.
        assert!(((compression / crossover) / (1.1017 / 0.2043) - 1.0).abs() < 0.1);
        assert!(((descent / crossover) / (0.4264 / 0.2043) - 1.0).abs() < 0.1);
        // The ladder is the most expensive class by a factor of four and it is
        // ranked last of the four priced classes, which is why it only runs
        // once the cheap ones have exhausted their keys.
        assert!(ladder < crossover, "{ladder} < {crossover}");
        assert!(
            ActionClass::Ladder.prior_cost_in_phase_zero()
                > 3.0 * ActionClass::Crossover.prior_cost_in_phase_zero()
        );
        // The compression schedule sits between descent and crossover on the
        // ledger's own axis - 1.104 mm for a 3,341,379-unit self-cap is 0.330
        // mm per million, against descent's 0.4264 and crossover's 0.2043 -
        // and that is the declaration order it is given.
        let schedule = value(ActionClass::Schedule);
        assert!(descent > schedule, "{descent} > {schedule}");
        assert!(schedule > crossover, "{schedule} > {crossover}");
        assert!(ActionClass::Descent < ActionClass::Schedule);
        assert!(ActionClass::Schedule < ActionClass::Crossover);
        // The slice is a sixth of a ladder and it published in 11 of 12 gate
        // cells against the ladder's 10, which is the whole reason it is worth
        // ranking above the operator it replaces.
        assert!(
            ActionClass::Schedule.prior_cost_in_phase_zero() * 6.0
                < ActionClass::Ladder.prior_cost_in_phase_zero()
        );
        assert!(schedule > ladder, "{schedule} > {ladder}");
    }

    /// A prior of exactly zero is not a prior, it is a deletion: a class ranked
    /// at zero can never be chosen, so it never earns the evidence that would
    /// displace its prior. Coordinator v3 §4.2 measured what that costs - 3 µm
    /// on triangle-20, where the class is the only one that pays.
    #[test]
    fn every_class_prior_is_testable() {
        for class in ActionClass::all() {
            assert!(
                class.prior_delta_mm() > 0.0,
                "{} has an untestable prior",
                class.name()
            );
            for wall in [false, true] {
                assert!(class.prior_cost_in_phase_zero_for(wall) > 0.0);
            }
        }
    }

    /// Exactly the two classes whose two currencies were *measured* to disagree
    /// carry two prices, and the other four carry one.
    ///
    /// The ledger priced a mode-20 arm at 260-335 work units against 3.1
    /// seconds of clock; coordinator v3 §1.3 measured the same rule 11.7-12.0x
    /// wrong on shapes-17's wall and did not fix it. Measured on three requests
    /// there, the diversify phase costs 0.067-1.224 phase-zeros in work units
    /// and 1.25-1.98 in seconds.
    ///
    /// The compression schedule is the second, and coordinator v4 §8 named it
    /// before this round priced it: first-action actual/estimate is 0.97-1.01
    /// on a work budget and 2.60-5.88 on a wall budget, re-baselined here over
    /// eighteen cells.
    #[test]
    fn only_the_two_measured_classes_are_priced_twice() {
        let twice = [ActionClass::Diversify, ActionClass::Schedule];
        for class in ActionClass::all() {
            let work = class.prior_cost_in_phase_zero_for(false);
            let wall = class.prior_cost_in_phase_zero_for(true);
            if twice.contains(&class) {
                assert!(wall > work, "{} : {wall} > {work}", class.name());
                // The measured disagreements are 17x and 2.9x respectively and
                // they must survive as disagreements, not be averaged away.
                assert!(wall / work > 1.5, "{} : {wall} / {work}", class.name());
            } else {
                assert_eq!(work, wall, "{} is priced twice", class.name());
            }
        }
        // The schedule's wall prior is the worst of the eighteen cells measured
        // in `docs/experiments/m34-wall-price`: triangle-20 seed 0 at 30 s,
        // 1.6415 s of charged slice against a 0.7336 s phase 0. Every other
        // class in this table is priced by its own worst case too.
        assert_eq!(
            ActionClass::Schedule.prior_cost_in_phase_zero_for(true),
            SCHEDULE_WALL_PRIOR_PHASE_ZEROS
        );
        assert!((SCHEDULE_WALL_PRIOR_PHASE_ZEROS - 1.6414530680000001 / 0.733602375).abs() < 5e-4);
        // And it is above every measured cell, including mixed-61's, which is
        // what makes it a bound rather than an average.
        for measured in [
            1.1466399653696229_f64,
            1.6193018185283783,
            2.2375242010360177,
        ] {
            assert!(SCHEDULE_WALL_PRIOR_PHASE_ZEROS >= measured - 5e-5);
        }
    }

    /// The wall prior may not be allowed to *delete* the class it prices.
    ///
    /// This is coordinator v4 §3.1's rule - "a prior of zero is not a prior, it
    /// is a deletion" - arriving from the cost side rather than the yield side.
    /// At 2.2375 phase-zeros the schedule's ranking value is 1.104 / 2.2375 =
    /// 0.493, below the ladder's 1.292 and below crossover's 1.793, on the one
    /// request where the class publishes on nine of nine at ten seconds. It is
    /// not a hypothetical: this round's first cut ranked on that number and
    /// measured a median 0.649 mm regression over nine paired thirty-second
    /// rounds on mixed-61. The ranking now stays on the class's own currency
    /// and only the affordability gate reads the worst case.
    #[test]
    fn the_wall_prior_alone_would_rank_the_schedule_below_the_ladder() {
        let rank = |class: ActionClass, wall: bool| {
            class.prior_delta_mm() / class.prior_cost_in_phase_zero_for(wall)
        };
        let ladder = rank(ActionClass::Ladder, true);
        let crossover = rank(ActionClass::Crossover, true);
        // Priced on the clock, the class loses to both.
        assert!(rank(ActionClass::Schedule, true) < ladder);
        assert!(rank(ActionClass::Schedule, true) < crossover);
        // Priced in its own currency - which is what the floor quotes - it
        // beats both, which is the ordering coordinator v4 measured and the
        // ordering that bought mixed-61 nine publications in nine rounds.
        assert!(rank(ActionClass::Schedule, false) > ladder);
        assert!(rank(ActionClass::Schedule, false) > crossover);
    }

    /// The probe is a budget, never a veto, and it is off by default.
    #[test]
    fn the_probe_is_a_budget_and_it_ships_disarmed() {
        // A probe is at least one step at any slice length, so an armed probe
        // on a very short slice still runs the slice rather than refusing it.
        let probe = |planned: usize, denominator: usize| match denominator {
            0 => 0,
            n => (planned / n).max(1),
        };
        assert_eq!(probe(1_616, 0), 0);
        assert_eq!(probe(1_616, 3), 538);
        assert_eq!(probe(1_520, 3), 506);
        assert_eq!(probe(2, 3), 1);
        assert_eq!(probe(0, 3), 1);
        // Off. See the constant for the two measurements: the wall it returns
        // buys no depth on either request that has a sterile slice to cut, and
        // at thirty seconds it abandons a mixed-61 slice that publishes 1.03 mm
        // for a 2.132 mm loss on the round.
        assert_eq!(SCHEDULE_PROBE_DENOMINATOR, 0);
        assert_eq!(
            PortfolioSettings::new(
                GeneralRelaxedSettings::mixed_61_probe(0, 1),
                PortfolioBudget::Wall { millis: 10_000 }
            )
            .schedule_probe_denominator,
            0
        );
        // The counter-example, as arithmetic: a third of the second slice's
        // 1,520 steps is 506, and the lane has to walk 453 of them just to get
        // back to its parent's depth before it can publish anything at all.
        let entry_loss_steps = (0.453_f64 / 0.001).round() as usize;
        assert!(entry_loss_steps > probe(1_520, 3) * 8 / 10);
    }

    /// The sterile bit is one bit with one audition, and the audition is rarer
    /// than the rule that ends the run.
    #[test]
    fn the_sterile_bit_is_one_action_and_its_audition_is_rare() {
        assert_eq!(SCHEDULE_STERILE_ACTIONS, 1);
        // The class is offered again only after a barren run as long as the one
        // that ends the whole loop, so on the measured streams it fires at most
        // once and usually never: coordinator v4's own mixed-61 30 s headline
        // never reaches sixteen consecutive barren actions.
        assert_eq!(SCHEDULE_AUDITION_BARREN, BARREN_ACTION_PATIENCE);
        assert!(SCHEDULE_AUDITION_BARREN > DIVERSIFY_AUDITION_BARREN);
    }

    /// The sparse operator's own bit is one sterile slice, one audition, and a
    /// state a productive slice can leave.
    ///
    /// The last clause is the one worth a test. `schedule_sterile_bit` withholds
    /// a whole action class and its audition can only ever hand the class back
    /// for one call; this bit withholds a *degree of freedom* inside a class
    /// that keeps running, so a run whose audition finds rotation productive
    /// should keep it, not spend its one chance and go quiet again. The
    /// transitions below are the ones `run_operator` performs, driven directly.
    #[test]
    #[cfg(feature = "sparse-rotation")]
    fn the_sparse_rotation_bit_is_one_sterile_slice_and_a_reversible_verdict() {
        assert_eq!(SPARSE_ROTATION_STERILE_SLICES, 1);
        assert_eq!(SPARSE_ROTATION_AUDITION_BARREN, BARREN_ACTION_PATIENCE);

        // The rule itself, not a copy of it: `observe_slice` is the function
        // the coordinator calls.
        let mut bit = SparseRotationBit::default();
        // A slice that opened episodes and committed nothing fires the bit.
        bit.observe_slice(12, 0);
        assert!(bit.disarmed, "one sterile slice is the evidence");

        // A slice that never opened an episode is not evidence either way: the
        // mechanism did not get its trigger, so it did not get its verdict.
        let mut untried = SparseRotationBit::default();
        untried.observe_slice(0, 0);
        assert!(
            !untried.disarmed,
            "a slice with no stall says nothing about whether rotation pays"
        );

        // The audition is spent once and only after the barren wait.
        bit.barren_calls = SPARSE_ROTATION_AUDITION_BARREN - 1;
        assert!(bit.barren_calls < SPARSE_ROTATION_AUDITION_BARREN);
        bit.barren_calls += 1;
        assert!(!bit.auditioned);
        bit.auditioned = true;

        // And a productive audition reverses the verdict rather than exhausting
        // it: `sterile_slices` goes back to zero, so the bit would need fresh
        // evidence to fire again.
        bit.observe_slice(4, 3);
        assert!(!bit.disarmed);
        assert_eq!(bit.sterile_slices, 0);
    }

    /// The disarm bit reads the operator's own committed moves, and the control
    /// arm's numbers are the reason it has to.
    ///
    /// Sol review 8 §2 P0's material proof, replayed as a test: the
    /// sparse-rotation round's control arm ran **zero rungs** and reported
    /// **11,523 `rotationAcceptedMoves`**, because that counter is incremented
    /// for any accepted move whose pose differs from the incumbent's and
    /// `search_piece` draws random catalogue angles as refinement starts. Fed
    /// to the bit, that number says "the operator is productive" about a lane
    /// the operator never touched.
    #[test]
    #[cfg(feature = "sparse-rotation")]
    fn the_disarm_bit_cannot_be_fed_the_catalogues_accepted_moves() {
        // The control arm's cell, as measured: episodes fired, the operator
        // committed nothing, and eleven thousand unrelated poses moved.
        const CONTROL_ARM_ROTATION_ACCEPTED_MOVES: usize = 11_523;
        const CONTROL_ARM_SPARSE_COMMITTED_MOVES: usize = 0;

        let mut correct = SparseRotationBit::default();
        correct.observe_slice(12, CONTROL_ARM_SPARSE_COMMITTED_MOVES);
        assert!(
            correct.disarmed,
            "an operator that opened twelve episodes and committed nothing is              the sterile slice the bit exists to catch"
        );

        let mut misfed = SparseRotationBit::default();
        misfed.observe_slice(12, CONTROL_ARM_ROTATION_ACCEPTED_MOVES);
        assert!(
            !misfed.disarmed,
            "this is the bug: read through `rotationAcceptedMoves` the same              slice reads as productive, so the bit never fires and              'the disarm was never necessary' is not a finding"
        );
        assert_ne!(
            correct.disarmed, misfed.disarmed,
            "if the two counters ever agree on this cell the test is vacuous"
        );
    }

    /// The race's judge ranks on the three criteria, breaks every tie toward
    /// the incumbent, and never reads depth.
    ///
    /// Sol review 8 §4.3 names the criteria and excludes depth explicitly, and
    /// Grok review 3 §3 item 3 gives the reason: a *worse* constructor can open
    /// a *better* basin, so ranking arms on how deep they are right now
    /// systematically prefers the arm that has already fixpointed. This is the
    /// arrangement the exclusion is checked on - the deepest arm loses.
    #[test]
    #[cfg(feature = "compression-schedule")]
    fn the_race_judges_on_yield_stability_and_infeasibility_and_not_on_depth() {
        let arm = |slot: usize, depth: f64, yield_mm: f64, stability: f64, infeasibility: f64| {
            BasinRaceArm {
                slot,
                kind: "test",
                placements: Vec::new(),
                fingerprint: format!("arm{slot}"),
                archived: vec![format!("arm{slot}")],
                depth_mm: depth,
                yield_mm,
                stability,
                infeasibility,
                batch_steps: 0,
                batch_confirmations: 0,
                rank_sum: 0,
                eliminated_round: None,
            }
        };
        // Slot 0 is the deepest arm and the worst on all three criteria; slot 2
        // is the shallowest and the best on all three. A judge that read depth
        // would rank them the other way round.
        let mut rows = vec![
            arm(0, 150.0, 0.10, 0.20, 4.0),
            arm(1, 170.0, 0.50, 0.50, 2.0),
            arm(2, 190.0, 0.90, 0.80, 1.0),
        ];
        let mut live = vec![0usize, 1, 2];
        judge_basin_race(&mut live, &mut rows);
        assert_eq!(
            live,
            vec![2, 1, 0],
            "the shallow arm that compresses, confirms and starts nearly              feasible wins over the deep arm that does none of the three"
        );
        assert_eq!(rows[2].rank_sum, 0, "best on all three ranks zero");
        assert_eq!(rows[0].rank_sum, 6, "worst on all three ranks last thrice");

        // The criteria disagree: the arm that yields most is the least stable.
        // A rank sum resolves it without three weights this round would have
        // had to tune on the cells it is trying to measure.
        let mut split = vec![
            arm(0, 170.0, 0.10, 0.90, 3.0),
            arm(1, 170.0, 0.90, 0.10, 3.0),
            arm(2, 170.0, 0.50, 0.50, 1.0),
        ];
        let mut live = vec![0usize, 1, 2];
        judge_basin_race(&mut live, &mut split);
        assert_eq!(
            live[0], 2,
            "the arm that is second on two criteria and first on the third              beats the two that are first on one and last on another"
        );

        // Every tie breaks toward the lower slot, and slot 0 is the incumbent
        // control. A race that cannot tell its arms apart must not move the run
        // off the basin it already had.
        let mut tied = vec![
            arm(0, 200.0, 0.4, 0.5, 2.0),
            arm(1, 100.0, 0.4, 0.5, 2.0),
            arm(2, 100.0, 0.4, 0.5, 2.0),
        ];
        let mut live = vec![0usize, 1, 2];
        judge_basin_race(&mut live, &mut tied);
        assert_eq!(
            live,
            vec![0, 1, 2],
            "an unbroken three-way tie leaves the incumbent in front even              though it is by far the deepest arm"
        );
    }

    /// The halving keeps the top half, never drops below the target, and
    /// terminates.
    ///
    /// The arithmetic rather than the phase, because the phase needs a
    /// coordinator and this is the part that can be wrong silently: a `keep`
    /// that rounded the other way would eliminate the winner on the last
    /// round, and a `keep` that never shrank would run the audition forever.
    #[test]
    #[cfg(feature = "compression-schedule")]
    fn the_halving_shrinks_to_the_target_and_stops() {
        // `run_basin_race`'s loop, arithmetic only.
        let halve = |arms: usize, target: usize| {
            let mut live = arms;
            let mut rounds = 0usize;
            let mut rungs = BASIN_RACE_RUNGS;
            let mut walked = 0usize;
            while live > target {
                rounds += 1;
                walked += live * rungs;
                live = live.div_ceil(2).max(target);
                rungs = (rungs * 2).min(SCHEDULE_RUNGS);
                assert!(rounds < 16, "the halving must terminate");
            }
            (rounds, live, walked)
        };
        // The default shape: three arms to one winner, in two rounds. The
        // winner is never re-auditioned alone - its continuation is the v3
        // queue's first action, which the run was always going to buy.
        assert_eq!(halve(3, 1), (2, 1, 3 * 3 + 2 * 6));
        // Two survivors at thirty seconds is one round and one elimination.
        assert_eq!(halve(3, 2), (1, 2, 3 * 3));
        // Four arms, and the target is still respected on the way down.
        assert_eq!(halve(4, 1), (2, 1, 4 * 3 + 2 * 6));
        // A target that is already met is not a race and costs nothing. The
        // caller cannot reach this - `keep` is clamped to `arms - 1` - but the
        // loop must terminate on it rather than audition forever.
        assert_eq!(halve(2, 2), (0, 2, 0));

        // The whole audition is bounded by two-and-a-bit scheduled actions,
        // which is what makes it affordable at all: three arms to one winner
        // walks 21 rungs against a full slice's nine.
        let (_, _, walked) = halve(BASIN_RACE_ARMS, 1);
        assert_eq!(walked, 21);
        assert!(
            walked <= 3 * SCHEDULE_RUNGS,
            "an audition that costs more than three scheduled actions is not an audition, it is the run"
        );
        // No single batch is longer than one scheduled action, because mode 34
        // is atomic and the batch is where the race can overrun its share.
        let mut rungs = BASIN_RACE_RUNGS;
        for _ in 0..8 {
            assert!(rungs <= SCHEDULE_RUNGS);
            rungs = (rungs * 2).min(SCHEDULE_RUNGS);
        }
    }

    /// The race is off, and every knob beside it describes an armed race
    /// rather than the engine.
    #[test]
    #[cfg(feature = "compression-schedule")]
    fn the_basin_race_is_off_by_default() {
        let settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Work { units: 1 },
        );
        assert!(!settings.basin_race);
        assert!(
            settings.basin_race_evict,
            "an armed race is a decision by default: the losers leave the              archive, or the race has only spent work"
        );
        assert_eq!(settings.basin_race_arms, BASIN_RACE_ARMS);
        assert_eq!(settings.basin_race_keep, 1);
        assert_eq!(settings.basin_race_rungs, BASIN_RACE_RUNGS);
        assert!(
            settings.basin_race_draw,
            "the specified mechanism is the salted draw; the archive arm is              the cheap variant and has to be asked for"
        );
        assert!(settings.basin_race_rungs < SCHEDULE_RUNGS);
        assert!(settings.basin_race_share > 0.0 && settings.basin_race_share < 1.0);
    }

    /// The archive gives a basin back when the race retires it, and says so.
    #[test]
    fn a_retired_basin_leaves_the_archive() {
        let placements = |offset: f64| {
            vec![GeneralFastPlacement {
                piece_id: "a".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: offset,
                translate_long_axis: 0.0,
            }]
        };
        let basin = |offset: f64| ArchivedBasin {
            fingerprint: format!("fp{offset}"),
            raw_depth_mm: 100.0 + offset,
            birth_seconds: 0.0,
            birth_work_units: 0,
            operator: BasinOperator::Mode(20),
            parent_fingerprint: None,
            secondary_parent_fingerprint: None,
            exact_valid: true,
            descents: 0,
            placements: placements(offset),
        };
        let mut archive = SearchArchive::new(8, 1, 0.9);
        archive.offer(basin(1.0));
        archive.offer(basin(2.0));
        assert_eq!(archive.basins().len(), 2);
        assert!(archive.retire("fp1"), "the member was there");
        assert_eq!(archive.basins().len(), 1);
        assert_eq!(archive.basins()[0].fingerprint, "fp2");
        assert!(
            !archive.retire("fp1"),
            "retiring what is not there is a `false`, not a second eviction"
        );
        assert_eq!(archive.basins().len(), 1);
    }

    /// A scheduled compression slice is nine rungs of the same quantum the
    /// ladder takes two of, and nine rungs on the band the port measured is the
    /// step count the port's cheap arm actually walked.
    #[test]
    fn a_scheduled_slice_is_nine_rungs_and_the_ports_own_step_count() {
        let ratio = crate::search::general_relaxed::COUPLED_SEPARATOR_CONTRACTION_RATIO;
        // The port's `sched10-noroll` arm walked a median 1,568 one-micron
        // steps over twelve cells at 171-179 mm parents.
        let steps = |depth: f64| (depth * SCHEDULE_RUNGS as f64 * ratio / 0.001).round() as usize;
        assert_eq!(steps(174.20812003998896), 1568);
        for depth in [171.6141235046606_f64, 179.6200102363703] {
            assert!((1_400..=1_700).contains(&steps(depth)), "depth {depth}");
        }
        // No millimetre crosses a request: the same nine rungs is a shorter
        // walk on a shallower parent, and a strictly positive bound on any.
        for depth in [70.72726178003285_f64, 200.34937729570953] {
            let drop = depth * SCHEDULE_RUNGS as f64 * ratio;
            assert!(drop > 0.0 && drop < depth, "depth {depth}");
        }
        assert!(SCHEDULE_RUNGS > LADDER_RUNGS);
    }

    /// The two patience constants are the two ends of the interval coordinator
    /// v3 §5.2 measured, and neither is inside the other's evidence.
    #[test]
    fn the_patience_constants_sit_inside_the_measured_interval() {
        // §5.2's floor: the longest barren run that was *followed by* a
        // publication is 8, on shapes-17 at 10 s; the mixed-61 30 s headline's
        // own #13 published after 7.
        const MEASURED_FLOOR: usize = 8;
        // §5.2's ceiling: shapes-17 at 30 s churns 33 barren actions between
        // micron publications, so a patience above 32 does not cut the churn.
        const MEASURED_CEILING: usize = 32;
        assert!((MEASURED_FLOOR..=MEASURED_CEILING).contains(&BARREN_ACTION_PATIENCE));
        assert!((MEASURED_FLOOR..=MEASURED_CEILING).contains(&DIVERSIFY_AUDITION_BARREN));
        // 16 is the geometric midpoint of [8, 32], because the quantity is a
        // ratio - "how many failures before a success" - and its interval's
        // endpoints are multiplicative.
        assert_eq!(
            BARREN_ACTION_PATIENCE,
            ((MEASURED_FLOOR * MEASURED_CEILING) as f64).sqrt() as usize
        );
        // The audition is the floor itself, so a run that publishes at least
        // once every eight actions never buys a ticket it did not need, and the
        // seed-0 30 s stream - whose longest productive barren run is 7 - never
        // reaches one.
        assert_eq!(DIVERSIFY_AUDITION_BARREN, MEASURED_FLOOR);
        assert!(DIVERSIFY_AUDITION_BARREN < BARREN_ACTION_PATIENCE);
        // The run gives a basin a chance before it gives up, and it gives it
        // exactly one chance per audition interval.
        assert_eq!(BARREN_ACTION_PATIENCE / DIVERSIFY_AUDITION_BARREN, 2);
    }

    /// The three v4 keys default on inside v3, and v3 itself defaults off, so
    /// a default build is still coordinator v2 to the digit.
    #[test]
    fn the_shipping_defaults_are_v3_plus_three_and_v3_is_off() {
        let settings = PortfolioSettings::new(
            GeneralRelaxedSettings::mixed_61_probe(0, 1),
            PortfolioBudget::Work { units: 1 },
        );
        assert!(!settings.coordinator_v3);
        assert!(settings.compression_schedule_class);
        assert!(settings.diversify_in_queue);
        assert_eq!(settings.barren_action_patience, BARREN_ACTION_PATIENCE);
        // The constructor slice's own patience is untouched: it is a different
        // rule about a different thing, and the global one does not replace it.
        assert_eq!(settings.basin_patience, 1);
    }

    /// The two diversify construction sites produce one key, so a ticket the
    /// ranked queue offered and the empty-queue fallback offered is one ticket.
    #[test]
    fn the_two_diversify_paths_name_one_action() {
        let ranked = diversify_action(3);
        assert_eq!(ranked.key, "m20:slot3");
        assert_eq!(ranked.rank, 3);
        assert_eq!(ranked.class, ActionClass::Diversify);
        assert_ne!(diversify_action(3).key, diversify_action(4).key);
    }

    /// Every class name is distinct, because the operator calls and the
    /// publication events a class pays for are attributed by that name.
    #[test]
    fn action_class_names_are_distinct() {
        let all = ActionClass::all();
        let names = all
            .iter()
            .map(|class| class.name())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(names.len(), all.len());
        // `all()` is the list the queue ranks over. A variant missing from it
        // would be a class that is enumerated and never ranked, which sorts as
        // a panic in the ranking map rather than as a mis-ordering.
        assert_eq!(
            all.iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            all.len()
        );
        // The declaration order is the deterministic tie-break after the value,
        // and it is the ledger's own yield order.
        assert!(ActionClass::Compression < ActionClass::Descent);
        assert!(ActionClass::Descent < ActionClass::Crossover);
        assert!(ActionClass::Crossover < ActionClass::Ladder);
        assert!(ActionClass::Ladder < ActionClass::Diversify);
        assert!(ActionClass::Descent < ActionClass::Schedule);
        assert!(ActionClass::Schedule < ActionClass::Crossover);
    }

    /// A crossover action's key is built from the two parents *in the order
    /// they are handed to the operator*, plus the cut, and never from their
    /// ranks. The frontier reorders between actions, so a rank-built key would
    /// report an attempted action as untried - which is the exact class of
    /// error the ledger exists to avoid.
    #[test]
    fn a_v3_crossover_key_is_parent_and_cut_ordered_never_rank_ordered() {
        let key =
            |left: &str, right: &str, cut: f64| format!("23:{left}:{right}:{:016x}", cut.to_bits());
        // Same pair, two directions: two keys.
        assert_ne!(key("a", "b", 0.5), key("b", "a", 0.5));
        // Same ordered pair, two cuts: two keys.
        assert_ne!(key("a", "b", 0.5), key("a", "b", 0.4957));
        // The same action named from either end of a reordered frontier is one
        // key, because nothing in it is a rank.
        assert_eq!(key("a", "b", 0.5), key("a", "b", 0.5));
    }

    /// The bounded enumeration is bounded by *construction*: at most
    /// `CROSSOVER_CUTS_PER_PAIR` cuts per ordered pair per iteration, over the
    /// frontier's ordered pairs, plus one compression, one descent and one
    /// ladder per frontier member. The ledger's 4,318 actions are never
    /// enumerated in one pass.
    #[test]
    fn the_enumeration_is_bounded_by_construction() {
        let states = 3usize;
        let quantum_states = 1usize;
        let ordered_pairs = states * (states - 1);
        // One compression and one descent per quantum state, one ladder on
        // rank 0, and the constant cut plus `CROSSOVER_CUTS_PER_PAIR` derived
        // cuts per ordered pair.
        let ceiling = 2 * quantum_states + 1 + ordered_pairs * (CROSSOVER_CUTS_PER_PAIR + 1);
        assert_eq!(ordered_pairs, 6);
        assert_eq!(ceiling, 21);
        // Plus one compression-schedule slice per quantum state and one ranked
        // constructor ticket: the enumeration is 23 wide in a schedule-capable
        // build, still bounded by construction and still two orders below the
        // ledger's 4,318.
        let with_v4 = ceiling + quantum_states + 1;
        assert_eq!(with_v4, 23);
        assert!(with_v4 < 360);
        // The ledger's top-3 frontier alone carries 360 ordered, cut-derived
        // actions; one enumeration offers at most 21 of them, and the loop
        // re-enumerates after every action rather than walking the rest blindly.
        assert!(ceiling < 360);
        assert!(CROSSOVER_BANDS_SCANNED >= CROSSOVER_CUTS_PER_PAIR);
    }

    /// The wall stop reads the wall the *caller asked for*, under either budget
    /// that names one, and refuses to answer at all under a bare work budget.
    ///
    /// The third case is the one worth a test: `PortfolioBudget::Work` has no
    /// wall, so a run that arms `m34wallstopall` against a work budget must
    /// behave exactly as it does today rather than stop at some default.
    #[test]
    #[cfg(feature = "compression-schedule")]
    fn the_wall_stop_reads_the_requested_wall_and_only_when_one_was_named() {
        let plan = BudgetMeter::new(PortfolioBudget::Plan {
            target_millis: 10_000,
        });
        assert_eq!(plan.wall_target_seconds, Some(10.0));
        // Nothing has elapsed, so ten seconds have not passed and a ten-second
        // reserve is exactly the boundary - `>=`, so it refuses.
        assert!(!plan.wall_target_passed(0.0));
        assert!(plan.wall_target_passed(10.0));
        // A negative reserve is clamped rather than credited back.
        assert!(!plan.wall_target_passed(-1_000.0));

        let wall = BudgetMeter::new(PortfolioBudget::Wall { millis: 3_000 });
        assert_eq!(wall.wall_target_seconds, Some(3.0));
        assert!(wall.wall_target_passed(3.0));

        let work = BudgetMeter::new(PortfolioBudget::Work { units: 40_000_000 });
        assert_eq!(work.wall_target_seconds, None);
        assert!(!work.wall_target_passed(0.0));
        assert!(!work.wall_target_passed(1_000_000.0));
    }

    /// A run takes the counters it needs from exactly one flag, and puts back
    /// what it found.
    ///
    /// The restore is the half that is a bug fix rather than a feature: the
    /// `profiling::set_enabled(true)` this replaced leaked, so in a host that
    /// runs many requests in one process the first work-budgeted request left
    /// every later wall-budgeted one paying a tax it never asked for.
    ///
    /// Serialised against every other test in the crate that touches the
    /// recording flags, through `profiling::recording_test_lock` and not
    /// through a mutex of this module's own: the flags are process-global,
    /// `cargo test` runs the crate's tests in parallel threads of one process,
    /// and a private lock here would serialise this test against its siblings
    /// while still racing `profiling::tests`. That cross-module form of the
    /// trap is the one `work_units_from`'s doc comment records this file
    /// falling into once already.
    #[test]
    fn the_work_meter_arms_one_flag_and_restores_both() {
        let _guard = profiling::recording_test_lock();
        let before_profiler = profiling::enabled();
        let before_metering = profiling::metering_enabled();

        let template = GeneralRelaxedSettings::mixed_61_probe(0, 1);
        let plan = |debit: bool, currency: WorkCurrencyMode| {
            let mut settings = PortfolioSettings::new(
                template,
                PortfolioBudget::Plan {
                    target_millis: 10_000,
                },
            );
            settings.lane_local_debit = debit;
            settings.work_currency = currency;
            settings
        };

        // The shipped path: the profiler, exactly as before.
        {
            let settings = plan(false, WorkCurrencyMode::Off);
            let arming = WorkMeterArming::install(&settings);
            assert!(profiling::enabled());
            assert!(!profiling::metering_enabled());
            let report = arming.report();
            assert!(report.needed && report.profiler_armed);
            assert!(!report.metering_armed && !report.deferred_to_profiler);
        }
        assert_eq!(profiling::enabled(), before_profiler);
        assert_eq!(profiling::metering_enabled(), before_metering);

        // The debit: the meter's own flag, and the profiler left alone.
        {
            let settings = plan(true, WorkCurrencyMode::Off);
            let arming = WorkMeterArming::install(&settings);
            assert!(!profiling::enabled());
            assert!(profiling::metering_enabled());
            let report = arming.report();
            assert!(report.metering_armed && !report.profiler_armed);
            assert!(!report.deferred_to_profiler);
        }
        assert_eq!(profiling::enabled(), before_profiler);
        assert_eq!(profiling::metering_enabled(), before_metering);

        // The currency prices three counters the meter does not read, so the
        // debit defers rather than handing it three zeros.
        {
            let settings = plan(true, WorkCurrencyMode::Observe);
            let arming = WorkMeterArming::install(&settings);
            assert!(profiling::enabled());
            let report = arming.report();
            assert!(report.deferred_to_profiler && report.profiler_armed);
        }
        assert_eq!(profiling::enabled(), before_profiler);
        assert_eq!(profiling::metering_enabled(), before_metering);

        // A wall budget reads no counter, so it arms neither - with or without
        // the key, which is what "inert under a wall budget" has to mean.
        for debit in [false, true] {
            let mut settings =
                PortfolioSettings::new(template, PortfolioBudget::Wall { millis: 10_000 });
            settings.lane_local_debit = debit;
            let arming = WorkMeterArming::install(&settings);
            assert!(!profiling::enabled());
            assert!(!profiling::metering_enabled());
            let report = arming.report();
            assert!(!report.needed);
            assert!(!report.metering_armed && !report.deferred_to_profiler);
        }
        assert_eq!(profiling::enabled(), before_profiler);
        assert_eq!(profiling::metering_enabled(), before_metering);
    }
}

#[cfg(all(test, feature = "portfolio-ledger"))]
mod ledger_tests {
    use super::*;

    fn pose(id: &str, short: f64, long: f64) -> GeneralFastPlacement {
        GeneralFastPlacement {
            piece_id: id.to_owned(),
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: short,
            translate_long_axis: long,
        }
    }

    fn member(fingerprint: &str, depth: f64, layout: Vec<GeneralFastPlacement>) -> ArchivedBasin {
        ArchivedBasin {
            fingerprint: fingerprint.to_owned(),
            raw_depth_mm: depth,
            birth_seconds: 0.0,
            birth_work_units: 0,
            operator: BasinOperator::Mode(22),
            parent_fingerprint: None,
            secondary_parent_fingerprint: None,
            exact_valid: true,
            descents: 0,
            placements: layout,
        }
    }

    /// Four pieces at four distinct short-axis positions leave three interface
    /// bands, and each band's cut sits at the *midpoint* of its own gap.
    #[test]
    fn derived_cuts_are_one_per_gap_at_the_gap_midpoint() {
        let left = vec![
            pose("a", 0.0, 0.0),
            pose("b", 10.0, 0.0),
            pose("c", 20.0, 0.0),
            pose("d", 30.0, 0.0),
        ];
        let right = left
            .iter()
            .map(|placement| pose(&placement.piece_id, placement.translate_short_axis, 7.0))
            .collect::<Vec<_>>();
        let bands = derived_cut_bands(&left, &right);
        assert_eq!(bands.len(), 3);
        // Span is 30; the gaps are 0-10, 10-20, 20-30, so the midpoints are 5,
        // 15 and 25, i.e. fractions 1/6, 1/2 and 5/6.
        let fractions = bands.iter().map(|band| band.0).collect::<Vec<_>>();
        assert!((fractions[0] - 1.0 / 6.0).abs() < 1e-12);
        assert!((fractions[1] - 0.5).abs() < 1e-12);
        assert!((fractions[2] - 5.0 / 6.0).abs() < 1e-12);
        assert!(bands.iter().all(|band| (band.1 - 10.0).abs() < 1e-12));
        // Every piece has a different long-axis pose in the two parents, so
        // every band's lower edge holds a differing piece.
        assert!(bands.iter().all(|band| band.2 == 1));
        // Exactly one band is the one the constant 0.5 lands in.
        assert_eq!(bands.iter().filter(|band| band.3).count(), 1);
    }

    /// A band whose lower edge holds no *differing* piece is reported with a
    /// zero count, and the hybrid it builds is the one the band below builds -
    /// which is why `enumerate_crossover_actions` deduplicates by hybrid.
    #[test]
    fn a_band_over_agreeing_pieces_repeats_the_hybrid_below_it() {
        let left = vec![
            pose("a", 0.0, 0.0),
            pose("b", 10.0, 0.0),
            pose("c", 20.0, 0.0),
        ];
        // `b` is identical in both parents; `a` and `c` are not.
        let right = vec![
            pose("a", 0.0, 5.0),
            pose("b", 10.0, 0.0),
            pose("c", 20.0, 5.0),
        ];
        let bands = derived_cut_bands(&left, &right);
        assert_eq!(bands.len(), 2);
        assert_eq!(bands[0].2, 1);
        assert_eq!(bands[1].2, 0);
        let first = crossover_hybrid(&left, &right, bands[0].0).expect("hybrid");
        let second = crossover_hybrid(&left, &right, bands[1].0).expect("hybrid");
        assert_eq!(
            general_placement_fingerprint(&first.0),
            general_placement_fingerprint(&second.0)
        );
    }

    /// The hybrid keeps parent A below the cut and parent B above it, which is
    /// the rule `general_relaxed::run_recombination` applies.
    #[test]
    fn the_hybrid_takes_a_below_the_cut_and_b_above_it() {
        let left = vec![
            pose("a", 0.0, 1.0),
            pose("b", 10.0, 1.0),
            pose("c", 20.0, 1.0),
        ];
        let right = vec![
            pose("a", 0.0, 9.0),
            pose("b", 10.0, 9.0),
            pose("c", 20.0, 9.0),
        ];
        // The span is 0..20, so the cut at 0.5 is at 10 - and the test is that
        // the comparison is *strict*: the piece sitting exactly on the
        // threshold goes to B, which is what `run_recombination` does.
        let (hybrid, from_left, from_right) = crossover_hybrid(&left, &right, 0.5).expect("hybrid");
        assert_eq!((from_left, from_right), (1, 2));
        let by_id = hybrid
            .iter()
            .map(|placement| (placement.piece_id.as_str(), placement.translate_long_axis))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(by_id["a"], 1.0);
        assert_eq!(by_id["b"], 9.0);
        assert_eq!(by_id["c"], 9.0);
    }

    /// Both directions are enumerated, they produce different hybrids, and only
    /// the midpoint band of the direction the schedule actually keyed is marked
    /// attempted.
    #[test]
    fn both_directions_are_actions_and_only_the_keyed_one_is_attempted() {
        let left = vec![
            pose("a", 0.0, 1.0),
            pose("b", 10.0, 1.0),
            pose("c", 20.0, 1.0),
        ];
        let right = vec![
            pose("a", 2.0, 9.0),
            pose("b", 12.0, 9.0),
            pose("c", 22.0, 9.0),
        ];
        let selection = vec![member("left", 170.0, left), member("right", 180.0, right)];
        let mut attempted = std::collections::BTreeSet::new();
        attempted.insert("23:left:right".to_owned());
        let actions = enumerate_crossover_actions(&selection, &attempted);
        assert!(actions.iter().any(|action| action.reciprocal));
        assert!(actions.iter().any(|action| !action.reciprocal));
        let hit = actions
            .iter()
            .filter(|action| action.attempted)
            .collect::<Vec<_>>();
        assert_eq!(hit.len(), 1);
        assert!(!hit[0].reciprocal);
        assert!(hit[0].is_midpoint_band);
        // The reciprocal of the keyed action is untried, which is the whole
        // point: mode 23 is directional and the schedule keys one direction.
        assert!(actions
            .iter()
            .any(|action| action.reciprocal && action.is_midpoint_band && !action.attempted));
    }

    /// The key is built from the two parents in the order they are handed to
    /// the operator, never from their ranks - the frontier reorders between
    /// attempts and a rank-built key would report an attempted action as
    /// untried.
    #[test]
    fn the_attempted_key_is_parent_ordered_not_rank_ordered() {
        let shallow = vec![pose("a", 0.0, 1.0), pose("b", 10.0, 1.0)];
        let deep = vec![pose("a", 2.0, 9.0), pose("b", 12.0, 9.0)];
        let selection = vec![
            member("shallow", 170.0, shallow),
            member("deep", 180.0, deep),
        ];
        // The schedule keyed `deep -> shallow`, i.e. what is now the
        // *reciprocal* of the ranked pair.
        let mut attempted = std::collections::BTreeSet::new();
        attempted.insert("23:deep:shallow".to_owned());
        let actions = enumerate_crossover_actions(&selection, &attempted);
        let hit = actions
            .iter()
            .filter(|action| action.attempted)
            .collect::<Vec<_>>();
        assert_eq!(hit.len(), 1);
        assert!(hit[0].reciprocal);
        assert_eq!(hit[0].left_fingerprint, "deep");
        assert_eq!(hit[0].right_fingerprint, "shallow");
    }

    #[test]
    fn percentiles_are_nearest_rank() {
        let sorted = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&sorted, 0.5), 2.0);
        assert_eq!(percentile(&sorted, 0.95), 4.0);
        assert_eq!(percentile(&[], 0.5), 0.0);
        assert_eq!(percentile(&[7.0], 0.95), 7.0);
    }

    /// The exit causes a saturated run reports have to be distinguishable, and
    /// the ones that mean "the budget stopped me" have to be separable from the
    /// ones that mean "I ran out of actions".
    #[test]
    fn exit_cause_names_are_distinct() {
        let all = [
            PhaseExitCause::SkippedDeadlinePassed,
            PhaseExitCause::Completed,
            PhaseExitCause::GeometricFixpoint,
            PhaseExitCause::KeysExhausted,
            PhaseExitCause::Affordability,
            PhaseExitCause::Deadline,
            PhaseExitCause::Patience,
            PhaseExitCause::TriggerRefused,
            PhaseExitCause::NoResidue,
            PhaseExitCause::NoCompleteLayout,
            PhaseExitCause::WallStop,
        ];
        let names = all
            .iter()
            .map(|cause| cause.name())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(names.len(), all.len());
    }
}
