# The multi-basin race, and three counters that were measuring the wrong thing

> ## Qualified, 2026-08-22 — the verdict stands, §4.3's explanation does not
>
> `docs/sol-review-9-m34cap-provenance.md` §P0 (second) audited this round and
> agreed with **the decision** - the race is off and stays off - while refusing
> **the reason** §4.3 gives for it. *"«0/21 perché i criteri sono una
> landslide» non è dimostrato."* Four defects, each of which would produce 0/21
> on its own:
>
> * the ranker is not dense - it orders by arm number and then by array
>   position, so an identical `stability = 1.0` on every arm still assigns rank
>   0/1/2 and a zero-variance criterion votes for slot 0
>   (`portfolio.rs:4927` at that HEAD);
> * `confirmations_attempted == 0` scores `stability = 1.0`, and since higher is
>   better that is the **maximum**, not the neutral value §4.3 calls it
>   (`portfolio.rs:5397`);
> * the arms are not isolated - every audition archives and immediately tries to
>   publish (`portfolio.rs:3246` at that HEAD), so a challenger can become the
>   incumbent before it is judged and stay there after it is eliminated;
>   §4.1's retirement paragraph is about the *archive* and does not undo a
>   publication;
> * the winner is never adopted: some challengers are removed and slot 0 is
>   never retired (`portfolio.rs:5192`), so the race produces a queue ticket
>   rather than a decision.
>
> To it, §10.7's caveat below is the closest this document comes to the finding
> and it does not reach it: a criterion with zero variance is not merely
> uninformative here, it is *actively voting for slot 0* through the tie-break.
>
> **What is unaffected.** The cost diagnosis - m20 priced 71,500x below what it
> costs, so a share ceiling in the legacy currency cannot bound the draws - is
> sound, was re-measured in `docs/experiments/work-currency/` §4, and is the
> reason the retirement is not merely a null. The equal-work depth deltas
> (+2.366 / +2.934 mm at ten seconds, +1.879 mm at thirty) are measurements and
> stand.
>
> **What would have to change before re-opening it**, per that review: dense
> ranks, arms that do not publish globally, equal maturation, and explicit
> adoption of the winner. Under the ten-second mandate it is cut from the board
> - see [`docs/shipped-surface.md`](../../shipped-surface.md) §3.

Four things. The three fixes are what both reviews said had to happen before any
further sparse-rotation or design-C number could be read at all; the race is the
spend both reviews ranked first.

Three of the four are negative results, and they are negative in ways that are
worth more than a positive one would have been: each retires a claim the
campaign was carrying. Each is stated with the number that decides it.

---

## The headline

| | |
|---|---|
| **Fix (a), trigger B** | the disarm compares the **handed** loss, not the step's historical minimum. `10 → 8 → 9 → 8.5` now disarms on the last sweep; the old rule stayed armed. One shared `StallDetector`, so the serial loop and every fan-out worker run the same rule |
| **Fix (b), the disarm bit** | an operator-specific chain replaces `rotation_accepted_moves`. Measured on 12 parents at equal work: the **control arm proposes zero rungs and reports 3,841 `rotationAcceptedMoves` against 0 committed sparse moves** — Sol's 11,523 cell, reproduced on this tree. On design B the old counter overstates the operator by **1.602×** |
| **Fix (c), design C** | the accepted witness is wired into a child frontier, spec-keyed and off. Sol's A/B, 12 parents, equal work: **`publish` == `off` on 12 of 12, to the digit** — design C as shipped never survived to the published depth. With the wire, **2 of 12 descendant publications**; 2 better / 3 worse / 7 tied, median **0.000 mm** |
| **The race** | built, spec-keyed, off. Three arms, three criteria, successive halving, losers retired from the archive. Two variants measured |
| **The race's verdict** | **it never once moved the run off the incumbent: 0 of 21 cells** — both variants at ten seconds, and the top-2 arm at thirty. So the entire depth delta is cost |
| **The equal-work gate** | **the race fails it.** The two equal-work mixed-61 cells lose **+2.366** and **+2.934 mm** (the third, not equal-work, loses +4.245); shapes-17 is saturated; triangle-20 moves by at most 0.044 mm for a reason that is not the race (§4.5) |
| **Why, in one ratio** | the work meter prices a second of mode 20 at **92.7 units** and a second of mode 34 at **6,628,431** — a **71,500×** spread inside one phase of one run. Mode 20 is **70.8% of the race phase's wall and 0.0123% of its work**, so a work-denominated share ceiling cannot bound it |
| **Four pinned gates** | 4 of 4 on both binaries, and the **whole-document digest is identical to the base binary on all four** |
| **Determinism** | work mode **9/9 race on, 9/9 race off** — the gate this round's code is responsible for. Plan mode on a contended box: 7/9 and 5/9, and every one of those six misses is a *plan* disagreement rather than a document one (§8) |
| **Suites** | both pass first attempt, exits **0** and **0** — 1,268 and 1,322 tests, zero failures, and the known-flaky `free_material_multi_eviction...` passed first attempt too |

**Recommendation.** Keep the three fixes. Keep the race **off** and keep the
code: between them §4.3 and §4.4 say exactly what would have to change for it to
be worth arming — a criterion set that can compare an incumbent with a
challenger, and a meter that can see mode 20 — and neither is a constant to be
tuned. Design C: do **not** cut it on Sol's stated rule — 2/12 is not 0/12 — and
do **not** ship the adoption. §5.4.

---

## 0. What this round was asked to do

Sol review 8 §4 item 3 and Grok review 3 §3 item 2 are the same spend under two
names. Sol:

> **M — Multi-basin race con successive halving, non m20 feeder.** Il best arm
> FCV ha spread 165.656–174.280: il rischio dominante è ancora entrare nel basin
> sbagliato. Avviare 2–3 salts a cap breve, valutare non solo depth ma
> primo-batch m34 yield, binding-front stability e proxy infeasibility;
> continuare top-1/top-2. Rischio: l'early leader può essere il late loser e
> dividere 10 s può affamare tutti. Gate equal-work su molti seed/request.

And three one-line fixes both reviews demanded first. All four are below, in the
brief's own order, because the fixes are what make the race's instrument
readable at all.

The canonical instrument is `plan=<ms>` (`docs/experiments/calibrated-plan/`):
one depth per seed, deterministic, with the wall as the noisy reference. Every
paired number is at equal plan, and §4.2 explains why "equal plan" is a stronger
statement here than usual.

---

# Part I — the three fixes

## 1. Fix (a): trigger B compares the loss it was handed

### 1.1 The bug

`docs/experiments/sparse-rotation/` documents the trigger as *"a sweep that left
the frontier infeasible **and** did not lower the loss it was handed"*. The code
compared against the step's historical minimum instead — `stall_loss =
stall_loss.min(now)`, seeded with the **entry** loss. Sol review 8 §2 P1 and
Grok review 3 §2 item 1 both name it, and Grok names the consequence:
*"candidato meccanico della perdita a 30 s"*, where the armed arm's loss share
went 31.8% → 40.1%.

The two rules disagree exactly when translation resumes *above* a minimum the
step has already left behind — the normal behaviour of a weighted repair,
because `update_weights` moves the weights under the frontier after every sweep:

| sweep | loss | handed | documented rule | historical-minimum rule |
|---|---:|---:|---|---|
| entry | 10 | — | — | — |
| 1 | 8 | 10 | disarm | disarm |
| 2 | 9 | 8 | arm | arm |
| 3 | **8.5** | **9** | **disarm** | **arm** |

### 1.2 The fix

The rule is now a two-line state machine, `StallDetector`, and **both** call
sites go through it — the serial repair loop and each parallel fan-out worker.
They were two copies of the same six lines before, which is how a rule drifts.

Two properties are deliberate and pinned:

* **equality is a stall.** A sweep that returned exactly the loss it was handed
  did not lower it, and the fixpoint is the state the rungs exist for.
* **a `NaN` loss arms.** The comparison is written `!(now < handed)` rather than
  `now >= handed`, so an unreadable loss fails *armed*: it is not evidence that
  translation is working.

Regression test:
`general_relaxed::tests::trigger_b_disarms_when_translation_resumes_above_the_step_minimum`.
It runs the witness sequence through the production detector **and** runs the old
rule beside it, asserting the two disagree — so a change that made them agree
again fails rather than quietly making the test vacuous.

### 1.3 What it changes in a shipped run

Nothing. It is a policy change on an off-by-default operator: `sparse_rotation`
is `false` in `PortfolioSettings::new`, and the four pinned gates reproduce with
an identical whole-document digest (§7). What it changes is what an *armed* arm
does, which §2 makes measurable for the first time.

## 2. Fix (b): the disarm bit was reading the catalogue

### 2.1 The bug, reproduced on this tree

`rotation_accepted_moves` counts *any* accepted move whose committed pose differs
from the incumbent's in rotation or mirror. `search_piece` draws
`focused_samples_per_move + global_samples_per_move` random catalogue candidates
as refinement starts, and a start that wins at a different catalogue angle is one
of those moves — on a lane that was never offered a rung. Sol review 8 §2 P0
gives the material proof from the previous round's control arm: **zero rungs,
11,523 `rotationAcceptedMoves`.**

`drivers/attribution.py`, `evidence/attribution-12parents.json`. Twelve pinned
mixed-61 parents (171.6–179.6 mm, the compression-schedule round's own set), one
serial mode-34 slice each at a **3,341,379-unit work cap**, three arms on one
binary, `0.002` allowance:

| arm | rungs proposed | `rotationAcceptedMoves` | `sparseRotationCommittedMoves` |
|---|---:|---:|---:|
| **control** (operator off) | **0** | **3,841** | **0** |
| design A (armed everywhere) | 501,564 | 43,650 | 0 |
| design B (sparse) | 51,296 | 6,139 | **3,833** |

The control row is the finding: 3,841 accepted "rotation" moves over twelve
slices that proposed not one rung. Fed to the disarm bit, that reads as *"the
operator is productive"* about a lane the operator never touched, and "the disarm
was never necessary" is an artefact rather than a finding.

The design-A row is the other half of the check: the sparse column is **zero**
there too, and correctly so. Design A's rungs belong to the whole descent and to
no episode, so a *sparse* column that counted them would be a second copy of
`rotation_rungs_proposed` rather than an attribution. The bit takes no verdict on
design A either, because it requires `episodes > 0` and design A has none.

### 2.2 The chain that replaced it

Three links, each a subset of the one before, each taken at the only site where
both sides of the question exist:

1. **proposal** — `sparse_rotation_rungs_proposed`, incremented in
   `refine_candidate` beside the design-A counter, only while an episode is open;
2. **winner** — `sparse_rotation_rung_winners`, incremented when a
   continuous-axis candidate from an open episode becomes the incumbent **and**
   moved the pose. A rung that wins on a tie, or whose angle rounds back onto the
   incumbent's key, bought nothing;
3. **commit** — `sparse_rotation_committed_moves`, incremented at the
   accepted-move site only when the pose being written is still, key for key, the
   pose the rung produced.

Link 3 does the work, and it is deliberately a *comparison* rather than a flag.
The owner is recorded by the rung that won it and **nothing clears it**, so any
later stage that moves the pose — the NFP axis minimiser, the second refinement
pass, the dynamic-hazard revert — makes the keys disagree and the episode loses
the credit, without that stage having to know the field exists.

Design B's funnel over the same twelve parents:

| | count | as a fraction |
|---|---:|---|
| episodes | 3,391 | |
| sparse rungs proposed | 104,244 | 30.7 per episode |
| rung winners | 29,247 | **28.06%** of proposals |
| committed moves | 3,833 | **13.11%** of winners |
| committed episodes | 2,322 | **68.48%** of episodes |
| `rotationAcceptedMoves` | 6,139 | **1.602×** the committed moves |

That last ratio is the size of the mis-attribution on an *armed* lane: 2,306 of
the 6,139 moves the old counter gave the operator were not the operator's.

`sparse_rotation_committed_episodes / sparse_rotation_episodes` = **68.48%** is
the number a disarm bit should read, and it is now what it reads. The bit's rule
was extracted into `SparseRotationBit::observe_slice` so the regression test
drives the production function rather than a copy of it — the previous test
re-implemented the rule inline and would have passed straight through this bug.

Regression tests:
`general_relaxed::tests::a_committed_move_is_charged_to_the_episode_that_actually_produced_it`
(episode identity, the two-axis pose comparison, distinct-episode counting) and
`portfolio::tests::the_disarm_bit_cannot_be_fed_the_catalogues_accepted_moves`,
which feeds one slice through the bit twice — once with the committed count, once
with the control arm's accepted count — and asserts the two verdicts **differ**.

## 3. Fix (c): design C's accepted witness had nowhere to go

### 3.1 The bug

Sol review 8, verbatim:

> C trova proposte exact-valid, ma aggiorna solo `published_depth_mm/placements`;
> non aggiorna `state`, `confirmed_state`, floor o archive. Quindi 0/12 finali
> prova che il one-shot publication viene poi dominato, non che
> `witness → m34` non componga.

### 3.2 The wire

`Se2WitnessSettings::adopt`, a fourth field on the existing key
(`se2w=trust:iterations:maxcalls[:adopt]`; three-part specs still parse and still
mean `adopt = 0`, so every previously recorded spec reproduces), default
**false**. When it is set and a witness is accepted, the witness layout becomes
**both halves of the schedule's snapshot**:

* `confirmed_state` at `confirmed_state`'s own clamp — legal by construction,
  because the certificate's line search only ever returns a layout
  `validate_publication` accepted;
* the live `state` at the **frontier** clamp, so the next sweep repairs the
  witness toward the depth the schedule has already stepped to.

The **floor is not moved**, and that is a decision:
`CompressionSchedule::note_confirmed` takes no depth argument on purpose, so a
floor is what an accepted confirmation *at the frontier* leaves behind, and a
witness accepted at the floor's own depth has not compressed anything yet.

Mapping the witness back onto the lane's state is `relaxed_state_from_moved`,
and it refuses rather than guesses: `se2_witness_proposal` builds its output by
zipping the placements it was handed, so row `i` out is row `i` in — both the
length and every piece id are checked, and a mismatch returns `None`, which the
caller reads as "do not adopt".

Regression test:
`general_relaxed::tests::a_witness_maps_back_onto_the_parent_state_slot_for_slot`,
on a parent whose slots are deliberately **not** in input-index order, so a
mapping that used the row position as the input index would fail rather than pass
by coincidence.

---

# Part II — the measurements

## 4. The race

### 4.1 What was built

A phase, not a class, and it runs **before** the v3 queue so the queue inherits a
decision rather than a ticket. `run_basin_race`, spec keys
`race=arms:keep:rungs[:share]`, `racedraw`, `raceevict`, all off by default.

**The arms.** Slot 0 is the **incumbent control** — the layout phase 0 published,
no draw at all. It is in the race for two reasons, and the second is what makes
the equal-work gate fair:

* a winner that is not slot 0 is a basin the un-raced run would never have used,
  which is the round's central question reduced to one integer;
* its audition batch is **not overhead**, because it is the first mode-34 action
  the queue would have spent on that layout anyway. The race's true price is the
  challenger arms alone.

Slots 1.. are salted constructor draws by default, and the salting is the
ledger's: mode 20 derives `construction_seed` from
`parent_seed_key ^ CONSTRUCTION_SEED_DOMAIN ^ grid_key(target_depth_mm)`, so two
draws that differ only in their **seed** are replicas and two that differ in
their **target** are different lotteries. Each slot moves the clamp by
`BASIN_TARGET_SALT_RELATIVE_STEP` and takes its own void-cell divisor, exactly as
the diversify class does, and then descends the draw with one mode-22 quantum —
because a raw constructor layout and a descended one are not the same kind of
object and an audition comparing them would be measuring the descent.

**The cap is in rungs, not work units,** for the reason the schedule class
already gives at its own call site: a work cap in the coordinator's currency
reads zero when profiling is off, and a wall-budget run has it off. An audition
is `basin_race_rungs = 3` against a full action's `SCHEDULE_RUNGS = 9`.

**The criteria are Sol's three, and depth is excluded.** Grok review 3 §3 item 3
gives the reason — a *worse* constructor can open a *better* basin — so ranking
on how deep an arm is right now systematically prefers the arm that has already
fixpointed:

| criterion | read from | direction |
|---|---|---|
| first-batch m34 yield | raw mm the audition took off the arm's own parent | higher |
| binding-front stability | `confirmations_accepted / confirmations_attempted` | higher |
| proxy infeasibility | `(entry_collision_pairs + entry_boundary_violations) / pieces` | lower |

An arm whose audition batch produced no schedule report at all — the schedule
refused to enter its layout — scores the **neutral** 1.0 on stability and the
**worst possible** value on infeasibility, and the second is deliberate: a batch
the schedule would not enter is not evidence that the basin is good, and the race
must not commit to an arm it could not measure. That arm's `infeasibility`
reaches the JSON as `null`, because `f64::INFINITY` is not representable there;
in the tables below it reads `inf`.

Ranked by **rank sum**, not by a weighted score: the three are in three
incommensurable units, so any weighting would be three constants this round had
to tune on the same nine cells it is trying to measure. A rank sum needs none and
is invariant to every monotone rescaling of any criterion. **Every tie breaks
toward the lower slot**, so the incumbent wins every tie — a race that cannot tell
its arms apart must not move the run off the basin it already had.

**The halving.** A round runs only while there is something to decide.
`keep = ceil(live / 2)`, never below the target, rungs doubled each round and
capped at `SCHEDULE_RUNGS`. Three arms to one winner is two rounds and **21
rungs** — 3×3 then 2×6 — against a full action's nine. The survivor is
deliberately *not* re-auditioned alone: its continuation is the queue's first
mode-34 action, which the run was always going to buy, and running it inside the
race would charge the race for it. That is also the literal form of "the loser's
work returns to the winner": the race stops early and everything its share had
left goes to the queue that is now working on the arm the race chose.

Eliminated challengers are **retired from the archive**
(`SearchArchive::retire`), and *every* layout they put there is retired, not just
the last — `run_operator` archives whatever it produces, so a drawn arm
auditioned twice has left up to four members behind: the raw draw, its descent,
and one per batch that improved on it. With `raceevict=0` the losers stay and the
queue can rank them like any other member, which is a real arm to measure and is
why the key exists.

### 4.2 The battery: 0 of 18

`drivers/racebattery.py`, `evidence/racebattery-draw-10s.json` and
`evidence/racebattery-archive-10s.json`. Three fixtures × three seeds × two arms
(`race=0` / `race=3:1:3`), arm order rotated per cell, `plan=10000`, v3 on,
`0.002` allowance, the bare request.

**Equal plan is an unusually strong statement here.** `plan=<ms>` is installed at
the end of phase 0 — *before* the race phase and before the queue — so both arms
of a cell run the same phase 0 and read the same probe **counter**, which is
bit-identical between them. The only input that differs is the one clock reading
the ladder exists to absorb, so the two arms should land on the same rung and
receive the same integer work budget. Should, not must: the ladder straddles
under load, and this driver therefore **checks** `portfolio.plan.units` per cell
rather than assuming it, and excludes a row where the two disagree from every
aggregate rather than averaging it in.

The specified arm, salted constructor draws:

| cell | plan units, off / on | off | on | delta | equal work | race work | race s | wall off / on | actions off / on | winner |
|---|---|---:|---:|---:|---|---:|---:|---|---|---|
| mixed-61 s0 | 24,891,457 / 21,644,745 | 175.3878 | 179.6330 | +4.2452 | **no** | 4,774,379 | 8.92 | 7.61 / 14.73 | 6 / 3 | slot 0 |
| mixed-61 s1 | 24,891,457 / same | 174.1700 | 176.5363 | **+2.3663** | yes | 5,523,724 | 9.64 | 7.16 / 17.08 | 5 / 3 | slot 0 |
| mixed-61 s2 | 21,644,745 / same | 176.1620 | 179.0963 | **+2.9343** | yes | 4,883,949 | 8.18 | 7.77 / 13.46 | 5 / 3 | slot 0 |
| shapes-17 s0 | 7,075,705 / 6,152,787 | 200.3490 | 200.3494 | +0.0004 | **no** | 2,219,428 | 4.54 | 9.59 / 8.23 | 10 / 3 | slot 0 |
| shapes-17 s1 | 6,152,787 / 7,075,705 | 200.3494 | 200.3494 | 0.0000 | **no** | 2,227,712 | 4.11 | 7.67 / 9.63 | 8 / 6 | slot 0 |
| shapes-17 s2 | 6,152,787 / 7,075,705 | 200.3490 | 200.3490 | 0.0000 | **no** | 2,217,775 | 4.19 | 7.69 / 10.34 | 8 / 7 | slot 0 |
| triangle-20 s0 | 37,856,795 / same | 70.7711 | 70.7273 | −0.0439 | yes | 15,466,810 | 6.14 | 6.68 / 10.86 | 5 / 3 | slot 0 |
| triangle-20 s1 | 43,535,314 / same | 70.7468 | 70.7273 | −0.0196 | yes | 15,771,744 | 6.20 | 7.24 / 11.58 | 5 / 3 | slot 0 |
| triangle-20 s2 | 43,535,314 / same | 70.7416 | 70.7273 | −0.0144 | yes | 15,514,834 | 6.10 | 8.33 / 12.18 | 7 / 4 | slot 0 |

The archive variant (`racedraw=0`, challengers taken from the basins phase 0
already archived), same nine cells, six of them equal-work: **race better 0,
worse 4, tied 2**, mean +0.151 mm. Its shapes-17 cells are the control this round
did not have to construct — the archive offered no distinct challenger, the race
did nothing at all (`rounds = 0`, race work **0**), and its two equal-work cells
came out at **exactly 0.0000 mm** with wall parity (8.00/8.06 and 8.20/9.16 s).
**A race with nothing to decide costs nothing measurable**, so the phase itself
is not the tax: the challengers are.

**A pooled median over these nine cells is the wrong statistic and this round
will not quote one.** Which cells are equal-work changes between sessions with
the plan ladder, and the three fixtures are three different regimes. Read by
fixture:

* **mixed-61 loses, decisively**: +2.366 and +2.934 mm on the two equal-work
  cells, +4.245 on the third;
* **shapes-17 is saturated** at 200.349 in every arm of every round this campaign
  has run, so its six cells carry essentially no information about the pick;
* **triangle-20 moves by at most 0.044 mm** in either direction across both
  batteries, and §4.5 shows that the movement is not the race.

### 4.3 Why the race never moves: the criteria are a landslide

**Winner slot 0 in 18 of 18 cells**, both variants. That is not bad luck, and the
arm rows say what it is. The three mixed-61 cells of the specified arm, which are
the ones carrying the verdict, in full:

| cell | slot | kind | depth | yield | stability | infeasibility | confirmations | rank sum | eliminated |
|---|---:|---|---:|---:|---:|---:|---:|---:|---|
| mixed-61 s0 | 0 | incumbent | 181.0280 | 0.5610 | 1.000 | **0.000** | 93 | **0** | — |
| mixed-61 s0 | 1 | salted-constructor | 196.5434 | 0.0000 | 1.000 | 0.639 | 148 | 3 | — |
| mixed-61 s0 | 2 | salted-constructor | 194.6280 | 0.0000 | 1.000 | inf | 0 | 6 | 1 |
| mixed-61 s1 | 0 | incumbent | 179.0550 | 0.6350 | 1.000 | **0.000** | 76 | **1** | — |
| mixed-61 s1 | 1 | salted-constructor | 192.4179 | 0.0000 | 1.000 | 0.574 | 138 | 5 | 1 |
| mixed-61 s1 | 2 | salted-constructor | 194.2437 | 0.7157 | 1.000 | 0.541 | 147 | 2 | — |
| mixed-61 s2 | 0 | incumbent | 179.3050 | 0.3570 | 1.000 | **0.000** | 57 | **1** | — |
| mixed-61 s2 | 1 | salted-constructor | 210.0220 | 0.3634 | 1.000 | 0.541 | 158 | 2 | — |
| mixed-61 s2 | 2 | salted-constructor | 199.4109 | 1.1193 | 1.000 | 0.623 | 151 | 4 | 1 |

A rank sum is only comparable within the round that produced it, which is why
the eliminated arms carry a three-arm sum and the survivors carry the two-arm
sum the final judge gave them. `drivers/summarize.py race` prints all 45 rows.

Over all **45 arms** of both batteries:

* **`stability` is 1.000 on every single one.** The criterion has *zero variance*
  — 45 arms, one distinct value. The schedule steps by one canonical grid
  quantum, so on these three requests a confirmation attempt is essentially
  always accepted; an arm whose batch attempted nothing scores the neutral 1.0.
  Binding-front stability, read off the schedule's own confirmation ledger, is
  not a signal on this workload.
* **`infeasibility` takes exactly two values on the incumbent across all
  eighteen cells** — 0.000 on mixed-61 and triangle-20, 0.353 on shapes-17 —
  against 0.350–1.000 on the challengers, and it is **strictly lower than every
  challenger of its own cell, in all fifteen cells that had one.** That is
  structural, not incidental: the incumbent is a *published, exact-valid*
  layout, so the proxy sees few or no violating pairs, and a fresh draw or an
  unrepaired archive basin always has some. The criterion is not measuring
  "which basin is better", it is measuring "has this arm already been repaired"
  — and the incumbent always has.
* **`yield` also favours the incumbent** in most cells (0.000–0.635 mm against
  0.000–1.119 mm), because three rungs on an already-repaired frontier compress
  and three rungs on a raw draw mostly do not.

So the incumbent wins 3–0 or 2–1 in every cell, and the race spends up to
**15.8 M work units** reaching a foregone conclusion — or exactly zero, in the
six archive-variant cells where there was no challenger to audition. Two of
Sol's three criteria are the right *kind* of criterion and the wrong
*comparison*: they discriminate among peer basins, and the arm set contains one
arm that is not a peer.

This is a correction to the brief rather than a failure to follow it. A race
whose arms are all challengers — drop slot 0, commit to a fresh basin
unconditionally — would make the criteria discriminate again, and would also be a
strictly worse policy at ten seconds, because §4.4 is what a challenger costs
before it can be judged at all.

### 4.4 The price, and the ratio that explains it

`drivers/summarize.py price`, mixed-61 seed 0's race phase, read off
`portfolio.operatorCalls`. Shares and rates rather than seconds, because those
are the two forms that survive a contended box (§10 caveat 3):

| call | wall | % of race wall | work units | % of race work | units/s |
|---|---:|---:|---:|---:|---:|
| `race m20 slot1` | 3.149 s | 35.3% | 310 | 0.0065% | 98 |
| `race m22 quantum on slot1` | 0.920 s | 10.3% | 811,865 | 17.0156% | 882,884 |
| `race m20 slot2` | 3.165 s | 35.5% | 275 | 0.0058% | 87 |
| `race m22 quantum on slot2` | 1.312 s | 14.7% | 1,524,998 | 31.9619% | 1,162,399 |
| `race m34 batch slot0 (3 rungs)` | 0.295 s | 3.3% | 1,074,976 | 22.5301% | 3,639,152 |
| `race m34 batch slot1 (3 rungs)` | 0.072 s | 0.8% | 1,358,870 | 28.4801% | 18,928,159 |

Aggregated by operator:

| operator | wall | % of race wall | work | % of race work | **units per second** |
|---|---:|---:|---:|---:|---:|
| mode 20 | 6.313 s | **70.8%** | 585 | **0.0123%** | **92.7** |
| mode 22 | 2.232 s | 25.0% | 2,336,863 | 48.98% | 1,047,216 |
| mode 34 | 0.367 s | 4.1% | 2,433,846 | 51.01% | **6,628,431** |

**The work meter prices a second of mode 34 at 71,500× a second of mode 20.**
Grok review 3 §3 item 3 predicted exactly this — *"mode 20 è quasi gratis in work
units e il work budget lo sotto-prezza"* — and this is the number. The ratio is
two rates measured in the same phase of the same run, so a uniform slowdown moves
both and leaves it where it is.

The consequence is structural rather than a tuning miss. The race's share ceiling
(`basin_race_share = 0.34` of what phase 0 left) is enforced in the budget's own
currency, and under `plan=` that currency is work. So it **cannot bound the
draws' wall**: every mixed-61 cell exits the race phase on `deadline` having
spent 8.2–9.6 s, and the run lands at 13.5–17.1 s against a ten-second target
while the un-raced arm lands at 7.2–7.8 s. The affordability check in
`run_basin_race` is written the way the queue writes its own, and it is
measurably near-inert for this reason; the code says so at the site.

This is Sol review 8 §3 condition 4 arriving from a new direction. That condition
is about mode 34 being atomic and the work meter being *expensive*; this is the
same meter being **blind** to a different operator. Any scheduler that prices
mode 20 by the work meter will over-buy it.

### 4.5 The equal-work gate, stated plainly

The brief's gate is: *the race arm must not lose at equal total work.*

**It loses.** On mixed-61, the only fixture of the three where a basin decision
has room to matter at ten seconds, the two equal-work cells are **+2.366** and
**+2.934 mm** worse.

Two things make the verdict cleaner than the numbers alone:

* the race changed **nothing** (0/18 basin moves), so the entire delta is the
  audition's price and not a bad pick. A race that never picks differently has no
  quality upside to weigh against its cost;
* the triangle-20 cells where the race arm reads "better" are **not the race
  working**. `run_operator` archives *and publishes* whatever it produces, so a
  salted draw that lands at 70.7273 mm publishes even though the race then
  eliminated it from the archive. All three triangle-20 race arms land on exactly
  70.72726178003285 — the constructor draw's own depth, to seventeen digits, not
  a compressed one. That is the diversify class's value arriving through the
  race's plumbing, and it costs 15.5–15.8 M work units to buy 0.014–0.044 mm.

### 4.6 The thirty-second, top-2 arm

Sol's framing offers *"continuare top-1/top-2"*, and the second survivor is only
worth its slice when there is a slice left for it. `race=3:2:3` at `plan=30000`,
mixed-61 only — the one fixture where a basin decision has room —
`evidence/racebattery-draw-30s-keep2.json`:

| cell | plan units, off / on | off | on | delta | equal work | race work | race s | wall off / on | actions off / on | winner |
|---|---|---:|---:|---:|---|---:|---:|---|---|---|
| mixed-61 s0 | 66,211,771 / same | 164.1880 | 166.0670 | **+1.8790** | yes | 6,064,965 | 9.14 | 25.70 / 32.64 | 21 / 18 | slot 0 |
| mixed-61 s1 | 57,575,453 / 66,211,771 | 171.3620 | 166.9876 | −4.3744 | **no** | 5,523,724 | 8.53 | 22.42 / 36.45 | 13 / 19 | slot 0 |
| mixed-61 s2 | 57,575,453 / 43,535,314 | 165.1902 | 179.0963 | +13.9061 | **no** | 4,883,949 | 8.30 | 30.78 / 23.53 | 17 / 7 | slot 0 |

**0 of 3 basin moves again**, and the one cell that is equal-work loses
**+1.879 mm**. The two that are not equal-work are not evidence in either
direction and are shown so a reader can see how much budget separated them: at
thirty seconds the plan ladder straddles harder than at ten, which
`docs/experiments/calibrated-plan/` §10.3 predicts — *"the fitted bias rises with
the budget and a constant fitted at ten is no longer conservative"* — and s1's
two arms differ by a full rung and s2's by two.

Three cells is thin and this section does not pretend otherwise. What it does
show is that the pick rate does not change with the budget, which is what §4.3
predicts: the criteria's landslide is a property of the arm set, not of how long
the audition gets. The race's *price* is amortised — 4.9-6.1 M work units out of
a 43-66 M plan against the same absolute cost out of a 22-25 M plan — and the
race still loses the one readable cell, because it is still buying nothing.

## 5. Design C: `publish == off` on twelve of twelve

`drivers/witnessab.py`, `evidence/witnessab-12parents.json`. The same twelve
pinned parents, the same 3,341,379-unit cap, design B armed on the equivariant
construction, witness `0.025:64:2` — the setting the sparse-rotation round priced.
Three arms: `off` (no witness), `publish` (`adopt = 0`, design C exactly as
shipped), `adopt` (`adopt = 1`, the child frontier).

| parent | off | publish | adopt | adopt − publish | witness accepted | adoptions | descendant |
|---|---:|---:|---:|---:|---:|---:|---|
| s0 174.2081 | 173.4960 | 173.4960 | 173.4960 | 0.0000 | 0 | 0 | |
| s1 176.0560 | 174.4859 | 174.4859 | **174.4230** | **−0.0629** | 2 | 2 | **yes** |
| s2 179.0060 | 177.2540 | 177.2540 | 177.2810 | +0.0270 | 1 | 1 | |
| s3 176.0610 | 176.0610 | 176.0610 | 176.0610 | 0.0000 | 0 | 0 | |
| s4 171.6495 | 171.0966 | 171.0966 | 171.0966 | 0.0000 | 0 | 0 | |
| s5 179.0518 | 177.3790 | 177.3790 | 177.4110 | +0.0320 | 1 | 1 | |
| s6 179.6200 | 178.5180 | 178.5180 | 178.5180 | 0.0000 | 0 | 0 | |
| s7 179.5223 | 178.4970 | 178.4970 | 178.4970 | 0.0000 | 0 | 0 | |
| s8 178.9320 | 177.2880 | 177.2880 | 177.8420 | **+0.5540** | 2 | 2 | |
| s9 174.9656 | 173.4413 | 173.4413 | **173.4350** | **−0.0063** | 1 | 1 | **yes** |
| s10 176.3622 | 175.3607 | 175.3607 | 175.3607 | 0.0000 | 0 | 0 | |
| s11 171.6141 | 170.4910 | 170.4910 | 170.4910 | 0.0000 | 0 | 0 | |

**How the brief's A/B was read.** "Accepted witness → child frontier → one m34
batch vs parent at equal work" admits two readings: run the batch *inside* the
slice that produced the witness, or lift the witness out and start a fresh batch
from it against a fresh batch from the parent. This round ran the first, for two
reasons. It is where the integration actually lives — the witness is produced by
a schedule step and the question is whether that step's own successors inherit
it — and it is the only one of the two that can produce the §5.1 finding, because
"the one-shot publication is overwritten" is a statement about what happens
*later in the same slice*. The external reading is a different and complementary
experiment; it is not run here, and §5.2's 2/12 should be read as a statement
about in-slice composition.

One thing the `adopt` arm gets for free, stated rather than buried: rebuilding
the tracker over the adopted layout is a `score_state`, and `score_state` does
not increment `surrogate_evaluations`, which is what the slice's work cap counts.
So each adoption costs one uncharged full rescore. It is the same exemption the
schedule's own rollback path has had since the port, and it is small — seven
adoptions across twelve slices, `61 × 60 / 2 = 1,830` pairs each, against a
3,341,379-unit cap per slice — but it is a thumb on the `adopt` arm's side of the
scale, and the arm loses anyway.

### 5.1 Sol's diagnosis is confirmed, exactly

**`publish` equals `off` on 12 of 12 parents, to the digit.** The witness was
accepted **7 times across 5 parents** and bought a cumulative
`se2WitnessBoughtMm` of **0.173 mm** — and not one micron survived to the
published depth. Design C as shipped is not a weak mechanism, it is a **no-op on
the published depth**: its one-shot publication is overwritten by the schedule's
own next accepted confirmation, every time. So the previous round's 0/12 was
measuring the integration, as Sol said.

### 5.2 With the wire, it composes — twice

**2 of 12 descendant publications** (s1 by 0.0629 mm, s9 by 0.0063 mm). Sol's
stated stopping rule is *"Se resta 0/12 descendant publications, taglio
witness/m33 dalla produzione"*, and 2/12 is not 0/12. The rule does not fire.

### 5.3 But it does not pay

2 better, **3 worse**, 7 tied; median **0.000 mm**; mean **+0.0453 mm** (worse).
The three losses are larger than the two wins, and s8 alone is +0.554 mm.

The counters say why, and it is a mechanism rather than noise. Adopting the
witness makes the frontier markedly harder to repair:

| parent | episodes, `publish` | episodes, `adopt` |
|---|---:|---:|
| s1 | 46 | 110 |
| s2 | 686 | 690 |
| s5 | 807 | **2,308** |
| s8 | 254 | **2,468** |
| s9 | 116 | 197 |

The two cells whose episode count explodes 2.9× and 9.7× are two of the three
where `adopt` loses. The witness moves every piece a little; the child frontier
it hands the schedule is a layout whose pairs the lane's weights know nothing
about, and the repair spends the rest of the slice re-learning them. The SE(2)
step is real — `validate_publication` accepted it — and it is *locally* free and
*globally* expensive.

### 5.4 Verdict

Do not cut design C on Sol's stated rule: the rule is 0/12 and the measurement is
2/12, so the premise is false and the previous null was the instrument.

Do not ship the adoption either. At equal work over twelve parents it is
2 up / 3 down / median 0.000, and its failure mode is the search-trajectory risk
Grok review 3 flagged for the whole rotation family rather than anything the
certificate did wrong. `adopt` stays a spec key, off, with the A/B that would
have to be won recorded here.

In one line: this round turned *"the witness does not compose"* into *"the
witness composes and the composition is not worth the frontier it disturbs"*,
which is a different claim about a different thing.

> **Qualified, 2026-08-22.** `docs/sol-review-9-m34cap-provenance.md` §P0
> (third) accepts *"do not ship the adoption"* and refuses *"the composition"*
> as a description of what 2/12 shows. Three reasons, and each is checkable in
> this round's own artefacts:
>
> * **`descendant` is defined as `final(adopt) < final(publish)`**
>   (`drivers/witnessab.py:150`), which is *"the trajectory changed and ended
>   lower"* and not *"a confirmation after the witness published"*. It refutes
>   "the trajectory never changes"; it does not establish composition.
> * **The arms are not equal-work.** Seed 1 is `10,150,405` units on one arm
>   against `10,433,031` on the other
>   (`evidence/witnessab-12parents.json:132,190`), so "at equal work over twelve
>   parents" above is 2.8% out on at least one cell.
> * **The adoption breaks the `confirmed` invariant.** It assigns the child to
>   `confirmed_state`, moves it onto the clamp frontier, and only then builds
>   the surrogate and tracker behind a fallible `?`; the ordinary path writes
>   `confirmed_state` only after the frontier has passed the composite
>   validator. So a rollback can restore a contract-valid but
>   envelope-infeasible snapshot, and a mode-34 `exactValid = true` can describe
>   a layout that never passed the internal composite gate.
>
> The verdict - `adopt` stays a spec key, off - is unchanged and is now
> **over**determined. What a retry would need: transactional construction in
> temporaries, the composite/proxy gate at the floor, the work charged, and an
> explicit *"post-adoption confirmation published"* counter.

---

# Part III — the protocol

## 6. Binaries

`evidence/binaries.json`, `drivers/build.sh`. The combo is
`jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator`.

| label | features | sha256 |
|---|---|---|
| `base-gate` | `jagua-experimental`, base commit `29d5780` | `afaf5b0fa0b43069ceaf933f5060355b4c1da90c1c23d5bd59973ac750ee97de` |
| `base-combo` | the combo, base commit `29d5780` | `5681046a61fc665e0448eec75c68cd163849792b084759d58410b25ecd3f7cc0` |
| `race-gate` | `jagua-experimental`, this tree | `c7c2e556f390e552bbc3c344b1943bad92464c068da5fe853bd6fcfe9b1d73b1` |
| `race-combo` | the combo, this tree | `607978581b45d7cca9c9584fdae0e4662ec4247d1d74f9337bb4123a18cb949b` |
| `race-se2` | the combo **plus** `se2-rigidity-certificate`, this tree | `a05b70fb29d5b024bb9c6d6c2fd8a26350b82090f2fc6d64a981e3ece7c3098d` |

`race-se2` exists because design C's certificate is behind its own feature and
the combo does not carry it. §5 is the only battery that uses it; §4 does not, so
the race numbers are on the protocol's own feature set.

## 7. The four pinned gates, and the whole document

`drivers/gates.py`, `evidence/gates-race.json`, `evidence/gates-base.json`.

| gate | pinned | race binary | base binary | doc digest |
|---|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | hit, 28.72 s | hit, 26.51 s | `e24b8451...` **identical** |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | hit, 3.38 s | hit, 3.39 s | `5f8186b7...` **identical** |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | hit, 3.71 s | hit, 3.73 s | `de0c40b7...` **identical** |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | hit, 3.29 s | hit, 3.26 s | `2d2e93c3...` **identical** |

`ALL_PASS: true` on both. The digest is the whole document with the wall-clock
and provenance fields stripped, and **all four are byte-identical between the
base commit's binary and this tree's** — a much stronger statement than four
scalars reproducing.

## 8. Determinism, two processes

`drivers/determinism.py`.

| arm | budget | binary | cells | equal | misses |
|---|---|---|---:|---:|---|
| `race=3:1:3` | `work=40000000` | `race-combo` | 9 | **9/9** | — |
| `race=0` | `work=40000000` | `race-combo` | 9 | **9/9** | — |
| `race=3:1:3` | `plan=10000` | `race-combo` | 9 | 7/9 | shapes-17 s0, s2 |
| `race=0` | `plan=10000` | `race-combo` | 9 | 5/9 | mixed-61 s1, shapes-17 s0/s1/s2 |
| default | `plan=10000` | `base-combo`, triangle-20 only | 3 | 2/3 | triangle-20 s1 |

**The work-mode row is the gate this round's code is responsible for**, and it is
9/9 in both arms: a work budget is a function of counters and not of the clock,
so the race's decision, its eviction and its report are all reproducible across
processes.

Plan mode's claim is two claims — the two processes must choose the same
`portfolio.plan.units`, and given that the documents must be identical with
`planCalibration` stripped — and **every plan-mode miss above is a failure of the
first**: `planAgrees` is false in all six, so the two processes bought different
budgets and ran different searches. Not one is a document disagreement at an
agreed plan.

The base binary fails in the same manner, on the one fixture it was run on, so
the failure mode is `docs/experiments/calibrated-plan/` §7's ladder straddle
under load rather than anything this round changed. It is a weaker control than
it looks — three cells on one fixture, and its miss is a different cell from the
race arms' — which is why the **work**-budget row above and not the plan rows is
the gate this round stands on. See §10 caveat 3 for what the box was doing.

`evidence/determinism-work-raceon.json`, `evidence/determinism-work-raceoff.json`,
`evidence/determinism-plan-raceon.json`, `evidence/determinism-plan-raceoff.json`,
`evidence/determinism-plan-base-triangle20.json`.

## 9. Suites

`drivers/run-suites.sh`, run on the committed tree with **nothing else of this
round's running**, and the exit status read on the line after the redirect
rather than through a pipe — `cargo test ... | tee log` reports `tee`'s status,
which is how a red suite gets written up as green.

| suite | features | result | exit |
|---|---|---|---:|
| 1 | `jagua-experimental` | **1,268 passed, 0 failed**, 2 ignored | **0** |
| 2 | the full combo | **1,322 passed, 0 failed**, 2 ignored | **0** |

`evidence/suite-jagua.log`, `evidence/suite-combo.log`.

All eight of this round's regression tests also pass in the certificate build,
exit **0** — `drivers/se2tests.sh`, `evidence/tests-se2-certificate.log`, which
exists because of the next paragraph.

**One of the eight is not reachable from either suite.**
`a_witness_maps_back_onto_the_parent_state_slot_for_slot` is gated on
`se2-rigidity-certificate`, which the `jagua-experimental` suite does not carry
and the protocol's full combo does not either — the certificate has its own
feature, and §6 says why this round needed a third binary because of it. Saying
so is the point — a test that runs in no suite the protocol names is a test that
will rot, and the next round should either fold the certificate into the combo
or keep running that script.

## 10. Honest caveats

1. **The race's negative verdict is eighteen cells at ten seconds and three at
   thirty.** §4.6's thirty-second, top-2 arm is three cells with **one**
   equal-work row, which is thin: it agrees with the ten-second result on the
   pick rate (0/3) and on the sign, and it is not on its own a thirty-second
   verdict. The claim this round makes is about the ten-second envelope and
   about the shape of §4.3's criteria.
2. **0 of 18 is 18 cells, not 18 independent draws.** shapes-17 is saturated at
   200.349 on every arm, so six of the eighteen carry very little information
   about the pick. The discriminating evidence is the mixed-61 and triangle-20
   arm rows in §4.3.
3. **The box was contended throughout the measurement pass.** Another agent's
   gates, batteries and test suite ran concurrently on the same sixteen cores,
   at a load average of 7.4–11.4. Two consequences, both stated rather than
   smoothed: the **wall** columns in §4.2 are contended-box readings, and the
   **plan-mode** determinism rows in §8 are load-sensitive by construction —
   plan mode reads the clock once, so a busier box moves `probe_seconds` and
   with it which rung of the ladder a process lands on. An earlier pass in this
   session reproduced more of those cells; its JSON was overwritten by the final
   pass, so that reading is **not** evidenced here and is not relied on
   anywhere. What is evidenced and load-independent: the **work**-mode
   determinism (§8), the four gate digests (§7), §2's and §5's work-capped
   counters, and §4.4's two *ratios*. Those carry every claim this round makes.
4. **Four of the nine draw-battery cells and three of the nine archive-battery
   cells are not equal-work,** because the plan ladder straddled. They are shown
   in the tables and excluded from every aggregate, and §4.2 declines to quote a
   pooled median for exactly this reason.
5. **`basin_race_share` does not bound the race's wall,** and §4.4 explains why it
   structurally cannot under a work budget. The code checks affordability the way
   the queue does; that check is near-inert here and says so at the site.
6. **The witness A/B is twelve parents on one request** — mixed-61 only, one trust
   radius, one iteration count. The 2/12 refutes a 0/12 stopping rule; it does not
   establish a rate.
7. **The three criteria were implemented as Sol specified them and one of them
   has zero variance on this workload.** Binding-front stability may well
   discriminate on a request whose confirmations sometimes fail; none of these
   three is such a request, and this round did not go looking for one.
8. **Nothing here touches the record lineage.** The `''` 0.0005 contract, the
   record 155/164 line and the four pinned gates are untouched, and §7 shows the
   gate documents are identical.

## 11. Reproducing this

```
bash docs/experiments/basin-race/drivers/build.sh   [BINDIR]            # 6
bash docs/experiments/basin-race/drivers/collect.sh [BINDIR] [OUTDIR]   # 2, 4, 5, 7, 8, 9
```

Those two are the whole round. `collect.sh` runs every battery in the order they
have to run and the order is not alphabetical: the wall-sensitive plan-mode
batteries first and alone, then the work-capped ones, then the work-mode
determinism gate, then both suites — which saturate every core and would have
made everything before them a measurement of the box.

Each driver also takes the binary as an argument, so a paired A/B can hold two of
them side by side:

```
D=docs/experiments/basin-race/drivers
P=docs/experiments/parallel-compression-schedule/evidence/parents.json
F=mixed-61,shapes-17,triangle-20

python3 $D/gates.py        race  RACE_GATE_BIN OUT/gates/race                      # 7
python3 $D/attribution.py  OUT/attr    SE2_BIN   $P                                # 2.1
python3 $D/witnessab.py    OUT/witness SE2_BIN   $P 0.025:64:2                     # 5
python3 $D/racebattery.py  OUT/draw    COMBO_BIN $F 0,1,2 10000 3:1:3              # 4.2
python3 $D/racebattery.py  OUT/arch    COMBO_BIN $F 0,1,2 10000 3:1:3,racedraw=0
python3 $D/determinism.py  OUT/det-w   COMBO_BIN $F 0,1,2 work 40000000 race=3:1:3 # 8
bash    $D/run-suites.sh                                                           # 9
bash    $D/se2tests.sh                   # all 8 of this round's tests, in the # 9
                                          # build that carries the certificate
python3 $D/smoke.py        COMBO_BIN mixed-61 0 10000 3:1:3      # one cell, printed
python3 $D/summarize.py    race|witness|attribution|gates|determinism|price  PATH...
```

`drivers/summarize.py` regenerates every table above straight out of the JSON,
because a table typed by hand from a JSON file is a table that can disagree with
it. `drivers/runlib.py` and `drivers/gatelib.py` carry the pinned CLI tail, the
`0.002` allowance, the salt sets and the request table, with `ROOT` pointed at
this worktree.
