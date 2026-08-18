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
    pub started_seconds: f64,
    pub elapsed_seconds: f64,
    pub work_units: u64,
    pub exact_valid: bool,
    pub raw_depth_mm: Option<f64>,
    pub archive_disposition: Option<String>,
    pub published: bool,
    pub failure_reason: Option<String>,
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
        }
    }
}

/// The budget meter. Wall time and work units behind one interface, so the
/// schedule below reads a *fraction spent* and never a clock directly.
struct BudgetMeter {
    budget: PortfolioBudget,
    started: Instant,
    work_base: u64,
}

impl BudgetMeter {
    fn new(budget: PortfolioBudget) -> Self {
        Self {
            budget,
            started: Instant::now(),
            work_base: work_units_now(),
        }
    }

    fn seconds(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn work_units(&self) -> u64 {
        work_units_now().saturating_sub(self.work_base)
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
    /// The fraction of the budget the protected phase 0 spent. Every later
    /// phase's deadline is a fraction of what is left after it; see
    /// [`PhaseSchedule`].
    protected_fraction: f64,
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
            exact_valid,
            descents: 0,
            placements,
        };
        let disposition = self.archive.offer(basin);
        (disposition, Some(raw_depth_mm))
    }

    /// Runs one deep-operator mode against one parent, archives whatever it
    /// produced, attempts publication, and records the call.
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
    ) -> GeneralPersistentVacancyDiagnostics {
        let mut relaxed = self.base_relaxed_settings();
        relaxed.persistent_vacancy_mode = mode;
        relaxed.persistent_vacancy_target_depth_mm = target;
        relaxed.persistent_vacancy_allow_unpinned_parent = true;
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
        let elapsed_seconds = self.meter.seconds() - started_seconds;
        let work_units = self.meter.work_units().saturating_sub(started_work);
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
        if !produced.is_empty() {
            let (archived, depth) = self.archive_layout(
                produced.clone(),
                BasinOperator::Mode(mode),
                parent_fingerprint.clone(),
            );
            disposition = Some(format!("{archived:?}"));
            raw_depth_mm = depth;
            published = self.try_publish(&produced, &format!("mode{mode}"));
        }
        self.operator_calls.push(OperatorCallReport {
            phase: self.phase_name.clone(),
            operator: format!("mode{mode}"),
            parent_fingerprint,
            started_seconds,
            elapsed_seconds,
            work_units,
            exact_valid: population.exact_valid,
            raw_depth_mm,
            archive_disposition: disposition,
            published,
            failure_reason: population.failure_reason.clone(),
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
    fn affordable(&self, deadline: f64, operator: &str, multiple: f64) -> bool {
        if !self.meter.has_room(deadline) {
            return false;
        }
        match self.mean_operator_cost(operator) {
            None => true,
            Some(cost) => self.meter.remaining_to(deadline) >= multiple * cost,
        }
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
        protected_fraction: 0.0,
    };

    // Everything phase 0 produced goes into the archive, including the arms
    // that lost: the coupled separator's control, treatment and
    // boundary-projection arms are three structurally different complete
    // layouts that the engine currently throws away.
    coordinator.archive_layout(
        constructed.placements.clone(),
        BasinOperator::Constructor,
        None,
    );
    let m0_placements = coordinator.incumbent.result.placements.clone();
    coordinator.archive_layout(m0_placements, BasinOperator::RelaxedM0, None);
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
                coordinator.archive_layout(placements, BasinOperator::CoupledSeparator, None);
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
    });
    // Everything after this point is a fraction of what phase 0 left, not of
    // the whole budget. See `PhaseSchedule`.
    coordinator.protected_fraction = coordinator.meter.spent_fraction().clamp(0.0, 1.0);

    // The constructor clamp, derived from the request rather than pinned.
    let area_lower_bound_depth_mm = area_lower_bound_depth_mm(pieces, fast_settings)?;
    let constructor_clamp_mm =
        constructor_clamp_mm(area_lower_bound_depth_mm, constructed_depth_mm);

    // ---- phase 1: alternation quanta across the distinct frontier ---------
    // First, not third. It is the most productive operator this schedule has
    // (9 publications in 18 calls on the v1 stream) and the constructor slice
    // that used to precede it published nothing in nineteen.
    let template_epochs = settings.relaxed_template.epochs.max(1);
    coordinator.run_phase("descent", settings.schedule.descent_by, |run| {
        let mut cycles = run.settings.descent_cycles.max(1);
        let mut epochs = run.settings.descent_relaxed_epochs.max(1);
        loop {
            // Re-selected every round, because a quantum's own output is an
            // archive member and therefore a candidate parent for the next
            // round. That is what turns a round-robin over the frontier into a
            // progressive descent rather than a repeated one.
            let frontier = run.archive.distinct_frontier(run.settings.descent_states);
            let mut spent_any = false;
            for basin in frontier {
                if !run.affordable(run.deadline, "mode22", 1.0) {
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
                break;
            }
            let deepened_cycles = (cycles * 2).min(ALTERNATION_MAX_CYCLES);
            let deepened_epochs = (epochs * 2).min(template_epochs);
            if deepened_cycles == cycles && deepened_epochs == epochs {
                run.descent_stalled = true;
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
            if !run.affordable(run.deadline, "mode23", 1.0) {
                return;
            }
            // Re-selected every attempt: a crossover that published moved the
            // incumbent, and the next pair should be drawn from where the
            // archive is now, not from where it was.
            let frontier = run
                .archive
                .distinct_frontier(run.settings.crossover_states.max(2));
            if frontier.len() < 2 {
                return;
            }
            let fingerprints = frontier
                .iter()
                .map(|basin| basin.fingerprint.clone())
                .collect::<Vec<_>>();
            let Some((left, right, key)) =
                first_unattempted_crossover_pair(&fingerprints, &run.attempted)
            else {
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
        if !run.affordable(run.deadline, "mode22", 1.0) {
            return;
        }
        let parent = run.incumbent.result.placements.clone();
        let fingerprint = run.incumbent.fingerprint.clone();
        let Some(depth) = run.incumbent.raw_depth_mm else {
            return;
        };
        if run.already_attempted(format!("22c:{fingerprint}")) {
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
        );
        if compressed.exact_valid {
            // Nothing to legalize. This is the whole demotion: on this stream
            // the trigger does not fire, and a phase that does not fire costs
            // nothing instead of costing six refused calls.
            return;
        }
        let residue = crate::search::general_relaxed::fast_placements_from_coupled_diagnostics(
            &compressed.final_placements,
        );
        if residue.len() != run.pieces.len() || !run.meter.has_room(run.deadline) {
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
        // One rung of the engine's own construction drop ladder below the
        // residue, which is the smallest bound this engine ever asks a
        // legalizer for.
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
            BasinTrigger::Never => return,
            BasinTrigger::OnStall if !run.descent_stalled => return,
            _ => {}
        }
        let priced = run.settings.basin_trigger == BasinTrigger::WhenDescendable;
        let patience = run.settings.basin_patience.max(1);
        let mut barren = 0usize;
        for slot in 0..run.settings.basin_slots {
            let publications_before = run.publications.len();
            if !run.meter.has_room(run.deadline) {
                return;
            }
            if priced {
                // A quantum is the price of *using* a basin, and a basin that
                // is not used is the 19/19 refusal. An arm has never been
                // priced when the first one is drawn, so it is charged a
                // quantum's price until it has priced itself.
                let Some(quantum) = run.mean_operator_cost("mode22") else {
                    return;
                };
                let arm = run.mean_operator_cost("mode20").unwrap_or(quantum);
                if run.meter.remaining_to(run.deadline) < arm + quantum {
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
                return;
            }
            if !run.meter.has_room(run.deadline) {
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
            );
            if run.publications.len() > publications_before {
                barren = 0;
            } else {
                barren += 1;
                if barren >= patience {
                    return;
                }
            }
        }
    });

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

    let elapsed_seconds = coordinator.meter.seconds();
    let work_units = coordinator.meter.work_units();
    let budget = coordinator.meter.budget;
    let descent_stalled = coordinator.descent_stalled;
    Ok(PortfolioOutcome {
        descent_stalled,
        result: coordinator.incumbent.result.clone(),
        incumbent: coordinator.incumbent,
        archive: coordinator.archive.report(),
        m0_diagnostics: Box::new(m0.diagnostics),
        phases: coordinator.phases,
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
        let entered_seconds = self.meter.seconds();
        let entered_work = self.meter.work_units();
        let calls_before = self.operator_calls.len();
        let publications_before = self.publications.len();
        let skipped = !self.meter.has_room(deadline);
        self.phase_name = name.to_owned();
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
        });
    }
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
            started_seconds: 0.0,
            elapsed_seconds: 1.25,
            work_units: 3_000_000,
            exact_valid: true,
            raw_depth_mm: None,
            archive_disposition: None,
            published: false,
            failure_reason: None,
        };
        assert_eq!(wall.call_cost(&call), 1.25);
        assert_eq!(work.call_cost(&call), 3_000_000.0);
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
    }
}
