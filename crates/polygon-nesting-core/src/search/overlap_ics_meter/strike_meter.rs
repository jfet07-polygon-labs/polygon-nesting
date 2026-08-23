//! **The two-arm strike meter.** One classifier, two patience predicates.
//!
//! docs/economics-round-spec.md, funded change 1, verbatim:
//!
//! > TREATMENT: work-denominated impatient strikes - after each master batch,
//! > `observe_raw` classifies (2 % Substantial / Marginal / None, untouched);
//! > None adds the batch's all-eight-workers `sample_evaluations`; strike at
//! > the quantum, frozen as KNOB: explore **1_630_000**, compress **815_000**
//! > […]; counts 3/5 unchanged; overshoot ≤ one batch. […]
//! > CONTROL: the frozen literals 200/3/100/5/0.98 on the identical executor
//! > and pacer - **strike semantics are the only delta between arms.**
//!
//! That last clause is the whole design of this module. Everything the two
//! arms share - the classifier, the minimum snapshot, the improving-strike
//! reset at 0.98, the strike counts, the rollback point - is written **once**
//! and reached by both. The only `match` on [`StrikeConfig`] in the whole file
//! is [`StrikeMeter::patience_exhausted`]. If a future edit adds a second one,
//! the arms have started to differ in something other than strike semantics
//! and the paired attribution clause stops meaning what it says.
//!
//! # The honest label
//!
//! Grok review 14 Round 2 vote 1, signed by both consultants: `1_630_000` is
//! **not** "what 200 always meant". One master iteration is an algorithmic
//! event - one eight-worker relocate tournament, one winner installation, one
//! all-row GLS update - and replacing 200 of them with however many consume
//! 1.63 M evaluations changes the number of GLS updates before rollback. The
//! treatment arm is *a distinct impatient-strike policy pre-derived from
//! source*, and the control arm is the closed member's own semantics. Neither
//! name is a hedge: the promotion clause decides between them, and if
//! attribution fails the control's policy remains the member with **no second
//! guess on the quanta**.
//!
//! # Where the quanta come from
//!
//! Sparrow's same-machine 3.742 M evaluations/s ÷ 460 iterations/s × 200 for
//! explore and × 100 for compress (docs/experiments/sparrow-mixed61). Grok
//! froze the two numbers as a KNOB in review 14 Round 2: *"`1_630_000` /
//! `815_000`. Not a more 'precise' 1 626 957 - 3742 K is already a truncated
//! `usize` from `f32`."* [`EXPLORE_WORK_QUANTUM`] and
//! [`COMPRESS_WORK_QUANTUM`] are those literals and [`frozen_literals_intact`]
//! is the tripwire that says so.
//!
//! # The order of one turn, and which batch pays
//!
//! `Engine::separate`'s loop folds the state at the top of a turn, classifies
//! that reading, tests the band, tests patience, and only then runs the
//! tournament at the bottom. So the reading classified at the top of turn *N*
//! is the state that turn *N-1*'s batch produced, and *that* is the batch
//! whose `sample_evaluations` a `None` charges. [`StrikeMeter::observe`] takes
//! both together for exactly this reason: a caller cannot accidentally charge
//! the wrong batch, because there is no way to charge one at all except by
//! classifying the reading it produced. The entry turn, which no batch
//! produced, charges zero.
//!
//! # Overshoot ≤ one batch
//!
//! The quantum is tested **after** the batch's cost is added, so accumulated
//! None-work at the moment of a strike is strictly less than
//! `quantum + cost_of_the_batch_that_crossed_it`. That is the spec's clause,
//! and [`tests::overshoot_never_exceeds_one_batch`] proves it over random
//! variable-cost sequences rather than asserting it in a comment.

use crate::search::overlap_ics::{
    observe_raw, Phase, RawObservation, SeparateLimits, STRIKE_IMPROVEMENT_RATIO,
};

/// The treatment arm's explore quantum, in all-eight-workers sample
/// evaluations. **Frozen as a KNOB by the spec**; no second guess.
pub const EXPLORE_WORK_QUANTUM: u64 = 1_630_000;

/// The treatment arm's compress quantum: half the explore quantum, because
/// Sparrow's compress patience is half its explore patience (100 vs 200).
/// **Frozen as a KNOB by the spec**; no second guess.
pub const COMPRESS_WORK_QUANTUM: u64 = 815_000;

/// Which arm the trajectory is running.
///
/// One enum, because the spec's promotion clause is a paired comparison of two
/// runs of *the same* executor and pacer that differ in this value and in
/// nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrikeConfig {
    /// **CONTROL.** The frozen literals: 200 iterations without a 2 %
    /// improvement in explore, 100 in compress, 3 and 5 strikes, the improving
    /// reset at 0.98. Carries the shipped [`SeparateLimits`] values rather
    /// than copies of them, so the constants cannot drift apart.
    IterationStrikes {
        explore: SeparateLimits,
        compress: SeparateLimits,
    },
    /// **TREATMENT.** The work-denominated impatient policy: strike when
    /// accumulated None-work reaches the phase's quantum. Counts unchanged at
    /// 3/5, classifier unchanged, improving reset unchanged.
    WorkStrikes {
        explore_quantum: u64,
        compress_quantum: u64,
        explore_strikes: u32,
        compress_strikes: u32,
    },
}

impl StrikeConfig {
    /// The control arm exactly as the closed member froze it.
    pub const CONTROL: Self = Self::IterationStrikes {
        explore: SeparateLimits::EXPLORE,
        compress: SeparateLimits::COMPRESS,
    };

    /// The treatment arm exactly as the spec's KNOB froze it.
    pub const TREATMENT: Self = Self::WorkStrikes {
        explore_quantum: EXPLORE_WORK_QUANTUM,
        compress_quantum: COMPRESS_WORK_QUANTUM,
        explore_strikes: SeparateLimits::EXPLORE.strikes,
        compress_strikes: SeparateLimits::COMPRESS.strikes,
    };

    /// The label an evidence document prints for the arm. Never parsed by
    /// anything that decides.
    pub fn arm(self) -> &'static str {
        match self {
            Self::IterationStrikes { .. } => "control-iteration-strikes",
            Self::WorkStrikes { .. } => "treatment-work-strikes",
        }
    }

    /// This arm's rule for one phase.
    pub fn rule(self, phase: Phase) -> StrikeRule {
        match self {
            Self::IterationStrikes { explore, compress } => {
                let limits = match phase {
                    Phase::Explore => explore,
                    Phase::Compress => compress,
                };
                StrikeRule {
                    patience: Patience::Iterations(limits.iterations_without_improvement),
                    strikes: limits.strikes,
                }
            }
            Self::WorkStrikes {
                explore_quantum,
                compress_quantum,
                explore_strikes,
                compress_strikes,
            } => match phase {
                Phase::Explore => StrikeRule {
                    patience: Patience::Work(explore_quantum),
                    strikes: explore_strikes,
                },
                Phase::Compress => StrikeRule {
                    patience: Patience::Work(compress_quantum),
                    strikes: compress_strikes,
                },
            },
        }
    }
}

/// What exhausts one strike's patience.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Patience {
    /// Master batches classified `None` since the last reset. The control.
    Iterations(u64),
    /// All-eight-workers `sample_evaluations` charged by `None` batches since
    /// the last reset. The treatment.
    Work(u64),
}

/// One phase's resolved rule: a patience threshold and a strike count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrikeRule {
    pub patience: Patience,
    pub strikes: u32,
}

/// What a completed strike did to the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrikeEvent {
    /// True when the strike beat the previous strike's entry by 2 % and
    /// therefore reset the count instead of raising it. Sparrow's
    /// `min_loss < 0.98 * initial_strike_loss`.
    pub improving: bool,
    /// The count after the transition.
    pub strikes: u32,
    /// True when `strikes >= rule.strikes`: the separation is out.
    pub struck_out: bool,
    /// The patience that had accumulated when the strike fired: batches for
    /// the control, sample evaluations for the treatment. The evidence
    /// document's `strikeCost`.
    pub accumulated: u64,
    /// The cost of the batch that crossed the threshold. `accumulated` minus
    /// this is strictly below the quantum, which **is** the overshoot clause.
    pub crossing_batch: u64,
}

/// **The strike accounting of one separation, in one place.**
///
/// Holds no state the engine also holds: the caller keeps the layout snapshot
/// and performs the rollback; this keeps the numbers that decide when to.
///
/// Both arms maintain **both** counters. The control's work counter and the
/// treatment's iteration counter are shadow diagnostics - Sol review 19 §P3
/// asks for "sample evaluations beside iterations" - and carrying them in both
/// arms is what makes the paired documents comparable term by term.
#[derive(Clone, Debug)]
pub struct StrikeMeter {
    rule: StrikeRule,
    /// This separation's best raw Φ. Owned by [`observe_raw`]; never reset by
    /// a strike, because a strike rolls the *layout* back to this minimum and
    /// does not un-see it.
    min_raw: f64,
    /// The strike-best the improving reset measures the next strike against.
    /// Seeded with the separation's entry raw Φ, as `Engine::separate` does.
    strike_entry_raw: f64,
    /// `observe_raw`'s own counter: None batches since the last Substantial or
    /// the last strike.
    since_improvement: u64,
    /// All-eight-workers sample evaluations charged by those None batches.
    accumulated_none_work: u64,
    /// The cost of the most recent batch charged, for the overshoot clause.
    last_charged_batch: u64,
    strikes: u32,
    // ------------------------------------------------ shadow diagnostics --
    batches: u64,
    charged_work: u64,
    substantial: u64,
    marginal: u64,
    none: u64,
}

impl StrikeMeter {
    /// A meter for one separation, at that separation's entry raw Φ.
    ///
    /// `entry_raw` is `energy::fold(&state).raw` before the first turn, which
    /// is what `Engine::separate` seeds `strike_entry_raw` with. `min_raw`
    /// starts at infinity so the first reading is always a new minimum, again
    /// as the shipped loop does.
    pub fn new(rule: StrikeRule, entry_raw: f64) -> Self {
        Self {
            rule,
            min_raw: f64::INFINITY,
            strike_entry_raw: entry_raw,
            since_improvement: 0,
            accumulated_none_work: 0,
            last_charged_batch: 0,
            strikes: 0,
            batches: 0,
            charged_work: 0,
            substantial: 0,
            marginal: 0,
            none: 0,
        }
    }

    /// Convenience: a meter for `config`'s rule in `phase`.
    pub fn for_phase(config: StrikeConfig, phase: Phase, entry_raw: f64) -> Self {
        Self::new(config.rule(phase), entry_raw)
    }

    /// **The one classification per turn**, and the one charge.
    ///
    /// `raw` is the fold at the top of the turn. `batch_sample_evaluations` is
    /// the all-eight-workers `sample_evaluations` of the batch that produced
    /// it - zero on the entry turn, which no batch produced.
    ///
    /// Returns [`observe_raw`]'s own verdict unchanged, because the caller
    /// still owes the minimum snapshot on
    /// [`RawObservation::is_new_minimum`]. This function does not take the
    /// snapshot and must not: the state is the engine's.
    ///
    /// * `Substantial` - resets accumulated None-work (and, via `observe_raw`,
    ///   the iteration counter).
    /// * `Marginal` - moves the snapshot, adds nothing, resets nothing.
    /// * `None` - adds this batch's evaluations to accumulated None-work (and,
    ///   via `observe_raw`, one to the iteration counter).
    pub fn observe(&mut self, raw: f64, batch_sample_evaluations: u64) -> RawObservation {
        self.batches += 1;
        self.charged_work = self.charged_work.saturating_add(batch_sample_evaluations);
        self.last_charged_batch = batch_sample_evaluations;
        // The frozen classifier, called and not reimplemented. It owns
        // `min_raw` and `since_improvement`; this function owns the work
        // counter that shadows the second of them.
        let class = observe_raw(raw, &mut self.min_raw, &mut self.since_improvement);
        match class {
            RawObservation::Substantial => {
                self.substantial += 1;
                self.accumulated_none_work = 0;
            }
            RawObservation::Marginal => self.marginal += 1,
            RawObservation::None => {
                self.none += 1;
                self.accumulated_none_work = self
                    .accumulated_none_work
                    .saturating_add(batch_sample_evaluations);
            }
        }
        class
    }

    /// **The only place the two arms differ.**
    ///
    /// True when this strike's patience is spent and the caller must roll the
    /// layout back and call [`StrikeMeter::strike`].
    pub fn patience_exhausted(&self) -> bool {
        match self.rule.patience {
            Patience::Iterations(limit) => self.since_improvement >= limit,
            Patience::Work(quantum) => self.accumulated_none_work >= quantum,
        }
    }

    /// The strike ladder, after the caller has rolled the layout back.
    ///
    /// Identical in both arms, and identical to `Engine::separate`'s: the
    /// improving strike (`min_raw < 0.98 * strike_entry_raw`) resets the count,
    /// anything else raises it; the strike-best moves to `min_raw`; **both**
    /// patience counters clear. `min_raw` itself does not move - the layout
    /// went back to it, the separation did not un-see it.
    pub fn strike(&mut self) -> StrikeEvent {
        let improving = self.min_raw < STRIKE_IMPROVEMENT_RATIO * self.strike_entry_raw;
        if improving {
            self.strikes = 0;
        } else {
            self.strikes += 1;
        }
        let accumulated = match self.rule.patience {
            Patience::Iterations(_) => self.since_improvement,
            Patience::Work(_) => self.accumulated_none_work,
        };
        let crossing_batch = match self.rule.patience {
            // One batch of patience is one batch, whatever it cost.
            Patience::Iterations(_) => 1,
            Patience::Work(_) => self.last_charged_batch,
        };
        self.strike_entry_raw = self.min_raw;
        self.since_improvement = 0;
        self.accumulated_none_work = 0;
        StrikeEvent {
            improving,
            strikes: self.strikes,
            struck_out: self.strikes >= self.rule.strikes,
            accumulated,
            crossing_batch,
        }
    }

    pub fn rule(&self) -> StrikeRule {
        self.rule
    }

    pub fn strikes(&self) -> u32 {
        self.strikes
    }

    /// The best raw Φ this separation has seen. The pool entry's `raw_phi`.
    pub fn min_raw(&self) -> f64 {
        self.min_raw
    }

    pub fn strike_entry_raw(&self) -> f64 {
        self.strike_entry_raw
    }

    /// The control's patience counter, live in both arms.
    pub fn since_improvement(&self) -> u64 {
        self.since_improvement
    }

    /// The treatment's patience counter, live in both arms.
    pub fn accumulated_none_work(&self) -> u64 {
        self.accumulated_none_work
    }

    /// Shadow counters, for the paired document. Never read by a decision.
    pub fn shadow(&self) -> ShadowCounters {
        ShadowCounters {
            batches: self.batches,
            charged_work: self.charged_work,
            substantial: self.substantial,
            marginal: self.marginal,
            none: self.none,
        }
    }
}

/// The diagnostics both arms carry, so the paired comparison is term by term.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShadowCounters {
    /// Turns classified, i.e. `observe` calls.
    pub batches: u64,
    /// Sample evaluations offered to the meter, whatever the class did with
    /// them. `charged_work - accumulated_none_work` at any moment is the work
    /// the improving classes forgave.
    pub charged_work: u64,
    pub substantial: u64,
    pub marginal: u64,
    pub none: u64,
}

/// **The frozen-literal tripwire.**
///
/// The spec freezes `200 / 3 / 100 / 5 / 0.98` and the KNOB quanta
/// `1_630_000` / `815_000`. This returns `true` only while every one of those
/// six numbers is what it was signed as, reading the shipped constants rather
/// than copies, so an edit to `SeparateLimits::EXPLORE` in `overlap_ics` turns
/// this red from a module that never touches that file.
pub fn frozen_literals_intact() -> bool {
    SeparateLimits::EXPLORE.iterations_without_improvement == 200
        && SeparateLimits::EXPLORE.strikes == 3
        && SeparateLimits::COMPRESS.iterations_without_improvement == 100
        && SeparateLimits::COMPRESS.strikes == 5
        // Bit equality, not `==`: the literal the spec signed is `0.98`, and a
        // constant that had drifted to the next representable double would
        // pass a tolerance test while being a different number.
        && STRIKE_IMPROVEMENT_RATIO.to_bits() == 0.98_f64.to_bits()
        && EXPLORE_WORK_QUANTUM == 1_630_000
        && COMPRESS_WORK_QUANTUM == 815_000
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // TWO reference implementations, both written from text rather than from
    // the module above, because a property test against a paraphrase of the
    // implementation proves only that the paraphrase was faithful.
    //
    //   * `SpecReference` is written from docs/economics-round-spec.md's own
    //     sentences, and it keeps the whole batch history rather than running
    //     counters, so it cannot agree with `StrikeMeter` by having the same
    //     shape.
    //   * `ShippedInlineReference` is a literal transcription of the strike
    //     block of `Engine::separate` at this commit. It exists to prove the
    //     control arm is the closed member's semantics and not a re-derivation
    //     of them.
    // ---------------------------------------------------------------------

    /// One turn of a synthetic trajectory: the raw Φ the fold read, and what
    /// the batch that produced it cost.
    #[derive(Clone, Copy, Debug)]
    struct Turn {
        raw: f64,
        cost: u64,
    }

    /// What a driven run produced, in a shape both the meter and the
    /// references can report.
    #[derive(Clone, Debug, Default, PartialEq)]
    struct Run {
        classes: Vec<u8>,
        /// `(turn index, strikes after, improving)`.
        strikes: Vec<(usize, u32, bool)>,
        struck_out_at: Option<usize>,
        min_raw_bits: u64,
        strike_entry_bits: u64,
    }

    fn class_code(class: RawObservation) -> u8 {
        match class {
            RawObservation::Substantial => 0,
            RawObservation::Marginal => 1,
            RawObservation::None => 2,
        }
    }

    /// The module under test, driven exactly the way `Engine::separate` drives
    /// its loop: classify the top-of-turn fold, then test patience, then (if
    /// the separation survives) run the next batch.
    fn drive_meter(rule: StrikeRule, entry_raw: f64, turns: &[Turn]) -> Run {
        let mut meter = StrikeMeter::new(rule, entry_raw);
        let mut run = Run::default();
        for (index, turn) in turns.iter().enumerate() {
            run.classes
                .push(class_code(meter.observe(turn.raw, turn.cost)));
            if meter.patience_exhausted() {
                let event = meter.strike();
                run.strikes.push((index, event.strikes, event.improving));
                if event.struck_out {
                    run.struck_out_at = Some(index);
                    break;
                }
            }
        }
        run.min_raw_bits = meter.min_raw().to_bits();
        run.strike_entry_bits = meter.strike_entry_raw().to_bits();
        run
    }

    /// **Reference A** - from docs/economics-round-spec.md's sentences.
    ///
    /// Keeps the classification history and recomputes patience from it on
    /// every turn, which is the definition read literally: "None adds the
    /// batch's all-eight-workers `sample_evaluations`; Substantial resets
    /// accumulated None-work; Marginal adds nothing; strike at the quantum".
    fn spec_reference(rule: StrikeRule, entry_raw: f64, turns: &[Turn]) -> Run {
        let mut run = Run::default();
        let mut min_raw = f64::INFINITY;
        let mut strike_entry_raw = entry_raw;
        let mut strikes = 0u32;
        // The window of turns since the last reset (Substantial or strike),
        // as `(class, cost)`.
        let mut window: Vec<(u8, u64)> = Vec::new();
        for (index, turn) in turns.iter().enumerate() {
            // The 2 % classifier, restated from the spec text: a new minimum
            // below 0.98 x the incumbent is Substantial, any other new minimum
            // is Marginal, anything else is None. The comparison is against
            // the incumbent *before* it moves.
            let class = if turn.raw < min_raw {
                let substantial = turn.raw < 0.98 * min_raw;
                min_raw = turn.raw;
                if substantial {
                    0
                } else {
                    1
                }
            } else {
                2
            };
            run.classes.push(class);
            if class == 0 {
                window.clear();
            } else {
                window.push((class, turn.cost));
            }
            let spent: u64 = match rule.patience {
                // "200 iterations without a 2 % improvement": the None turns
                // in the window, counted.
                Patience::Iterations(_) => {
                    window.iter().filter(|(class, _)| *class == 2).count() as u64
                }
                // "None adds the batch's all-eight-workers sample_evaluations".
                Patience::Work(_) => window
                    .iter()
                    .filter(|(class, _)| *class == 2)
                    .map(|(_, cost)| *cost)
                    .sum(),
            };
            let threshold = match rule.patience {
                Patience::Iterations(limit) => limit,
                Patience::Work(quantum) => quantum,
            };
            if spent >= threshold {
                let improving = min_raw < 0.98 * strike_entry_raw;
                if improving {
                    strikes = 0;
                } else {
                    strikes += 1;
                }
                strike_entry_raw = min_raw;
                window.clear();
                run.strikes.push((index, strikes, improving));
                if strikes >= rule.strikes {
                    run.struck_out_at = Some(index);
                    break;
                }
            }
        }
        run.min_raw_bits = min_raw.to_bits();
        run.strike_entry_bits = strike_entry_raw.to_bits();
        run
    }

    /// **Reference B** - the strike block of `Engine::separate`, transcribed.
    ///
    /// Only meaningful for the control arm; the shipped loop has no work
    /// counter. It is here so the control arm is measured against the closed
    /// member rather than against a description of it.
    fn shipped_inline_reference(limits: SeparateLimits, entry_raw: f64, turns: &[Turn]) -> Run {
        let mut run = Run::default();
        let mut min_raw = f64::INFINITY;
        let mut strike_entry_raw = entry_raw;
        let mut since_improvement = 0u64;
        let mut strikes = 0u32;
        for (index, turn) in turns.iter().enumerate() {
            run.classes.push(class_code(observe_raw(
                turn.raw,
                &mut min_raw,
                &mut since_improvement,
            )));
            if since_improvement >= limits.iterations_without_improvement {
                let improving = min_raw < STRIKE_IMPROVEMENT_RATIO * strike_entry_raw;
                if improving {
                    strikes = 0;
                } else {
                    strikes += 1;
                }
                strike_entry_raw = min_raw;
                since_improvement = 0;
                run.strikes.push((index, strikes, improving));
                if strikes >= limits.strikes {
                    run.struck_out_at = Some(index);
                    break;
                }
            }
        }
        run.min_raw_bits = min_raw.to_bits();
        run.strike_entry_bits = strike_entry_raw.to_bits();
        run
    }

    // ------------------------------------------------------- the generator --
    //
    // `counter_hash` is the member's own stream and there is no `rand::` in
    // this tree by FAST rule. A splitmix step keyed on the vector's index is
    // enough for a property test and it is reproducible without a seed file.

    fn mix(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A Φ walk that looks like a real separation: mostly non-improving, with
    /// occasional marginal and rare substantial minima, and **variable batch
    /// costs** in the range the rerun measured on the shelf (11 203-19 131
    /// evaluations per master iteration).
    ///
    /// `substantial_per_1000` is how often a 2 % improvement lands. The
    /// reference vectors use 20 (an improvement every ~50 turns, which is what
    /// a live separation looks like); the strike-timing vectors use 2, because
    /// a work quantum needs ~110 consecutive non-improvements to fire and a
    /// generator that forgives the counter every 50 turns would leave the
    /// property untested on most seeds.
    fn synthetic_turns(seed: u64, len: usize, substantial_per_1000: u64) -> Vec<Turn> {
        let mut turns = Vec::with_capacity(len);
        let mut best = 1.0f64;
        for index in 0..len {
            let roll = mix(seed ^ (index as u64).wrapping_mul(0x1000_0000_0000_0001));
            let bucket = roll % 1_000;
            let raw = if bucket < substantial_per_1000 {
                // A substantial minimum: strictly below 0.98 x the incumbent.
                best *= 0.90;
                best
            } else if bucket < substantial_per_1000 + 80 {
                // A marginal minimum: a new best inside the 2 % band.
                best *= 0.999;
                best
            } else {
                // Not a minimum.
                best * (1.0 + ((roll >> 8) % 1_000) as f64 / 1_000.0)
            };
            let cost = 11_203 + ((roll >> 20) % (19_131 - 11_203 + 1));
            turns.push(Turn { raw, cost });
        }
        turns
    }

    // --------------------------------------------------------- the vectors --

    #[test]
    fn the_frozen_literals_are_what_the_spec_signed() {
        assert!(frozen_literals_intact());
        assert_eq!(
            StrikeConfig::CONTROL.rule(Phase::Explore),
            StrikeRule {
                patience: Patience::Iterations(200),
                strikes: 3
            }
        );
        assert_eq!(
            StrikeConfig::CONTROL.rule(Phase::Compress),
            StrikeRule {
                patience: Patience::Iterations(100),
                strikes: 5
            }
        );
        assert_eq!(
            StrikeConfig::TREATMENT.rule(Phase::Explore),
            StrikeRule {
                patience: Patience::Work(1_630_000),
                strikes: 3
            }
        );
        assert_eq!(
            StrikeConfig::TREATMENT.rule(Phase::Compress),
            StrikeRule {
                patience: Patience::Work(815_000),
                strikes: 5
            }
        );
    }

    #[test]
    fn control_arm_matches_the_spec_reference_on_1024_vectors() {
        for seed in 0..1_024u64 {
            for rate in [2u64, 20] {
                let turns = synthetic_turns(seed, 700, rate);
                for phase in [Phase::Explore, Phase::Compress] {
                    let rule = StrikeConfig::CONTROL.rule(phase);
                    assert_eq!(
                        drive_meter(rule, 1.0, &turns),
                        spec_reference(rule, 1.0, &turns),
                        "control arm, {phase:?}, seed {seed}, rate {rate}"
                    );
                }
            }
        }
    }

    #[test]
    fn treatment_arm_matches_the_spec_reference_on_1024_vectors() {
        for seed in 0..1_024u64 {
            for rate in [2u64, 20] {
                let turns = synthetic_turns(seed, 700, rate);
                for phase in [Phase::Explore, Phase::Compress] {
                    let rule = StrikeConfig::TREATMENT.rule(phase);
                    assert_eq!(
                        drive_meter(rule, 1.0, &turns),
                        spec_reference(rule, 1.0, &turns),
                        "treatment arm, {phase:?}, seed {seed}, rate {rate}"
                    );
                }
            }
        }
    }

    #[test]
    fn control_arm_is_the_shipped_inline_strike_block() {
        for seed in 0..1_024u64 {
            for rate in [2u64, 20] {
                let turns = synthetic_turns(seed, 700, rate);
                for (phase, limits) in [
                    (Phase::Explore, SeparateLimits::EXPLORE),
                    (Phase::Compress, SeparateLimits::COMPRESS),
                ] {
                    assert_eq!(
                        drive_meter(StrikeConfig::CONTROL.rule(phase), 1.0, &turns),
                        shipped_inline_reference(limits, 1.0, &turns),
                        "control arm vs `Engine::separate`, {phase:?}, seed {seed}, rate {rate}"
                    );
                }
            }
        }
    }

    /// The classifier is shared and no strike touches `min_raw`, so the two
    /// arms must classify **every** turn identically for as long as both are
    /// still running. They disagree about when a strike fires and about
    /// nothing else, which is the spec's "strike semantics are the only delta
    /// between arms" as a property rather than a promise.
    #[test]
    fn the_arms_differ_only_in_strike_timing() {
        let mut ever_differed = false;
        for seed in 0..256u64 {
            let turns = synthetic_turns(seed, 400, 2);
            let control = drive_meter(StrikeConfig::CONTROL.rule(Phase::Explore), 1.0, &turns);
            let treatment = drive_meter(StrikeConfig::TREATMENT.rule(Phase::Explore), 1.0, &turns);
            let shared = control.classes.len().min(treatment.classes.len());
            assert_eq!(
                control.classes[..shared],
                treatment.classes[..shared],
                "seed {seed}: the arms classified differently"
            );
            ever_differed |= control.strikes != treatment.strikes;
        }
        // If the two arms never struck at different turns the vector would be
        // proving nothing, so the test says that it does.
        assert!(
            ever_differed,
            "the two arms struck identically on all 256 vectors"
        );
    }

    /// **The overshoot clause.** At every strike the treatment takes,
    /// `accumulated - crossing_batch` is strictly below the quantum: the meter
    /// crossed the line inside one batch and not two.
    #[test]
    fn overshoot_never_exceeds_one_batch() {
        for seed in 0..512u64 {
            let turns = synthetic_turns(seed, 6_000, 2);
            for (phase, quantum) in [
                (Phase::Explore, EXPLORE_WORK_QUANTUM),
                (Phase::Compress, COMPRESS_WORK_QUANTUM),
            ] {
                let mut meter = StrikeMeter::new(StrikeConfig::TREATMENT.rule(phase), 1.0);
                let mut fired = 0;
                for turn in &turns {
                    meter.observe(turn.raw, turn.cost);
                    if meter.patience_exhausted() {
                        let event = meter.strike();
                        fired += 1;
                        assert!(
                            event.accumulated >= quantum,
                            "seed {seed} {phase:?}: struck below the quantum"
                        );
                        assert!(
                            event.accumulated - event.crossing_batch < quantum,
                            "seed {seed} {phase:?}: overshot by more than one batch \
                             ({} - {} >= {quantum})",
                            event.accumulated,
                            event.crossing_batch
                        );
                        if event.struck_out {
                            break;
                        }
                    }
                }
                assert!(fired > 0, "seed {seed} {phase:?}: the vector never struck");
            }
        }
    }

    /// **The variable-batch-cost vector the spec's FAST union names.**
    ///
    /// A separation that never improves, with batches that cost what the
    /// rerun's stuck 10 s cells cost. The strike must fire at the first batch
    /// whose cumulative cost reaches the quantum, and the batch count must
    /// land where Sol's arithmetic says it does - "roughly 200 batches to
    /// about 85-145" - rather than at a number this module chose.
    #[test]
    fn variable_batch_costs_strike_where_the_arithmetic_says() {
        // The two ends of the measured range and one mixed sequence.
        for cost in [11_203u64, 19_131] {
            let mut meter = StrikeMeter::new(StrikeConfig::TREATMENT.rule(Phase::Explore), 1.0);
            // The entry turn: no batch produced it, so it charges nothing.
            // `min_raw` starts at infinity, so it is always a new minimum.
            assert_eq!(meter.observe(1.0, 0), RawObservation::Substantial);
            let mut batches = 0u64;
            loop {
                // Every reading after the entry is a non-improvement.
                batches += 1;
                meter.observe(2.0 + batches as f64, cost);
                if meter.patience_exhausted() {
                    break;
                }
                assert!(batches < 10_000, "never struck at cost {cost}");
            }
            let expected = EXPLORE_WORK_QUANTUM.div_ceil(cost);
            assert_eq!(batches, expected, "cost {cost}");
            assert!(
                (85..=146).contains(&batches),
                "cost {cost} struck at {batches}, outside the spec's 85-145 reading"
            );
        }

        // Variable costs: the cumulative sum is what decides, not an average.
        let costs = [
            11_203u64, 19_131, 12_000, 18_000, 11_500, 19_000, 13_333, 17_777,
        ];
        let mut meter = StrikeMeter::new(StrikeConfig::TREATMENT.rule(Phase::Explore), 1.0);
        meter.observe(1.0, 0);
        let mut spent = 0u64;
        let mut batches = 0usize;
        loop {
            let cost = costs[batches % costs.len()];
            batches += 1;
            spent += cost;
            meter.observe(2.0 + batches as f64, cost);
            if meter.patience_exhausted() {
                break;
            }
            assert!(
                spent < EXPLORE_WORK_QUANTUM,
                "patience outlived the quantum"
            );
            assert!(batches < 10_000, "never struck");
        }
        assert!(spent >= EXPLORE_WORK_QUANTUM);
        assert!(spent - costs[(batches - 1) % costs.len()] < EXPLORE_WORK_QUANTUM);
        let event = meter.strike();
        assert_eq!(event.accumulated, spent);
        assert_eq!(event.crossing_batch, costs[(batches - 1) % costs.len()]);
        assert_eq!(
            meter.accumulated_none_work(),
            0,
            "the strike cleared the meter"
        );
    }

    /// The control arm is blind to batch cost, by construction: the same Φ
    /// walk under wildly different costs strikes at the same turns.
    #[test]
    fn the_control_arm_ignores_batch_cost() {
        let base = synthetic_turns(7, 900, 20);
        let cheap: Vec<Turn> = base
            .iter()
            .map(|t| Turn {
                raw: t.raw,
                cost: 1,
            })
            .collect();
        let dear: Vec<Turn> = base
            .iter()
            .map(|t| Turn {
                raw: t.raw,
                cost: 10_000_000,
            })
            .collect();
        let rule = StrikeConfig::CONTROL.rule(Phase::Explore);
        assert_eq!(
            drive_meter(rule, 1.0, &cheap),
            drive_meter(rule, 1.0, &dear)
        );
    }

    /// Marginal adds nothing and resets nothing - the middle class is the
    /// whole point of the 2 % rule, and the treatment arm must not quietly
    /// turn it into a reset.
    #[test]
    fn marginal_adds_nothing_and_resets_nothing() {
        let rule = StrikeRule {
            patience: Patience::Work(1_000),
            strikes: 3,
        };
        let mut meter = StrikeMeter::new(rule, 1.0);
        // Entry turn: a new minimum from infinity is Substantial.
        assert_eq!(meter.observe(1.0, 0), RawObservation::Substantial);
        assert_eq!(meter.observe(2.0, 400), RawObservation::None);
        assert_eq!(meter.accumulated_none_work(), 400);
        // Inside the 2 % band: the snapshot moves, the counter does not.
        assert_eq!(meter.observe(0.999, 500), RawObservation::Marginal);
        assert_eq!(meter.accumulated_none_work(), 400);
        assert_eq!(meter.since_improvement(), 1);
        // Below the band: the counter clears.
        assert_eq!(meter.observe(0.5, 500), RawObservation::Substantial);
        assert_eq!(meter.accumulated_none_work(), 0);
        assert_eq!(meter.since_improvement(), 0);
        assert!(!meter.patience_exhausted());
    }

    /// A trickle of marginal minima cannot hold a separation open under either
    /// arm: neither counter advances, so neither strikes - and that is the
    /// shared, frozen behaviour, not a treatment regression.
    #[test]
    fn a_marginal_trickle_holds_both_arms_open_identically() {
        for rule in [
            StrikeConfig::CONTROL.rule(Phase::Explore),
            StrikeConfig::TREATMENT.rule(Phase::Explore),
        ] {
            let mut meter = StrikeMeter::new(rule, 1.0);
            let mut best = 1.0;
            // The entry turn: a new minimum from infinity is Substantial by
            // construction, and it is not part of the trickle.
            assert_eq!(meter.observe(best, 0), RawObservation::Substantial);
            for _ in 0..5_000 {
                best *= 0.9999;
                assert_eq!(meter.observe(best, 19_131), RawObservation::Marginal);
                assert!(!meter.patience_exhausted());
            }
        }
    }

    /// The improving-strike reset, in both arms: a strike whose minimum beat
    /// the previous strike's entry by 2 % does not count against the cap.
    #[test]
    fn the_improving_strike_resets_the_count_in_both_arms() {
        for rule in [
            StrikeRule {
                patience: Patience::Iterations(3),
                strikes: 3,
            },
            StrikeRule {
                patience: Patience::Work(3),
                strikes: 3,
            },
        ] {
            let mut meter = StrikeMeter::new(rule, 1.0);
            // The entry turn pins `min_raw` at the entry Φ.
            assert_eq!(meter.observe(1.0, 0), RawObservation::Substantial);
            // Three non-improvements at cost 1 exhaust either patience.
            for _ in 0..3 {
                assert_eq!(meter.observe(5.0, 1), RawObservation::None);
            }
            assert!(meter.patience_exhausted());
            // `min_raw` is 1.0, which is not below 0.98 x the 1.0 entry.
            let first = meter.strike();
            assert!(!first.improving);
            assert_eq!(first.strikes, 1);
            // Now beat the strike-best (1.0) by well over 2 %.
            assert_eq!(meter.observe(0.5, 1), RawObservation::Substantial);
            for _ in 0..3 {
                assert_eq!(meter.observe(9.0, 1), RawObservation::None);
            }
            assert!(meter.patience_exhausted());
            let second = meter.strike();
            assert!(second.improving, "0.5 < 0.98 * 1.0 is an improving strike");
            assert_eq!(second.strikes, 0);
        }
    }

    /// Struck out is `strikes >= rule.strikes`, and the count is the arm's
    /// own: 3 in explore, 5 in compress, in both arms.
    #[test]
    fn strike_counts_are_3_and_5_in_both_arms() {
        for config in [StrikeConfig::CONTROL, StrikeConfig::TREATMENT] {
            for (phase, expected) in [(Phase::Explore, 3u32), (Phase::Compress, 5)] {
                let rule = config.rule(phase);
                let mut meter = StrikeMeter::new(rule, 1.0);
                // The entry turn pins `min_raw` at 1.0, which is not below
                // `0.98 * 1.0`, so no strike below is an improving one.
                meter.observe(1.0, 0);
                let mut struck = 0;
                // A climbing Φ never improves, so every strike is non-improving.
                let mut turn = 0u64;
                loop {
                    turn += 1;
                    meter.observe(1.0 + turn as f64, 1_000_000);
                    if meter.patience_exhausted() {
                        let event = meter.strike();
                        struck += 1;
                        if event.struck_out {
                            break;
                        }
                    }
                    assert!(
                        turn < 100_000,
                        "{:?} {phase:?} never struck out",
                        config.arm()
                    );
                }
                assert_eq!(struck, expected, "{} {phase:?}", config.arm());
                assert_eq!(meter.strikes(), expected);
            }
        }
    }

    /// A zero-cost batch buys the treatment arm infinite patience, and that is
    /// a **named** property rather than a surprise: a batch that evaluated no
    /// candidate did no work to charge. In the engine it cannot loop - a batch
    /// with an empty colliding set has raw Φ = 0, and the band test ends the
    /// separation above the strike ladder - but a caller that synthesises
    /// batches must know it.
    #[test]
    fn zero_cost_batches_never_strike_the_treatment_arm() {
        let mut meter = StrikeMeter::new(StrikeConfig::TREATMENT.rule(Phase::Explore), 1.0);
        meter.observe(1.0, 0);
        for turn in 0..100_000u64 {
            assert_eq!(meter.observe(2.0 + turn as f64, 0), RawObservation::None);
            assert!(!meter.patience_exhausted());
        }
        // The control arm, on the identical sequence, struck out long ago.
        let mut control = StrikeMeter::new(StrikeConfig::CONTROL.rule(Phase::Explore), 1.0);
        control.observe(1.0, 0);
        let mut struck_out = false;
        for turn in 0..100_000u64 {
            control.observe(2.0 + turn as f64, 0);
            if control.patience_exhausted() && control.strike().struck_out {
                struck_out = true;
                break;
            }
        }
        assert!(struck_out);
    }

    /// The shadow counters reconcile: every turn is classified exactly once,
    /// and the three classes partition the turns.
    #[test]
    fn shadow_counters_partition_the_turns() {
        let turns = synthetic_turns(11, 1_000, 20);
        let mut meter = StrikeMeter::new(StrikeConfig::TREATMENT.rule(Phase::Explore), 1.0);
        let mut offered = 0u64;
        for turn in &turns {
            meter.observe(turn.raw, turn.cost);
            offered += turn.cost;
            if meter.patience_exhausted() && meter.strike().struck_out {
                break;
            }
        }
        let shadow = meter.shadow();
        assert_eq!(
            shadow.batches,
            shadow.substantial + shadow.marginal + shadow.none
        );
        assert!(shadow.charged_work <= offered);
        assert!(meter.accumulated_none_work() <= shadow.charged_work);
    }

    /// Saturation rather than overflow: a caller that hands the meter
    /// `u64::MAX` twice gets a saturated counter and a strike, not a panic in
    /// release and a wrap in debug.
    #[test]
    fn absurd_batch_costs_saturate_instead_of_wrapping() {
        let mut meter = StrikeMeter::new(StrikeConfig::TREATMENT.rule(Phase::Explore), 1.0);
        meter.observe(2.0, u64::MAX);
        meter.observe(3.0, u64::MAX);
        assert_eq!(meter.accumulated_none_work(), u64::MAX);
        assert!(meter.patience_exhausted());
    }
}
