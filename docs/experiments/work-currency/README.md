# A second currency, and the constructor the first one could not see

`docs/experiments/basin-race/` §4.4 ended on a ratio: the work meter prices a
second of mode 34 at **6,628,431** units and a second of mode 20 at **92.7** -
a **71,500x** spread inside one phase of one run - and therefore *"a
work-denominated share ceiling cannot bound it"*. Grok review 3 §3 item 3
predicted it in advance; Sol review 8 §3 condition 4 names the same meter from
the other side. This round builds the fix and measures what it is worth.

The fix is a **parallel** currency. The shipped meter does not move, cannot
move, and is proved not to have moved: every pinned number in this repository
is denominated in it.

---

## The headline

| | |
|---|---|
| **The mispricing, measured** | over **520 operator calls** - 435 already in this repository, 85 new - mode 20 retires **89 units/s** against mode 22's **2,720,973**. The spread is **30,500x** pooled, and the basin-race phase's 71,500x is the same fact at its worst point |
| **Why no exchange rate could fix it** | over 9.246 s of mode-20 draw the profiling array reads **exactly zero on four of its five counters** - no candidate query, no neighbour test, no collision build - and 165 exact pair tests on the fifth. The constructor is not under-counted, it is **invisible** (§1.2) |
| **So the currency asks the operator** | the m34 self-meter pattern, generalised: the class price reads the operator's *own* per-call account. One measured mixed-61 draw - 3.147 s - is priced at **8,173,539** units where the shipped meter reads **310**, and 3.147 s at the reference rate is 8,183,300 |
| **Claim (a), the shipped meter is untouched** | four pinned gates 4/4 on both binaries with **all four whole-document digests identical**; and 9 of 9 coordinator documents identical between the base commit's binary and this tree's - **3,649 leaves, 0 differing** on the largest cell (§3.1) |
| **Claim (a), the observer is pure** | `cur2=2` prices every call and charges none: **9 of 9** documents identical to `cur2=0` once the currency's own block is removed, and 9 of 9 different with it in (§3.1) |
| **Determinism** | work mode, two processes, `cur2=1`: **9 of 9**. Plan mode 7 of 9, and **both misses are plan disagreements**, `calibrated-plan` §7's ladder straddle (§3.2) |
| **Claim (b), does it bound the draws?** | **yes, and by half.** mixed-61 race arm: process wall **14.28 s → 8.14 s** against a ten-second target, race phase **9.11 s → 3.92 s**, draws **2 → 1**. On triangle-20 the race now exits on **`affordability`** 3 times out of 3, a rule basin-race §4.4 called "measurably near-inert" (§4.1) |
| **Claim (b), does it save the race?** | **no, and it makes the loss legible.** Still **0 of 9** basin moves; at equal plan the currency arm is worse, not better, because the draw it used to get free now costs a third of the plan. Pricing was not the whole story - §4.3's criteria landslide is the rest, and the currency has ruled the pricing out rather than fixed it. **The race stays off** |
| **Claim (c), the counter tax** | **not recovered, and not made worse.** The currency's own instrumentation costs a median **0.000 mm** and is identical to the digit on 2 of 3 seeds. §6 names precisely what would recover the 1.882 mm and why this design cannot |
| **The canonical instrument** | plan mode, mixed-61, three seeds, three rounds: **9 of 9 equal-work, 9 of 9 tied at exactly 0.0000 mm**. The v3 queue buys **no** mode-20 action there, so the currency charges nothing and changes nothing (§5) |
| **But where the shipped queue *does* draw** | one diversify ticket on shapes-17, race off: **0.787 s, 8 exact pair tests and nothing else in the profiling array, 40 shipped units, 1,627,800 class units** - a **40,695x** repricing of an ordinary action of a shipped class (§5.1) |
| **A bug this round shipped and caught** | the first cut hardcoded the shipped meter's exact-pair coefficient as `43`; it is `5`. Every class over-charged itself 38x per pair test and **the unit test passed**, because it retyped the same literal. What caught it was `chargedExtraUnits` being non-zero on a mode-22 call in a run with no mode-20 calls in it (§2.4) |
| **Everything ships off** | `cur2=0` is the default and is byte-identical to a binary without the field |
| **Suites** | both pass, exits captured directly (§7) |

**Recommendation.** Keep the currency, keep it **off**, and keep the observing
mode armed in any future battery that reports operator costs - §1's table cost
one afternoon and could have been produced at any point in the last six rounds.

Do **not** arm `cur2=1` in the shipping default, and the reason is a gap in
the evidence rather than a result. On the canonical instrument it is a measured
no-op, because mixed-61's queue draws nothing (§5); the two configurations
where it *does* charge are the race, which is off and stays off (§4), and the
diversify class on shapes-17, which is saturated at 200.349 mm so the charge
has no consequence to measure (§5.1). **This round has not found a request
where the repriced class both fires and has room**, so the depth effect of
arming it is unmeasured, not zero, and that is the battery the next round
should run before the default moves.

What the round *establishes* is §8: the affordability rule, the share ceilings
and the class priors can now see a constructor. That is the precondition for
every scheduler question the campaign has parked on the grounds that mode 20 is
unpriceable, and §4.1 is the first time one of them has been answered.

---

# Part I — the measurement

## 1. The mispricing table

### 1.1 The rates

Two sources, and the first is free. `drivers/rates.py` harvests every
`portfolio.operatorCalls` row this repository already contains -
`basin-race/`, `replan/` and `calibrated-plan/` evidence, **435 calls** - and
then runs five new cells at `cur2=2`, the observing mode, which prices every
call and charges none of it (**85 calls**). The corpus carries wall and shipped
units; only the new runs carry the count vectors §2 is fitted on.

`evidence/rates.json`. The corpus, pooled:

| operator | calls | wall (s) | shipped units | units/s |
|---|---:|---:|---:|---:|
| mode 22 | 227 | 190.59 | 486,676,460 | **2,553,476** |
| mode 34 | 117 | 146.96 | 252,965,881 | 1,721,303 |
| mode 23 | 38 | 37.30 | 134,572,158 | 3,607,539 |
| **mode 20** | 35 | 53.50 | **4,540** | **84.9** |
| mode 26 | 9 | 15.74 | 11,193,888 | 711,031 |
| mode 31 | 9 | 0.01 | 1,485 | 141,773 |

This session's own runs, on one binary in one window:

| operator | calls | wall (s) | shipped units | units/s |
|---|---:|---:|---:|---:|
| mode 22 | 35 | 27.154 | 73,884,014 | **2,720,973** |
| mode 34 | 20 | 18.242 | 20,290,831 | 1,112,302 |
| mode 23 | 19 | 14.105 | 29,731,458 | 2,107,940 |
| **mode 20** | 7 | 9.246 | **825** | **89.2** |
| mode 26 | 2 | 3.684 | 3,200,229 | 868,798 |
| mode 31 | 2 | 0.002 | 770 | - |

The two agree on mode 20 to about **5%** (84.9 against 89.2) across two
sessions and two disjoint sets of runs, and they agree on the ordering of every
other class. **The mispricing is 30,100x in the corpus and 30,500x in this
session**; basin-race's 71,500x is the same fact measured inside the one phase
where mode 34 is at its densest, and the two numbers are consistent because
mode 34's rate is what moves between them, not mode 20's.

Mode 31 is two calls totalling **2 milliseconds** and is not priced by anything
here; it is in the table because leaving it out would be a choice.

### 1.2 The finding: the constructor is invisible, not under-counted

This is the part that decides the design, and it was not knowable from the
rates alone. Here is what the **whole** profiling counter array recorded across
those seven mode-20 calls - 9.246 seconds of wall:

| counter | mode 20 | mode 22 (27.154 s) |
|---|---:|---:|
| `CandidateQueries` | **0** | 73,710,994 |
| `NeighborTests` | **0** | 221,982,731 |
| `CollisionPolygonBuilds` | **0** | 0 |
| `FullRescores` | **0** | 40,901 |
| `ExactPairTests` | **165** | 34,604 |

Zero. Not "small": zero, on four of five counters, over nine seconds. The
layered construction scores through its own position-source pipeline and never
enters the relaxed lane's `score_placement`, which is where
`Counter::CandidateQueries` is incremented.

**No weighting of the profiling array can price mode 20, at any exchange
rate.** A currency built as "the same counters with better coefficients" - the
obvious design, and the one this round started with - is arithmetically
incapable of the job.

`CollisionPolygonBuilds` is worth a separate line, because it is the counter
that *should* have caught this and is zero on **both** columns. The engine has
two increment sites. The persistent-vacancy pipeline's is
`profiling::deep::count`, which is compiled out without the `search-profiling`
feature and no shipped binary carries it; the other is in `general_fast`'s
short-side-first construction and is not gated, but no operator call this round
measured reached it. (A whole direct-mode run does: the g1 gate's process-wide
reading is 2,913.) The count that carries mode 20's price is therefore the
operator's **own** copy of the same quantity - which is why the currency reads
`work.experimental_collision_builds + work.validator_collision_builds` and not
the array.

The same call's **own** account, from
`GeneralPersistentVacancyWorkDiagnostics`, in the same 9.246 seconds:

| the operator's own count | mode 20 |
|---|---:|
| `position_source_attempts` | 3,165,372 |
| `pair_visits` | 3,813,288 |
| `returned_positions` | 471,936 |
| `operator_collision_builds` | **309,935** |

So the currency is a generalisation of the **compression schedule's self-meter**
and not of the coordinator's counter. `CompressionSchedule::work_units` already
derives its exact half from its own confirmation count rather than sampling the
global array, and for the same reason: the operator knows what it did. This
round extends that from one class to a table.

---

## 2. The currency

### 2.1 What a price is

`crates/polygon-nesting-core/src/search/work_currency.rs`. Two halves, and the
split is the design:

* **the counts are deterministic.** Five profiling counters, read as a delta
  across one operator call, plus four of the operator's own per-call counts and
  the schedule's confirmations. Two processes running the same work-budgeted
  arm compute the same vector - §3.2 is the gate.
* **the weights are a machine profile.** Nanoseconds per count, per class,
  fitted on this box. They are facts about the hardware, not about the search,
  which is why they are a named constant with a driver that refits them.

The arithmetic is integer throughout - weights scaled by
`WORK_CURRENCY_SCALE = 1_000`, one division at the end - because the debit
feeds `BudgetMeter::self_metered_debit`, which is `u64` for the reason Sol
review 6 §1 gave.

### 2.2 The unit, and why `max` is safe

`WORK_CURRENCY_REFERENCE_RATE = 2_600_000` shipped-meter units per second. A
class's self-price is *the units the reference class would have retired in this
call's wall*, so the two currencies are commensurable and the coordinator can
settle at

```
charge = max(global_delta, operator_self_units, class_self_units)
```

which is the existing m34 transaction with a third arm. `max` can only ever
**raise** a price, so a repricing cannot manufacture budget a run did not have.
For that to be sound the reference rate has to sit at or above the rate the
shipped meter already charges the busiest class at; 2.6 M/s is above mode 22's
pooled 2.55 M and this session's 2.72 M, and mode 23's 3.6 M corpus rate is
*deliberately* left above it - `max` keeps mode 23's global delta, which is the
higher price, and a currency whose job is to stop under-pricing must not become
a discount.

### 2.3 The fit, and its honest width

`drivers/fitprofile.py`, `evidence/profile.json`. One count per class, not a
ten-parameter regression: the sample is small and a class's counts move
together within a request, so a ten-parameter fit on seven mode-20 calls across
three fixtures fits the request table, not the box. Candidates are ranked by
the **residual** - charge against `wall * REFERENCE_RATE`, geometric RMS of the
ratio - which is not the statistic the weight minimises (that is a median of
rates), so the ranking is a check rather than a restatement.

Only classes the shipped meter under-prices by more than 3x are fitted at all:

| class | shipped units/s | x reference rate | verdict | fitted count | scaled weight |
|---|---:|---:|---|---|---:|
| **mode 20** | 89 | **3.43e-05** | **under-priced** | `operatorCollisionBuilds` | **82,605** |
| mode 22 | 2,720,973 | 1.047 | comparable | - | - |
| mode 23 | 2,107,940 | 0.811 | comparable | - | - |
| mode 26 | 868,798 | 0.334 | comparable | - | - |
| mode 34 | 1,112,302 | 0.428 | comparable | - | - |
| mode 31 | - | - | 2 calls, 2 ms | - | - |

Nothing sits near the threshold: the nearest class to mode 20 is **four orders
of magnitude** away (mode 26 at 0.334 against 3.43e-05), so the `3.0` is not a
borderline call on this evidence. Mode 34 is left alone for a second reason -
it already carries its own meter, which reads about 11x the global counter on
the measured band, so `max` was already over-charging it.

The five candidates for mode 20:

| count | residual RMS | worst under | worst over | scaled weight |
|---|---:|---:|---:|---:|
| **`operatorCollisionBuilds`** | **1.703** | 0.793 | 2.631 | **82,605** |
| `returnedPositions` | 1.811 | 0.930 | 3.107 | 55,366 |
| `positionSourceAttempts` | 1.834 | 0.409 | 1.421 | 6,667 |
| `exactPairTests` | 1.889 | 0.445 | 2.508 | 149,171,712 |
| `pairVisits` | 2.051 | 0.475 | 2.927 | 13,044 |

(The two tables above are `evidence/profile.json`, which is the **re-fit**;
the weight the code ships and the batteries ran with is 82,506, and the last
paragraph of this section is why the two differ by 0.12%.)

and the residuals of the **shipped** weight,
`WORK_CURRENCY_M20_COLLISION_BUILD_WEIGHT = 82,506`, on the same seven calls:

| request | wall (s) | collision builds | shipped meter | target | class price | ratio |
|---|---:|---:|---:|---:|---:|---:|
| mixed-61 | 3.147 | 99,066 | 310 | 8,183,300 | 8,173,539 | **0.999** |
| mixed-61 | 3.156 | 101,743 | 275 | 8,204,444 | 8,394,407 | 1.023 |
| shapes-17 | 0.777 | 19,390 | 40 | 2,019,767 | 1,599,791 | 0.792 |
| shapes-17 | 0.773 | 19,906 | 30 | 2,010,442 | 1,642,364 | 0.817 |
| shapes-17 | 0.783 | 19,729 | 40 | 2,034,985 | 1,627,760 | 0.800 |
| triangle-20 | 0.297 | 24,630 | 65 | 773,268 | 2,032,122 | 2.628 |
| triangle-20 | 0.313 | 25,471 | 65 | 812,581 | 2,101,510 | 2.586 |

**This is a wide fit and the document will not pretend otherwise.** A factor of
**3.32** separates the best and worst cell. Two things put it in proportion,
and both are measured on the same seven calls:

* the **shipped meter's own** spread on this class, over these calls, is a
  factor of **5.63** - 38.8 units per second on one shapes-17 draw against
  218.6 on a triangle-20 one. So the currency is *tighter* than the instrument
  it replaces on this class, at 1/29,000 of the level;
* the spread is **between** fixtures, not within one: mixed-61's two calls
  agree to 2.4% and shapes-17's three to 3.2%.

The currency does not make a draw's price exact. It makes it the right order of
magnitude, which is the difference between a ceiling that binds and one that
does not.

The constant ships at the value the batteries below ran with. A re-fit on the
final binary hours later returned **82,605** - a **0.12%** difference, two
orders of magnitude inside the residual - and that is this round's only
statement about how reproducible the fit is. `evidence/profile.json` carries
the re-fit, so the table above and the JSON's `scaledWeight` differ by exactly
that 0.12% and the discrepancy is this paragraph rather than an error.

### 2.4 The bug this round shipped and caught

The first cut of `work_currency.rs` wrote the shipped meter's exact-pair
coefficient as `43_000` - from memory. It is **5**
(`WORK_UNITS_PER_EXACT_PAIR_TEST = 5`). So every class self-priced at 38x the
shipped meter per exact pair test, `max` charged the difference on **every
operator in every run**, and the currency's first race and plan batteries were
measuring a repricing of the whole engine rather than of the constructor.

**The unit test passed.** It computed its own expectation with the same wrong
literal - the failure mode Sol review 8 §1 names about a different test,
*"ricopia la conclusione"*.

What caught it was the evidence, not a test: `chargedExtraUnits` was non-zero
on a **mode-22** call, in a run whose `drawCalls` was **zero**. That is only
visible because the per-call block reports the currency's own contribution
separately from the settled total, which is the same argument `OperatorCharge`
makes for reporting four numbers instead of one.

Two things changed. `SHIPPED_EXACT_PAIR_TEST` is now *derived* from
`portfolio::WORK_UNITS_PER_EXACT_PAIR_TEST` in a `const`, so the two cannot
drift; and the regression
`an_unnamed_class_self_prices_at_the_live_global_meters_own_reading` reads the
**live** counters through the two functions that actually run in
`run_operator` and asserts they agree, which a test inside `work_currency`
could not do. Every battery below was re-run from scratch on the fixed binary;
nothing in this document is a number from before the fix.

---

# Part II — what it proves

## 3. Claim (a): the shipped meter did not move

### 3.1 Two equivalences

**The four pinned gates.** `drivers/gates.py`, `evidence/gates-base.json`,
`evidence/gates-ship.json`. The gate binary is `--features jagua-experimental`.

| gate | pinned | base | ship | whole-document digest |
|---|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | hit, 26.47 s | hit, 26.83 s | `1cc2ffd33ec2d399` **identical** |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | hit, 3.15 s | hit, 3.40 s | `8bbd9f7323f3adfa` **identical** |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | hit, 3.43 s | hit, 3.69 s | `43bf1bafed647499` **identical** |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | hit, 3.07 s | hit, 3.29 s | `8d50e39542eafd0e` **identical** |

`ALL_PASS: true` on both, and the digest is the whole document with the
wall-clock and provenance fields stripped - a much stronger statement than four
scalars reproducing. Note that g1 is a **mode-20** gate: the one class this
round reprices is also the one whose pinned regression is a direct-mode run,
and it reproduces byte for byte because a direct-mode run has no coordinator,
no operator call and therefore no currency.

**The coordinator documents.** `drivers/equiv.py`, `evidence/equivalence.json`.
Nine cells at `work=40000000`, the bare request, v3 on. The base binary is run
with **no `cur2` key at all** - an unarmed binary refuses a key it cannot
honour, which is the campaign's own rule for `fcv`, `crot` and `m34pconfirm` -
so this proves the stronger statement: *a spec without the key on the old
binary and a spec with the key at `0` on the new one are the same run.*

| cell | base == ship | observe is pure | leaves | differing | clock-only |
|---|---|---|---:|---:|---:|
| mixed-61 s0 | **yes** | **yes** | 3,649 | **0** | 61 |
| mixed-61 s1 | **yes** | **yes** | 2,991 | **0** | 46 |
| mixed-61 s2 | **yes** | **yes** | 3,828 | **0** | 64 |
| shapes-17 s0 | **yes** | **yes** | 2,268 | **0** | 93 |
| shapes-17 s1 | **yes** | **yes** | 1,934 | **0** | 64 |
| shapes-17 s2 | **yes** | **yes** | 2,196 | **0** | 87 |
| triangle-20 s0 | **yes** | **yes** | 1,999 | **0** | 30 |
| triangle-20 s1 | **yes** | **yes** | 2,177 | **0** | 30 |
| triangle-20 s2 | **yes** | **yes** | 2,115 | **0** | 36 |

Two columns need explaining, and the explanation is a limitation this round
found in the inherited instrument rather than a convenience.

`gatelib.py`'s `doc_digest` was written for the four gates, and **a gate is a
direct-mode run with no `portfolio` block at all.** A coordinator document
carries a dozen more clock readings - `startedSeconds`, `birthSeconds`,
`occupancyOverTime`, the schedule's per-action `seconds`. Used unchanged, the
digest disagreed on all nine cells. `runlib.leaf_diff` is what found them and
it is reported per cell for exactly that reason: **a digest that matches proves
nothing about what was stripped to make it match.** The "clock-only" column is
how many leaves differ before the wall-derived set is removed; the "differing"
column is how many differ after. Sixty-one, then zero. Not one work unit,
depth, fingerprint, counter or archive disposition is in the sixty-one, and
`WALL_DERIVED` deliberately excludes `estimatedCost` and `actualCost`, which
under a work budget are the work numbers the comparison is about.

The **observe** column is the second arm: `cur2=2` against `cur2=0` on the same
binary. It is `yes` when the two documents are identical *with the currency's
own block removed* - 9 of 9 - and the driver separately checks that they are
different *with it in*, also 9 of 9, so the arm cannot pass by reporting
nothing.

### 3.2 Determinism across two processes

`drivers/determinism.py`. Same binary, same spec, two processes.

| arm | budget | cells | equal | plans agree |
|---|---|---:|---:|---|
| `cur2=1` | `work=40000000` | 9 | **9/9** | 9/9 |
| `cur2=1` | `plan=10000` | 9 | 7/9 | 7/9 |

**The work-mode row is the gate this round's code is responsible for.** A work
budget is a function of counters; the currency's counts are counters, its
weights are constants and its arithmetic is integer, so a `cur2=1` run must be
as reproducible as a `cur2=0` one. It is, on all nine.

Plan mode's two misses - mixed-61 s2 and triangle-20 s1 - are **both**
`plansAgree = false`: the two processes read different clocks in phase 0,
straddled a ladder rung and bought different budgets, which is
`calibrated-plan` §7's predicted and only failure mode and is what
`basin-race` §8 and `replan` §13.1 both measured on the same box. Not one is a
document disagreement at an agreed plan. mixed-61 s2 is worth a line on its
own: the two processes bought 12,801,193 and 8,531,793 class-units of work and
published **176.16200000000003** either way.

`evidence/determinism-work-cur2.json`,
`evidence/determinism-plan-cur2.json`.

### 3.3 The one join that has to be argued, and was re-run instead

Suite 1 caught a defect **in this round's own test**, after the wall-sensitive
batteries had run: `an_unnamed_class_self_prices_...` called
`profiling::set_enabled(true)`, which is process-global, and `cargo test` runs
this module's tests in parallel threads of one process. It broke **three
sibling tests** that legitimately assume an unarmed meter reads zero
(`the_budget_currency_is_the_budgets_own` and the two self-metered debit
tests). The fix splits `work_units_now` and `work_currency_counts_now` into a
live half and a **snapshot** half, and the test drives the snapshot half over
four synthetic counter arrays - which is a better test as well as a
non-invasive one, because it can vary the counters the price must *ignore*.

That fix changed the binary after §4, §5 and §6 were measured, as did two
later edits: `WorkCurrencyMode::armed` became `pub` so the example can gate its
run-level block on the *arm* rather than on whether any call carried one, and
one unit test was renamed and given an exact assertion instead of a band. The
campaign's rule for this situation is `docs/experiments/replan/` §8.1: name the
delta, and **re-run the gates that can be affected rather than argue about
them.**

* named: `work_units_from(&totals)` contains the same two array reads and the
  same `saturating_mul`/`saturating_add` the inline version had; the
  visibility change adds no code path; the example's gate differs from the old
  one only for an armed run that dispatched **no** operator call, which none of
  §4-§6's runs is;
* re-run: the four pinned gates and the nine-cell equivalence battery above
  are on the **final** binary, not the battery one;
* and measured rather than argued: `drivers/binequiv.py`,
  `evidence/binequiv-cur2.json`, runs the battery binary
  `c63e5af0b92747f1` and the final binary `1c63cd404fd23472` side by side on
  nine cells at `work=40000000` with **`cur2=1`** - the armed arm the batteries
  were taken on, not merely the off one - and the documents are identical on
  **9 of 9**: same depth, same work units, same class units, same digest.

---

## 4. Claim (b): the race, re-run

`drivers/racebattery.py`, `evidence/racebattery-10000.json`. Three arms per
cell, one binary, order rotated per cell, `plan=10000`, v3 on, `0.002`
allowance, the bare request:

```
off    race=0                the un-raced run
on     race=3:1:3            the race, priced by the shipped meter
on2    race=3:1:3,cur2=1     the race, priced by the parallel currency
```

Load 1 min 6.42 / median 7.39 / max 8.14 over 27 runs.

### 4.1 It bounds the draws, and by half

This is the claim basin-race §4.4 said a work-denominated ceiling
**structurally could not** make:

| fixture | process wall, off / on / **on2** | race phase s, on → **on2** | draws, on → **on2** | race exit, on → **on2** |
|---|---|---|---|---|
| mixed-61 | 7.57 / 14.28 / **8.14** | 9.11 → **3.92** | 2.00 → **1.00** | deadline → deadline |
| shapes-17 | 8.25 / 9.84 / **6.04** | 4.24 → **1.59** | 3.00 → **2.00** | deadline → deadline |
| triangle-20 | 7.17 / 10.55 / **9.04** | 5.14 → **2.25** | 2.00 → **1.00** | deadline → **affordability** |

Three readings, in order of how much they say:

* **mixed-61's race arm re-enters its wall target.** 13.89 / 13.81 / 15.14 s
  becomes 8.14 / 7.93 / 8.36 s against a ten-second budget. basin-race §4.4
  recorded 13.5-17.1 s and named the meter as the reason; the meter was the
  reason.
* **the draws are what shrank.** Two mode-20 draws per race become one on
  mixed-61 and on triangle-20, three become two on shapes-17. The affordability
  check in `run_basin_race` - which basin-race §4.4 called *"measurably
  near-inert"* and which says so in a comment at its own call site - now
  refuses the second arm, and on triangle-20 it becomes the race's **exit
  cause** on 3 cells of 3. It is `deadline` on 9 of 9 in this round's shipped-
  meter arm and on 9 of 9 in
  `basin-race/evidence/racebattery-draw-10s.json`, so this is the first time
  in the campaign that rule has ever stopped anything.
* **the price the check reads.** On mixed-61 the `on` arm's **two** draws are
  charged **585 / 550 / 640** units in total for 6.35 / 6.34 / 6.24 seconds,
  and the `on2` arm's **one** draw is charged **8,173,849 / 7,830,764 /
  8,288,047** for 3.19 / 3.20 / 3.20 seconds. Per draw and against a plan of
  **24,891,457** units, that is **0.0012% of the plan becoming 33%** for the
  same three seconds of the same operator. The counts are not different; the
  price is.

### 4.2 It does not save the race, and that is the finding

| arm | equal-work cells | median Δ | better / worse / tied | moved off incumbent |
|---|---:|---:|---|---:|
| `on` (shipped meter) | 8 of 9 | 0.0000 | 3 / 3 / 2 | **0 of 9** |
| `on2` (currency) | 7 of 9 | +0.0004 | 1 / 5 / 1 | **0 of 9** |

Per cell, at equal plan (all three mixed-61 cells are equal-work in both arms,
which is a stronger row than basin-race got):

| cell | plan units | off | on | on2 | on−off | on2−off |
|---|---:|---:|---:|---:|---:|---:|
| mixed-61 s0 | 24,891,457 | 175.3878 | 179.6330 | 179.5869 | +4.2452 | **+4.1991** |
| mixed-61 s1 | 24,891,457 | 174.1700 | 176.5363 | 179.6330 | +2.3663 | **+5.4630** |
| mixed-61 s2 | 24,891,457 | 176.1620 | 175.5060 | 179.0060 | −0.6560 | **+2.8440** |

The brief's hypothesis was that *"the +2.4/+2.9 mm should shrink toward parity
if the pricing was the whole story"*. It does not shrink; on two of three cells
it grows. **The pricing was not the whole story, and this is how a currency
rules a hypothesis out rather than confirming it.**

The mechanism is not subtle and the numbers above contain it. The race's price
did not go up - it was always this expensive - but under the shipped meter it
was paid in **wall**, which a plan-mode arm overruns silently, and under the
currency it is paid in **budget**, which comes straight out of the queue. Eight
million units of a 24.9 M plan is a third of the run, and §4.3's verdict is
that the race buys nothing with it: **0 of 9 cells moved off the incumbent** in
either arm, exactly as basin-race's 0 of 21.

So: the currency converts an invisible wall overrun into a visible budget
charge. That is the correct behaviour and it makes the race's cost legible for
the first time. It is not an argument for arming the race, and this round
repeats basin-race's recommendation without qualification - **the race stays
off**, and what would have to change is §4.3's criteria, which no currency
touches.

---

## 5. The canonical instrument: plan mode, race off

`drivers/planbattery.py`, `evidence/planbattery-10s.json`. mixed-61, three
seeds, three rounds, `plan=10000`, arms interleaved, `cur2=0` against `cur2=1`
and nothing else different. Load 1 min 5.32 / median 7.83 / max 9.19 over 18
runs.

| seed | plan units, all 3 rounds | `cur2=0` | `cur2=1` | Δ | equal work | draws off/on | charged extra |
|---|---:|---:|---:|---:|---|---:|---:|
| 0 | 24,891,457 | 175.3877782649107 | 175.3877782649107 | **0.0000** | 3/3 | 0 / 0 | **0** |
| 1 | 24,891,457 | 174.17000000000002 | 174.17000000000002 | **0.0000** | 3/3 | 0 / 0 | **0** |
| 2 | 24,891,457 | 176.16200000000003 | 176.16200000000003 | **0.0000** | 3/3 | 0 / 0 | **0** |

**9 of 9 equal-work, 9 of 9 tied at exactly 0.0000 mm** - every one of the
eighteen runs landed on the same rung, and every arm reproduced its own seed's
depth to seventeen digits in all three rounds. The three depths are
`calibrated-plan` §9's own plan-mode numbers (175.388 / 174.170 / 176.162) to
the digit. Process wall is within **0.26 s** per pair, in both directions.

The reason is in the last two columns and it is the honest headline of this
section: **on mixed-61 at ten seconds the v3 queue dispatches no mode-20 action
at all**, so the one class the currency reprices never runs, `chargedExtraUnits`
is zero, and the currency is exactly a no-op. That is the necessary condition
for arming it - a repricing that moved the canonical number would have to be
argued for - and it is *not* evidence that the currency does anything useful
there.

### 5.1 Where the shipped queue *does* draw, and what it was paying

Across §3.2's eighteen work-budgeted runs the diversify class fires on exactly
one fixture, and there the currency is not free. The whole of one such call,
from `evidence/determinism-work-cur2.json`, shapes-17 seed 0 - **a diversify
ticket in the shipped v3 queue, with the race off**:

| | |
|---|---:|
| action | `m20 ticket slot0 + m22 quantum` |
| phase | `diversify` |
| wall | **0.787 s** |
| profiling counters | 8 exact pair tests, and nothing else |
| its own counts | 124,884 position-source attempts, 74,091 pair visits, **19,729 collision builds** |
| **shipped meter** | **40 units** |
| **class price** | **1,627,800 units** |

**40,695x**, on a real action of a shipped class on a bare request. Nothing
about it is the race. The charge is **1,627,760** extra units - **8.6%** of
that run's 18.9 M - on all three shapes-17 seeds, and zero on mixed-61 and
triangle-20, where the queue drew nothing.

shapes-17 is saturated at 200.349 mm in every arm this campaign has run, so
the charge moved no depth and §3.2's nine documents are equal with it in. What
it establishes is the scale: an action the budget was pricing at forty units
costs one and a half million, and the queue has been buying it on that basis
for the whole of coordinator v3.

---

## 6. Claim (c): the counter tax

`drivers/countertax.py`, `evidence/countertax.json`. `calibrated-plan` §9
measured the profiling counters at **+1.882 mm** on mixed-61 at a ten-second
wall and called it a floor under any work-denominated budget. Sol review 8 §3
condition 4 asks for the *"debit lane-local economico"* that would remove it.
Three arms at `wall=10000`, three seeds, three rounds, arm order rotated:

| seed | counters off | counters on | counters on + `cur2=1` | counter tax | **currency tax** |
|---|---:|---:|---:|---:|---:|
| 0 | 168.7560 | 176.3094 | 176.3094 | +7.5534 | **0.0000** |
| 1 | 165.6558 | 176.0560 | 176.0560 | +10.4002 | **0.0000** |
| 2 | 174.2800 | 178.2857 | 176.1620 | +4.0057 | −2.1237 |
| | | | | median +7.553 | **median 0.000** |

**The currency does not recover any of the 1.882 mm, and it does not add to
it.** On seeds 0 and 1 the `cur2=1` arm is identical to the `cur2=0` arm to
seventeen digits across all three rounds; seed 2's −2.12 is the wall arm's own
run-to-run spread, which `calibrated-plan` §11.1 measured at eight distinct
depths on one seed. That is the expected result: under a **wall** budget
`debit_self_metered` returns zero by construction, so `cur2` cannot reprice
anything, and its whole cost is two extra `counter_totals()` calls per operator
call - a few dozen registry reads in a run that makes four to twelve operator
calls. It is measured here rather than assumed, which is the point of running
the third arm at all.

The counter tax itself is **not** reproduced at 1.882 mm: the median here is
7.553 mm. Two honest statements about that, and the second is the important
one. The sign and the direction reproduce on all three seeds. The magnitude
does not, because this box was not quiet (§9) and the counters-off arm is a
wall-budget arm, which is the least reproducible configuration this campaign
has - `calibrated-plan`'s own per-seed numbers were 2.700 / 1.527 / 1.882 and
these are 7.553 / 10.400 / 4.006 on the same three seeds. Seed 2 lands at
**exactly +1.8820** when the two-round pass is read instead of the three-round
one, which is a coincidence of two quantised attractors rather than a
reproduction, and it is recorded here so a reader is not tempted by it.

**Why the currency structurally cannot recover it, and what would.** Every
class but mode 20 is priced from the profiling array, so turning the array off
would make the budget read zero for the whole queue. The recovery would need
the *operator's own* counts to cover the classes that carry the spend, and the
mechanism for that already exists one level down:
`CompressionSchedule::work_spent` reads the lane's **own**
`surrogate_evaluations` rather than `Counter::CandidateQueries`, and its own
comment says the two are the same number incremented on the same line, with the
profiling one additionally gated on a process-global recording flag. Lifting
that counter out of the compression schedule and onto the relaxed lane, so mode
22 and mode 23 can self-report the way mode 34 already does, is what would let
a work budget run with `profiling::set_enabled(false)` and take the 1.882 mm
back. It is a well-defined next spend, it is not this one, and this round
claims **none** of that millimetre.

> **Spent, 2026-08-22, and the diagnosis above is half right.**
> `docs/experiments/consolidation/` §2 took the millimetre and found that the
> lift this paragraph specifies was **not what was needed**, for a reason this
> round could not see: the meter's exact half - `5 x ExactPairTests` - is
> **27%** of it (2.93 M of 10.79 M units on a measured mixed-61 plan run), and
> that half is counted in `kernel::exact`, which has no lane. A lane-local
> candidate-query counter alone would have under-charged a work budget by a
> quarter.
>
> What the flag turned out to be worth is the whole of it. The counters were
> never the cost - `meterOnly` is identical to `countersOff` on all three seeds
> in that round's re-measurement of `calibrated-plan` §9 - the **spans** were,
> and one flag armed both. `profiling::metering_enabled` is the second flag;
> the two counters the budget reads move onto it, and the budget is
> numerically unchanged because they are the same counters at the same sites.
>
> Measured: **84.9%** of the seconds for the same work at 24.9 M units,
> **82.5%** at 120 M, with the whole document identical on 9 of 9 cells.

---

# Part III — the protocol

## 7. Binaries and suites

`evidence/binaries.txt`. The combo is
`jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator`.

| label | features | sha256 (16) | what it measured |
|---|---|---|---|
| `base-gate` | `jagua-experimental`, base commit `8e7f82e` | `7a6487613f5de94f` | the four gates' left-hand side |
| `base-combo` | the combo, base commit `8e7f82e` | `10e2d46835466aed` | §3.1's equivalence, left-hand side |
| `battery-combo` | the combo, this tree before §3.3's fixes | `c63e5af0b92747f1` | §4, §5, §6 |
| `ship-gate` | `jagua-experimental`, this tree | `aabb285fe4a9e957` | §3.1's four gates |
| `ship-combo` | the combo, this tree | `1c63cd404fd23472` | §3.1, §3.2, §3.3, §1's observing runs |

`battery-combo` and `ship-combo` differ by §3.3's fix and are shown to produce
identical documents on nine cells of the armed arm; the wall-sensitive
batteries are attributed to the binary that actually ran them rather than
rounded into one.

No `se2-rigidity-certificate` build. Nothing this round touches
certificate-gated code: `work_currency` is behind `jagua-experimental` and the
settlement it feeds is in `portfolio`. Said rather than omitted.

### 7.1 The suites

`drivers/run-suites.sh`, exit status read on the line after the redirect rather
than through a pipe, because `cargo test ... | tee log` reports `tee`'s status
and that is how a red suite gets written up as green.

| suite | features | exit | tests |
|---|---|---:|---|
| `suite-jagua` | `jagua-experimental` | **0** | **1,288 passed**, 0 failed, 2 ignored, over 55 test binaries |
| `suite-combo` | the protocol's full combo | **0** | **1,348 passed**, 0 failed, 2 ignored, over 55 test binaries |

`EXITS jagua=0 combo=0`. Both passed on the first attempt of this run,
including the campaign's known flake
(`free_material_multi_eviction_shrinks_retained_container_capacity`), which did
not need a rerun in either suite. Logs: `evidence/suite-jagua.log`,
`evidence/suite-combo.log`.

The counts are **+13** and **+19** against `replan` §13.2's 1,275 and 1,329,
and the split is this round's twelve new tests plus one the round did not add
(the two suites' binary counts moved with the workspace, not with this diff).
All twelve appear in both suites - eight in `search::work_currency` and four in
`search::portfolio` - because `work_currency` is behind `jagua-experimental`,
which the gate binary's feature set already carries. **No test in this round is
unreachable from a suite the protocol names**, which is the failure
`basin-race` §9 had to write up and this one does not.

An earlier attempt at these suites is the reason §3.3 exists: it ran red at
874 passed / 4 failed, and three of the four failures were **pre-existing
tests** that this round's own test had broken by flipping a process-global
flag. The result above is the fixed tree, run alone, after every battery. The
only edits made to a `.rs` file after this run started are rustdoc comments in
`work_currency.rs`; no test, no signature and no expression changed.

## 8. What the currency is for

Not the race. The race is off and stays off. What §4.1 actually demonstrates is
a capability the coordinator did not have:

* **the affordability rule can now refuse a constructor.** It is the race's
  exit cause on 3 triangle-20 cells of 3 with `cur2=1`, and on **0 of 9** with
  the shipped meter in this round *and* 0 of 9 in
  `basin-race/evidence/racebattery-draw-10s.json`, where every cell of every
  fixture exits on `deadline`.
* **share ceilings become enforceable.** `basin_race_share = 0.34` was inert
  under a work budget for the reason §4.4 gave; the same arithmetic now binds.
* **`ActionClass::prior_cost_in_phase_zero_for` carries two prices for
  Diversify and Schedule** - 1.224 in work units and 1.979 in seconds for
  Diversify, a **17x** disagreement the class comment already calls out as "the
  same action, priced 17x apart on mixed-61". A currency in which the two agree
  is a currency in which that fork can close. This round does **not** close it:
  the priors are pinned numbers in the shipped meter and moving them is a
  separate, measured spend.

## 9. Honest caveats

1. **The box was not quiet.** Load 1 ran 3.9-10.4 across every battery, with
   another agent's work on the same sixteen cores. Every *wall* number here is
   a loaded-box number and is not comparable to `calibrated-plan`'s. What is
   load-independent and carries every claim: the four gate digests (§3.1), the
   nine-cell document equivalence (§3.1), the work-mode determinism (§3.2), and
   §4's arm-against-arm comparisons, which are interleaved within one window.
2. **The profile is one box, one afternoon, and one class.**
   `WORK_CURRENCY_PROFILE` has exactly one entry. The mechanism is per-class
   and the table is a table, but the *fitted* content of this round is a single
   weight for mode 20, and §2.3's residual spans a factor of 3.3 across three
   fixtures. A second box has not been tried.
3. **Seven mode-20 calls is a thin fit.** Three distinct configurations,
   repeated. The corpus's 35 calls agree on the *rate* to within 5% but carry
   no count vectors, because no previous round recorded them - which is the
   reason the observing mode exists and the reason it should stay armed in
   future batteries.
4. **The currency is not free where the diversify class fires, and the one
   fixture where it fires is saturated.** §5.1: 8.6% of a shapes-17 budget, on
   a request that has read 200.349 mm in every arm of every round this
   campaign has run. So the charge is measured and its *consequence* is not.
   This round has not found a request where the class fires, has room, and can
   be measured at equal plan, and until one exists the depth effect of arming
   `cur2=1` in a shipping default is unmeasured rather than zero.
5. **`cur2=1` is measured on nine work cells, nine plan cells and nine race
   cells.** That is 27 paired cells on three fixtures at two budgets. It is not
   a distribution and the three-round plan battery is three rounds.
6. **The equal-work count in §4.2 differs between arms** - 8 of 9 against 7 of
   9 - because the ladder straddled on different cells. The three mixed-61 rows
   are equal-work in both arms and they are the ones the verdict rests on;
   §4.2's pooled median over cells that were not the same set is reported and
   should not be leaned on.
7. **Mode 26 is under-priced by 2.9x and this round did not fit it.** Two calls
   on one fixture is not a fit. It is inside the 3x threshold, so it is left at
   the shipped meter, and that is a decision the threshold made rather than a
   measurement.
8. **Nothing here is wired into a production route.** `cur2` is a spec key on
   the benchmark example, the coordinator that reads it is `coordinator_v3`,
   which is off by default, and the currency's own default is `Off`.
9. **Nothing here touches the record lineage.** The `''` 0.0005 contract, the
   record 155/164 line and the four pinned gates are untouched, and §3.1 shows
   the gate documents are byte-identical.

## 10. Reproducing this

```
bash docs/experiments/work-currency/drivers/collect.sh [BINDIR] [OUTDIR]
bash docs/experiments/work-currency/drivers/publish.sh [OUTDIR] [BINDIR]
```

`collect.sh` runs every battery in the order they have to run, and the order is
not alphabetical: the wall-sensitive batteries first and alone, then the
work-capped ones, then the suites, which saturate every core and would have
made everything before them a measurement of the box.

Each driver also takes the binary as an argument, so a paired A/B can hold two
side by side:

```
D=docs/experiments/work-currency/drivers
F=mixed-61,shapes-17,triangle-20

python3 $D/gates.py       ship SHIP_GATE OUT/gates/ship                     # 3.1
python3 $D/equiv.py       OUT/equivalence.json BASE_COMBO SHIP_COMBO        # 3.1
python3 $D/rates.py       OUT/rates.json SHIP_COMBO                         # 1
python3 $D/fitprofile.py  OUT/rates.json OUT/profile.json                   # 2.3
python3 $D/determinism.py OUT/det-w.json SHIP_COMBO $F 0,1,2 work 40000000 cur2=1
python3 $D/binequiv.py    OUT/binequiv-cur2.json A_COMBO B_COMBO cur2=1     # 3.3
python3 $D/racebattery.py OUT SHIP_COMBO $F 0,1,2 10000 3:1:3               # 4
python3 $D/planbattery.py OUT/planbattery-10s.json SHIP_COMBO mixed-61 0,1,2 10000 3
python3 $D/countertax.py  OUT/countertax.json SHIP_COMBO mixed-61 10000 0,1,2 3
python3 $D/inspect.py     ONE_RUN.json --all      # one run, both currencies
python3 $D/summarize.py   rates|profile|equiv|determinism|race|plan|countertax PATH...
```

`drivers/summarize.py` regenerates every table above straight out of the JSON,
because a table typed by hand from a JSON file is a table that can disagree
with it. `drivers/runlib.py` and `drivers/gatelib.py` carry the pinned CLI
tail, the `0.002` allowance, the salt sets and the request table, with `ROOT`
pointed at this worktree.

The three arms, as one line each:

```
'work=40000000,cells=13:15:17:19,v3=1'            # the shipped meter, unchanged
'work=40000000,cells=13:15:17:19,v3=1,cur2=2'     # priced, reported, not charged
'work=40000000,cells=13:15:17:19,v3=1,cur2=1'     # priced and charged
```
