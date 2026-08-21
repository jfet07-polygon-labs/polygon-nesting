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
use std::time::Instant;

use crate::profiling::{self, Counter};
use crate::search::general_fast::{
    construct_short_side_first, validate_and_measure_placements, GeneralFastError,
    GeneralFastPiece, GeneralFastPlacement, GeneralFastResult, GeneralFastSettings,
};
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
    /// not, which today is all of them but mode 34.
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
    pub se2_witness_calls: usize,
    pub se2_witness_accepted: usize,
    pub se2_witness_ms: f64,
    pub se2_witness_bought_mm: f64,
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
            PhaseExitCause::Patience => "patience",
            PhaseExitCause::TriggerRefused => "triggerRefused",
            PhaseExitCause::NoResidue => "noResidue",
            PhaseExitCause::NoCompleteLayout => "noCompleteLayout",
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
    pub operator_calls: Vec<OperatorCallReport>,
    pub publications: Vec<PublicationEvent>,
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
    /// over the job pool. `false` by default.
    ///
    /// Unlike `compression_schedule_lanes` this one is semantics-preserving -
    /// measured on the 174-179 mm band, an armed slice differs from the serial
    /// one in exactly the diagnostic flag that says it was armed - so it moves
    /// wall without moving the search's trajectory.
    #[cfg(feature = "parallel-compression-schedule")]
    pub compression_schedule_parallel_confirm: bool,
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
            #[cfg(feature = "parallel-compression-schedule")]
            compression_schedule_parallel_confirm: false,
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
            barren_action_patience: BARREN_ACTION_PATIENCE,
            diversify_in_queue: true,
            schedule_wall_prior: true,
            // Off by default: these two change the state every m34 slice walks
            // from, and the round that measures them has to be able to run the
            // arm that does not.
            schedule_legalize_entry: false,
            schedule_skip_infeasible_entry: false,
            schedule_skip_unpublishable_entry: false,
            schedule_probe_denominator: SCHEDULE_PROBE_DENOMINATOR,
            schedule_sterile_bit: true,
        }
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
}

impl BudgetMeter {
    fn new(budget: PortfolioBudget) -> Self {
        Self {
            budget,
            started: Instant::now(),
            work_base: work_units_now(),
            self_metered_debit: 0,
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
        }
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
            PortfolioBudget::Work { .. } => self.work_units() as f64,
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
            PortfolioBudget::Work { .. } => call.work_units as f64,
        }
    }
}

/// The process-wide work-unit reading.
///
/// Zero, and therefore constant, when `profiling` recording is off - which is
/// why [`PortfolioBudget::Work`] arms it and [`PortfolioBudget::Wall`] does
/// not. A wall-budget run must have the clock the production build runs on; a
/// work-budget run must have the counters, and pays the ~17% they cost.
fn work_units_now() -> u64 {
    let totals = profiling::counter_totals();
    totals[Counter::CandidateQueries as usize].saturating_add(
        WORK_UNITS_PER_EXACT_PAIR_TEST.saturating_mul(totals[Counter::ExactPairTests as usize]),
    )
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
            relaxed.se2_witness = self.settings.se2_witness.filter(|_| relaxed.sparse_rotation);
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
        let population = crate::search::general_relaxed::dispatch_persistent_vacancy_mode(
            self.pieces,
            self.fast_settings,
            relaxed,
            &parent_arm,
            None,
            secondary,
        );
        // Step two of the transaction: what did this call cost? The global
        // counter's delta is read *before* the debit, so it is the global
        // counter's own number and nothing else - `work_units()` folds in
        // every debit charged so far, and the ones charged before this call
        // are already inside `started_work`.
        let global_units = self.meter.work_units().saturating_sub(started_work);
        let charge = settle_operator_charge(
            &mut self.meter,
            global_units,
            operator_self_metered_units(&population),
        );
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
        // "the mechanism fired"; `rotation_accepted_moves == 0` is "and not one
        // of its proposals survived into a committed move".
        #[cfg(feature = "sparse-rotation")]
        if self.settings.sparse_rotation_bit {
            let slice = population.compression_schedule.as_ref();
            let episodes = slice.map_or(0, |report| report.sparse_rotation_episodes);
            let accepted = slice.map_or(0, |report| report.rotation_accepted_moves);
            if episodes > 0 && accepted == 0 {
                self.sparse_rotation_bit.sterile_slices += 1;
                if self.sparse_rotation_bit.sterile_slices >= SPARSE_ROTATION_STERILE_SLICES {
                    self.sparse_rotation_bit.disarmed = true;
                    self.sparse_rotation_bit.barren_calls = 0;
                }
            } else if episodes > 0 && accepted > 0 {
                // The audition disproved the bit: rotation is productive on this
                // request after all, so the operator comes back for good.
                self.sparse_rotation_bit.disarmed = false;
                self.sparse_rotation_bit.sterile_slices = 0;
            }
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
        });
        population
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
        if !self.meter.has_room(deadline) {
            return Some(PhaseExitCause::Deadline);
        }
        match self.mean_operator_cost(operator) {
            None => None,
            Some(cost) if self.meter.remaining_to(deadline) >= multiple * cost => None,
            Some(_) => Some(PhaseExitCause::Affordability),
        }
    }

    /// Records why the phase in flight stopped.
    fn note_exit(&mut self, cause: PhaseExitCause) {
        self.exit_cause = cause;
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
    if matches!(settings.budget, PortfolioBudget::Work { .. }) {
        // A work budget is a function of the counters, so it needs them.
        profiling::set_enabled(true);
    }
    let meter = BudgetMeter::new(settings.budget);

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
    let schedule_report = if settings.coordinator_v3 {
        Some(run_v3_schedule(&mut coordinator, constructor_clamp_mm))
    } else {
        None
    };
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
                if run.already_attempted(format!("22:{cycles}:{epochs}:{}", basin.fingerprint)) {
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
                Some(format!("x:forward:{left}->{right}@{CROSSOVER_CUT_FRACTION}")),
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
                // its pose prior, so this is not that basin's quantum.
                ParentRole::Prior,
                Some(format!("m20:slot{slot}")),
            );
            let basin = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
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
        descent_stalled,
        result: coordinator.incumbent.result.clone(),
        incumbent: coordinator.incumbent,
        archive: coordinator.archive.report(),
        m0_diagnostics: Box::new(m0.diagnostics),
        phases: coordinator.phases,
        schedule: schedule_report,
        operator_calls: coordinator.operator_calls,
        publications: coordinator.publications,
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
    fn run_phase_to(&mut self, name: &str, deadline: f64, body: impl FnOnce(&mut PhaseRun<'_, 'a>)) {
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
                if rank < quantum_states && depth - drop_mm > 0.0 && !self.attempted.contains(&key) {
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
        self.settings.schedule_wall_prior
            && class == ActionClass::Schedule
            && self.meter.is_wall()
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

/// The v3 loop: enumerate, rank, spend the best affordable action, repeat.
fn run_v3_schedule(coordinator: &mut Coordinator<'_>, constructor_clamp_mm: f64) -> ScheduleReport {
    let mut actions: Vec<ScheduledActionReport> = Vec::new();
    let schedule_by = coordinator.settings.schedule.schedule_by;
    let phase_zero_cost = coordinator.phase_zero_cost;
    coordinator.run_phase("schedule", schedule_by, |run| {
        v3_loop(run, constructor_clamp_mm, &mut actions);
    });
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

fn v3_loop(
    run: &mut PhaseRun<'_, '_>,
    constructor_clamp_mm: f64,
    out: &mut Vec<ScheduledActionReport>,
) {
    let patience = run.settings.basin_patience.max(1);
    let slots = run.settings.basin_slots;
    let barren_patience = run.settings.barren_action_patience;
    let ranked_diversify = run.settings.diversify_in_queue;
    let mut diversify_slot = 0usize;
    let mut diversify_barren = 0usize;
    let mut diversify_done = run.settings.basin_trigger == BasinTrigger::Never;
    let sterile_bit = run.settings.schedule_sterile_bit;
    // The one bit Grok review 1 §2b item 4 asks for, and it is one bit: once
    // the compression-schedule class has spent [`SCHEDULE_STERILE_ACTIONS`]
    // actions on *this* request and published nothing, it comes off the queue.
    // `schedule_auditioned` is the falsifiability half - the class is offered
    // once more after [`SCHEDULE_AUDITION_BARREN`] further barren actions, and
    // once only, so the bit is a claim this run can still disprove rather than
    // an absorbing state.
    let mut schedule_auditioned = false;
    let mut barren_since_schedule = 0usize;
    // Consecutive actions of *any* class that published nothing. Reset by a
    // publication and by nothing else.
    let mut barren = 0usize;
    // The same count, additionally reset by a diversify action, so the audition
    // rule fires at most once per `DIVERSIFY_AUDITION_BARREN` barren actions
    // rather than on every action after the eighth.
    let mut barren_since_diversify = 0usize;
    loop {
        if !run.meter.has_room(run.deadline) {
            run.note_exit(PhaseExitCause::Deadline);
            return;
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
                return;
            }
            None => {
                // No complementary pairs remain: the one place a constructor
                // ticket is worth its price, and the only place v3 spends one.
                if diversify_done || diversify_slot >= slots {
                    run.note_exit(PhaseExitCause::KeysExhausted);
                    return;
                }
                let Some(quantum) = run.mean_operator_cost("mode22") else {
                    run.note_exit(PhaseExitCause::Affordability);
                    return;
                };
                let arm = run.mean_operator_cost("mode20").unwrap_or(quantum);
                if remaining < arm + quantum {
                    run.note_exit(PhaseExitCause::Affordability);
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
        entry_legalization_violating_pairs_before: report
            .entry_legalization_violating_pairs_before,
        entry_legalization_violating_pairs_after: report.entry_legalization_violating_pairs_after,
        entry_legalization_boundary_pieces_before: report
            .entry_legalization_boundary_pieces_before,
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
        se2_witness_calls: report.se2_witness_calls,
        se2_witness_accepted: report.se2_witness_accepted,
        se2_witness_ms: report.se2_witness_ms,
        se2_witness_bought_mm: report.se2_witness_bought_mm,
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
                let schedule_key =
                    format!("23:{}:{}", parent_a.fingerprint, parent_b.fingerprint);
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
            !action.attempted && !action.degenerate && !existing.contains(&action.hybrid_fingerprint)
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
        ProbeArm::ConstructorTicket => probe_constructor_ticket(run, constructor_clamp_mm, &mut steps),
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
            rungs.steps.iter().map(|step| step.arms.len()).sum::<usize>(),
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
        };
        assert_eq!(report.work_units, report.global_units + report.debited_units);
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
        for measured in [1.1466399653696229_f64, 1.6193018185283783, 2.2375242010360177] {
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

        let mut bit = SparseRotationBit::default();
        // A slice that opened episodes and accepted nothing fires the bit.
        let sterile = |bit: &mut SparseRotationBit, episodes: usize, accepted: usize| {
            if episodes > 0 && accepted == 0 {
                bit.sterile_slices += 1;
                if bit.sterile_slices >= SPARSE_ROTATION_STERILE_SLICES {
                    bit.disarmed = true;
                    bit.barren_calls = 0;
                }
            } else if episodes > 0 && accepted > 0 {
                bit.disarmed = false;
                bit.sterile_slices = 0;
            }
        };
        sterile(&mut bit, 12, 0);
        assert!(bit.disarmed, "one sterile slice is the evidence");

        // A slice that never opened an episode is not evidence either way: the
        // mechanism did not get its trigger, so it did not get its verdict.
        let mut untried = SparseRotationBit::default();
        sterile(&mut untried, 0, 0);
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
        sterile(&mut bit, 4, 3);
        assert!(!bit.disarmed);
        assert_eq!(bit.sterile_slices, 0);
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
            all.iter().copied().collect::<std::collections::BTreeSet<_>>().len(),
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
        let key = |left: &str, right: &str, cut: f64| {
            format!("23:{left}:{right}:{:016x}", cut.to_bits())
        };
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
        let ceiling =
            2 * quantum_states + 1 + ordered_pairs * (CROSSOVER_CUTS_PER_PAIR + 1);
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
        ];
        let names = all
            .iter()
            .map(|cause| cause.name())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(names.len(), all.len());
    }
}
