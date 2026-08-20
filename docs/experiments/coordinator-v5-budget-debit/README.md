# Coordinator v5, item 1: the self-meter debits the budget it prices

Two rounds live in this directory.

* **Round 1** (`66060f1`) wrote the debit. Its measurement section was wrong,
  and the "Correction round" section below says how wrong and why. Its evidence
  files are kept, unaltered, under `evidence/` and indexed as superseded by
  [`evidence/SUPERSEDED.md`](evidence/SUPERSEDED.md).
* **Round 2** (this one) applies Sol review 6 §1 in full: the ordering
  correction, the arithmetic correction, the six tests, the authentic v4 rerun
  at 40M and 120M, the wall curves, and the retraction. Its evidence is under
  [`evidence/round6/`](evidence/round6/).

## The bug, and the two things wrong with the first fix

`schedule_self_cost_units` (`portfolio.rs`) prices one compression-schedule
slice in the portfolio's own work currency, because the coordinator's global
counter systematically under-reads that operator: on the twelve gate cells the
same self-capped arm reads **307,767–3,343,739** units on the coordinator's
meter — an **11x spread** — against **3,341,665–3,356,020** on its own, a
spread of 0.4%. Round 1 charged the larger of the two into `BudgetMeter`, so
`spent_fraction`, `remaining_to` and every affordability check finally saw the
honest price.

Sol review 6 §1 accepted the idea and rejected the execution on four counts.
Two were about the code:

* **Ordering.** `run_operator` archived the layout, published it and wrote the
  call report *before* the self-cost was returned to `v3_loop`, which is where
  the debit was applied. So the archive's `birth_work_units`, the publication's
  `work_units` and `OperatorCallReport::work_units` for the action that
  incurred a charge were all stamped without it — while every *later*
  publication carried it. Work appeared on the anytime timeline one action
  after the action that spent it.
* **Arithmetic.** `debit_self_metered(global_meter_delta: f64, …)` put a
  53-bit mantissa between an exact counter and a budget compared against it,
  and returned nothing, so no caller could report what had been applied.

Two were about the evidence, and are treated in "The correction round" below.

## What the code does now

Execution of one deep-operator call is a four-step transaction, and the order
is the fix:

1. **dispatch** — `dispatch_persistent_vacancy_mode` runs the operator;
2. **charge** — `global_units` is read as the global counter's own delta,
   and `operator_self_metered_units(&population)` asks the operator what it
   charged itself (`Some` only for mode 34, and only under the
   `compression-schedule` feature);
3. **debit** — `settle_operator_charge` calls
   `BudgetMeter::debit_self_metered`, which is now
   `operator_self_units.saturating_sub(global_meter_delta)` in `u64`
   throughout, **returns the extra it applied**, and is a no-op that returns
   zero under a wall budget — the guard living in the method that owns the
   accumulator rather than at the call site, so a future caller cannot forget
   it;
4. **stamp** — and only now `archive_layout`, `try_publish` and the
   `OperatorCallReport` read the meter. Every one of them is therefore a
   reading of a settled budget.

Sol allowed a fallback — leave the ordering alone and report `globalUnits`,
`selfUnits`, `debitedUnits` and fix the timestamps post hoc. This round did the
transaction, which Sol preferred, **and** added the three explicit fields
anyway, on `OperatorCallReport` and on `ScheduledActionReport`: they are what
makes the ordering claim measurable from the outside rather than arguable from
the source, and the whole finding about this operator is the *gap* between the
two meters, which a document carrying only their maximum would hide.

One deliberate consequence beyond the ordering, called out because it is a
behaviour change and not only a reporting one: `OperatorCallReport::work_units`
is now the settled charge, so `BudgetMeter::call_cost` — and through it
`mean_operator_cost`, which the affordability rule consults — prices a *future*
mode-34 call at what the last one really cost. Under `jagua-experimental`
alone, where mode 34 does not exist, every one of these numbers is identical to
the old one by construction, which the gates below confirm as whole documents.

### The six tests Sol named

All in `portfolio.rs`'s test module. All six run in *both* feature
configurations — none of them is `#[cfg]`-gated, because
`settle_operator_charge` and `BudgetMeter::debit_self_metered` exist in both
and the whole point is that the wall/no-operator behaviour is compiled and
checked everywhere:

| Sol's requirement | test |
|---|---|
| global 30 / self 50 → spent 50 | `a_self_meter_above_the_global_counter_is_what_gets_spent` |
| global ≥ self → no extra | `a_global_counter_at_or_above_the_self_meter_debits_nothing` |
| two consecutive actions | `two_consecutive_self_metered_actions_both_land_on_the_budget` |
| saturation | `the_debit_saturates_rather_than_wrapping` |
| wall no-op | `a_wall_budget_never_debits_a_self_meter` |
| publication/archive/report include the current action | `the_current_actions_debit_is_already_on_the_meter_when_it_is_stamped` |

Two more were added: `the_schedules_own_report_is_the_self_metered_reading`
(the wiring the other six assume — the one test here that *is*
`#[cfg(feature = "compression-schedule")]`, since without the feature there is
no `schedule_self_cost_units` to name) and
`a_debited_call_is_priced_at_the_self_meter_for_the_next_one` (the `call_cost`
consequence above).

**What the sixth test does not do, stated rather than glossed.** It exercises
the settlement, not `run_operator`'s source order — reaching a real
`archive_layout`/`try_publish` from a unit test needs a whole engine run whose
mode-34 arm fires, which no unit test in this module can afford. The
end-to-end half is measured instead, on real run documents, by
`drivers/orderingcheck.py` and `drivers/stampdelta.py`; see "Finding 4,
measured" below. Both are discriminators, not plausibility checks.

## The correction round

### 1. The first round's battery validated nothing, and this is the retraction

Sol review 6 §1 finding 1 is correct, and the primary evidence is one field.
`evidence/battery-fixed-sched.json`'s only arm carries `"v3": false`, which the
driver renders as the spec `work=120000000,cells=13:15:17:19,v3=0` — visible in
every row. With `v3=0` the coordinator's v3 loop does not run, so the schedule
class does not run, so mode 34 does not run, so `schedule_self_cost_units`
returns nothing and `debit_self_metered` is never called. The depths
174.208 / 176.056 / 179.006 are correct numbers about runs that executed no
part of the code under test.

Everything the first round's README and the plan chapter concluded from that
battery is withdrawn. In particular:

* *"a paired baseline-vs-fixed 3-seed work=120M battery … produced identical
  depths in all four combinations"* — true, and empty: all four combinations
  had the code switched off.
* *"No headline number moved in either direction"* — **false when the code is
  actually run.** It moves. See below.
* *"every run in reach stopped on its own priced queue before the debit could
  be the deciding factor"* — false. Under the true v4 configuration the debit
  is the deciding factor at 40M on every seed, and it is what makes the run
  stop.

The plan chapter has been marked corrected in place rather than edited, and a
new chapter carries the real result.

### 2. The authentic v4 rerun

Configuration: `v3=1,sched=1,barren=16,divq=1` on a
`jagua-experimental,compression-schedule` build, mixed-61 from the bare
request, allowance `0.002`, seeds 0/1/2, three paired interleaved rounds,
`fixed` (this branch) against `unfixed` (`f32c629`, before the debit existed).
Depth at fixed work is deterministic, and the three rounds confirm it: every
cell reproduced its own depth exactly in all three.

**On `barren=16` rather than Sol's `barren=1`.** Three of those four spec keys
are booleans; `barren` is not — it is parsed as a `usize` patience
(`general_request_benchmark.rs`), and v4's value is
`BARREN_ACTION_PATIENCE = 16` (`portfolio.rs`). `barren=1` is a queue sixteen
times more impatient than v4, which is not the v4 configuration. The main
battery runs `barren=16`; the literal reading was run separately so the round
cannot be accused of picking the more convenient of two readings
(`evidence/round6/battery-barren1-40000000.json`).

#### 40M: the debit binds on every seed, and costs 4.376 mm on one

Sol predicted this ("il debit vincola già entro 40M") and it is what happened.
Per-cell table: `evidence/round6/table-work-40000000.md`.

| seed | fixed | unfixed | Δ (unfixed − fixed) | fixed actions | unfixed actions |
|---|---|---|---|---|---|
| 0 | 169.891 | 169.891 | 0 | 12 | 13 |
| 1 | 171.362 | 171.362 | 0 | 10 | 11 |
| 2 | **170.155** | **165.779** | **−4.376 mm** | 12 | 15 |

Depth is a minimum: **lower is better**, so a negative Δ is the unfixed arm
winning. Every one of the three seeds buys strictly *fewer* actions with the
fix, so the debit is load-bearing on all of them; it costs depth on one. Median
Δ over the nine paired cells is 0.0 mm, mean −1.459 mm, worst cell −4.376 mm.
All three rounds reproduced their own cell exactly, so this is deterministic,
not noise.

**This is a quality regression at fixed nominal work, and it is the correct
behaviour.** The mechanism is legible in the two seed-2 action sequences, which
are identical action for action and metered unit for metered unit through
iteration 10 and then diverge:

| iter | class | fixed metered | fixed debit | unfixed metered | depth after |
|---|---|---|---|---|---|
| 3 | schedule | 1,366,456 | 2,090,715 | 1,366,456 | 177.343 |
| 6 | schedule | 1,401,670 | 1,998,160 | 1,401,670 | 174.881 |
| 9 | schedule | 998,474 | 2,252,965 | 998,474 | 172.707 |
| 10 | schedule | 1,084,524 | 2,470,770 | 1,084,524 | 171.150 |
| 11 | compression / schedule | 2,338,593 | 0 | 1,202,193 | 170.155 / 169.570 |
| 12–14 | schedule ×3 | — stopped — | | 1,085,911 / 3,290,233 / 3,568,894 | 165.779 |

At iteration 11 the fixed run has spent 39.1M of its 40M and stops on
`affordability`. The unfixed run's meter reads about 30M at the same point,
so it buys four more schedule slices — every one of which publishes — and
reaches 165.779.

The under-read is not a gate-cell curiosity. Across the nine fixed 40M runs
there are **21 self-metered mode-34 calls**, of which **18 carried a debit**;
median self-metered reading **3,398,525** against a median global delta of
**1,083,234**, a 3.1x under-read in the wild. The three calls that debited
nothing are the `global ≥ self` case occurring naturally — the branch the
second unit test pins.

#### What the unfixed arm actually spent

The unfixed binary still reports `actualCost` (the ranked price) and
`meteredCost` (the counter delta) per action, so its thrown-away debit is
recoverable: `drivers/truecost.py` sums the difference. On the 40M battery:

| seed | unfixed reported | unfixed **true** | overrun | fixed reported = true | fixed overrun |
|---|---|---|---|---|---|
| 0 | 39,309,265 | **41,805,185** | +4.5% | 38,857,134 | −2.9% |
| 1 | 38,518,915 | **41,188,355** | +3.0% | 35,773,579 | −10.6% |
| 2 | 37,102,965 | **51,328,640** | **+28.3%** | 39,106,937 | −2.2% |

Sol computed 41.19M / 41.81M / 51.33M from the pinned v4 trace. These are
independent runs on a different commit and they land on the same three numbers
to five significant figures. **9 of 9 unfixed runs overran their budget; 0 of 9
fixed runs did.**

So the 4.376 mm is not a like-for-like loss: at 40M nominal the unfixed arm was
spending 51.3M. The 165.779 was bought with 28% more work than the run was
given, and the fix is what stops that.

#### Finding 4, measured

Two checks on real run documents, both discriminators rather than plausibility
arguments.

* **`drivers/orderingcheck.py`** — inside one document, every debited call must
  satisfy `workUnits == globalUnits + debitedUnits`. The pre-fix ordering
  computed `work_units` from the meter *before* the debit and so could only
  ever emit `workUnits == globalUnits`. On the 40M fixed runs: 6 debited calls,
  **6/6 exact, 0/6 pre-fix**, and all 6 publications and all 6 archived basins
  carry a reading at least the cumulative debit through their own call.
* **`drivers/stampdelta.py`** — across the paired arms. The two arms are
  bit-identical prefixes, so for any layout both produced, the global counter
  read the same when it was stamped and the whole stamp difference is the
  debit. The corrected ordering predicts
  `fixedStamp − unfixedStamp == cumulative debit *through this call inclusive*`;
  the pre-fix ordering predicts the *exclusive* sum, which differs by exactly
  this call's own debit. Over the nine paired 40M cells: 18 debited calls,
  36 comparable stamps (publication and `birthWorkUnits` each),
  **36/36 match the corrected identity, 0/36 match the pre-fix identity.**

Worked instance, seed 2, publication stamps: debits 2,090,715 / 1,998,160 /
2,252,965 / 2,470,770 produce stamp deltas 2,090,715 / 4,088,875 / 6,341,840 /
8,812,610 — the running total including the current call, every time.

#### 120M: nothing moves

| seed | fixed | unfixed |
|---|---|---|
| 0 | 163.927 | 163.927 |
| 1 | 162.161 | 162.161 |
| 2 | 164.004 | 164.004 |

Nine of nine paired cells equal, all three rounds identical. The debit is still
very much alive here — 57 self-metered calls, 54 of them debited, 89,583,915
units charged in total across the nine fixed runs — it simply does not change
which action the queue can afford by the time the run ends. Sol's 120M
counterfactual for the unfixed arm was 122.36M / 121.61M / 126.52M; measured
here: **122,358,786 / 121,613,866 / 126,516,058.**

Ordering checks at 120M: 54 debited calls, **54/54** satisfy
`workUnits == globalUnits + debitedUnits`, 0/54 the pre-fix identity; 96
comparable paired stamps, **96/96** the corrected identity, **0/96** the
pre-fix one.

> **A driver bug found and fixed while checking this.** The first version of
> `stampdelta.py` scored 6 of 102 rows at 120M as matching the *pre-fix*
> identity. They were not an ordering defect: all six were the same call on
> seed 1 across three rounds, archived `Duplicate` and publishing nothing, so
> the stamps under its fingerprint belong to the *earlier* call that produced
> that layout first — whose cumulative debit is smaller by construction. The
> driver now excludes calls that are not the first producer of their
> fingerprint and counts them under `skippedDuplicate` (3 at 120M, 0 at 40M).
> The number is reported rather than quietly dropped.

#### The equal-true-cost control: at equal work the fix costs nothing

The 40M comparison is not like-for-like — the arms did different amounts of
work. So: give the fixed arm 52M, which is above every unfixed *true* spend at
the 40M point, and compare against the unfixed 40M runs at matched true cost.

| run | nominal | **true** work | depth |
|---|---|---|---|
| unfixed, seed 2 | 40M | **51,328,640** | **165.779** |
| fixed, seed 2 | 52M | **51,339,455** | **165.779** |

Two runs whose true work differs by 10,815 units — 0.02% — land on **exactly
the same depth**. The entire 4.376 mm at 40M nominal is the 11.3M units of work
the unfixed run took and was not given. The fix does not cost search quality;
it costs the ability to spend work the budget did not authorise.

The unfixed arm at 52M overruns again, of course — true 56,146,023 (+8.0%) and
64,329,575 (+23.7%) on seeds 0 and 2 — which is the same finding at a second
budget.

#### `barren=1`, the literal reading, run so nobody has to take my word for it

| seed | fixed | unfixed | self-metered calls |
|---|---|---|---|
| 0 | 179.587 | 179.587 | 0 |
| 1 | 179.633 | 179.633 | 0 |
| 2 | 179.006 | 179.006 | 0 |

With a barren patience of 1 the queue never reaches the schedule class at all:
**zero self-metered calls in any run**, so the debit is never exercised and all
three cells are identical — and the depths are 8–10 mm worse than v4's. Running
the battery this way would have reproduced the previous round's mistake in a
new costume: an arm that never executes the code under test. This is the
evidence for reading Sol's `barren=1` as "the barren audition on" and running
v4's actual `BARREN_ACTION_PATIENCE = 16`.

### 3. The wall curves

3 s / 10 s / 30 s on mixed-61, same v4 spec, 3 seeds × 3 paired interleaved
rounds per point, 54 runs. **What these can and cannot show was decided before
running them**: under a wall budget `debit_self_metered` returns zero by
construction and `work_units_now()` is a constant, so the two binaries make
identical decisions on this path. The curve is not a search A/B — it is the
end-to-end version of `a_wall_budget_never_debits_a_self_meter`. If a depth
moved for a reason, the no-op claim would be false.

| point | cells equal | mean Δ (unfixed−fixed) | fixed arm: mode-34 calls / self-metered reported / **debited** |
|---|---|---|---|
| 3 s | 9 / 9 | 0.000 mm | 0 / 0 / **0** |
| 10 s | 7 / 9 | +0.703 mm | 6 / 6 / **0** |
| 30 s | 6 / 9 | +0.187 mm | 24 / 24 / **0** |

**Thirty self-metered mode-34 calls fired under a wall budget across these runs
and not one of them debited a unit.** That is the no-op observed rather than
argued.

The cells that differ are the shared box, not the change, and the numbers say
so: a wall-budget run is not reproducible, and the *within-arm* round-to-round
spread dwarfs the between-arm difference.

| point | within-arm spread (mean / max) | between-arm paired \|Δ\| (mean / max) |
|---|---|---|
| 3 s | 0.000 / 0.000 mm | 0.000 / 0.000 mm |
| 10 s | **5.709 / 8.271 mm** | 0.703 / 3.479 mm |
| 30 s | **4.276 / 6.054 mm** | 0.455 / 2.848 mm |

The same binary, same seed, same budget, run three times, moves by up to 8.3 mm
at 10 s. Against that, a 0.7 mm mean between two binaries that are provably
identical on this code path is nothing. Process wall medians agree to within
1%: 2.964 vs 2.892 s, 9.861 vs 9.854 s, 29.095 vs 29.102 s. Load average over
the run was 3.8–13.3 with two sibling agents active.

*A reporting caveat.* The batteries' `debit.selfMeteredCalls` counts calls that
*report* a self-metered reading, and only the fixed binary emits that field —
so the unfixed arm shows 0 everywhere by construction, not because mode 34 did
not run. Counted directly, the unfixed arm made 4 mode-34 calls at 10 s and 20
at 30 s.

## Gates, suites, and what is still broken

### The four pinned regression gates

Driver: `drivers/gates.py` over `drivers/gatelib.py`, which is
`docs/experiments/constructor-inner-certificate/drivers/lib.py` with the `ROOT`
line repointed at this worktree and nothing else changed. Binary:
`cargo build --release --example general_request_benchmark --features
jagua-experimental`, rebuilt from the tree as committed.

| gate | pinned depth | pinned fingerprint prefix | hit |
|---|---|---|---|
| g1 (m20) | 206.869 | `8a7737381238fa4d` | yes |
| g2 (m22) | 159.09233022733062 | `fa01012af1d559ae` | yes |
| g3 (m22) | 159.07876040364795 | `e28fba007f8031d4` | yes |
| g4 (m22) | 164.0375677990678 | `49f094d7e59a9008` | yes |

`ALL_PASS: true` (`evidence/round6/gates-fixed.json`). Stronger than the four
scalars: `drivers/gatedocdiff.py` compares the *whole* gate document between
the fixed and the unfixed binary, with build-identity and clock fields stripped,
and all four are byte-identical — `ALL_IDENTICAL: true`
(`evidence/round6/gates-docdiff.json`). Under `jagua-experimental` alone mode 34
does not exist, `operator_self_metered_units` is `None` on every call, and the
debit is provably inert; this measures that rather than asserting it.

### Both suites

The protocol's suite, and the feature combination this change's only live path
compiles under — Sol flagged the missing combo suite, and both are run:

| suite | exit | binaries | passed | failed |
|---|---|---|---|---|
| `cargo test --release --features jagua-experimental` | `EXIT_JAGUA=0` | 55 | 1257 | 0 |
| `cargo test --release --features jagua-experimental,compression-schedule` | `EXIT_JAGUA_SCHED=0` | 55 | 1272 | 0 |

Logs: `evidence/round6/suite-jagua.log`, `evidence/round6/suite-jagua-sched.log`.
The known-flaky
`search::layout_scorer::tests::free_material_multi_eviction_shrinks_retained_container_capacity`
passed on the first attempt in both; no rerun was needed. The 15-test
difference is the `compression-schedule` module's own tests plus this round's
one `#[cfg]`-gated test.

### What this fix still does not do

* **It does not bound a single action.** The debit is charged *after* the
  action it prices, so one indivisible action can still carry a run past its
  cap. Stated completely rather than by its best cell:

  | battery | fixed runs over budget | worst | unfixed runs over budget | worst |
  |---|---|---|---|---|
  | 40M | **0 / 9** | −2.2% | 9 / 9 | **+28.3%** |
  | 120M | **3 / 9** | **+1.23%** | 9 / 9 | **+5.43%** |
  | 52M | **0 / 6** | −1.3% | 4 / 6 | **+23.7%** |

  The three 120M overruns are the same seed-1 cell in all three rounds
  (121,474,651 against 120,000,000). The fix turns an unbounded overrun into
  one bounded by a single action's own debit; removing it needs preflight or
  p95 pricing, shorter quanta, or a deadline-aware batch — Sol review 6 §1
  finding 3.
* **It is not a wall-clock guard.** No-op under a wall budget by construction,
  so it is **not** what would have caught the 2/27 wall overruns. That is a
  design decision, not an oversight: seconds are seconds and the clock has no
  broad phase for an operator to ride free on.
* **It makes the schedule class more expensive, and the class was earning its
  keep.** On seed 2 at 40M, every extra slice the unfixed run bought published.
  The right response to that is item 2's eligibility prior deciding *whether*
  to schedule mode 34 — not making its price a lie again.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1` |
| branch | `wf48/coordinator-v5-debit-corrections` |
| base | `wf48/coordinator-v5-budget-debit` @ `66060f1` (which is based on `f32c629`) |
| salvage inspected | `worktree-wf_b7992967-b13-1` @ `3c849c9`, the interrupted attempt at this same task; its `portfolio.rs` transaction and its drivers were reused after review, its unverified battery leftovers were not |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json` |
| from-request allowance | `0.002`; record lineage `0.0005` |
| gate binary (`jagua-experimental`, fixed) | sha256 `f53be9ea82e6d75ebbc03f207c08d8bfc960c04da385e21f2ae7cb265a9d3a5a`, rebuilt from the committed tree and used for the gate numbers reported above |
| measurement binary (`jagua-experimental,compression-schedule`, fixed) | sha256 `62ecd44d618cdea4a663b41967f0aa3a4ace2103a250b19f1c6424fa0f683377`. This was built *before* the round's last two edits — a doc comment and two added `#[cfg(test)]` tests — so it is not byte-identical to a build of the committed tree (`1e8217f6…`). Rather than argue that neither edit can reach generated code, `drivers/verify-measurement-binary.sh` rebuilds and re-runs the cell the headline rests on: seed 2 at 40M reproduces 170.155, 39,106,937 work units and an 8,812,610 debit, and every field of the run document matches except the wall-clock seconds — the original ran at load 20+ in 36.2 s, the rebuild nearly idle in 19.9 s. `evidence/round6/measurement-binary-rebuild.json`. |
| measurement binary (unfixed, `f32c629`) | sha256 `ddb7d7468166fae3205d973260712dfa135c068774f0bf8d09e45f654bc8e9e4` — byte-identical to the sha the first round recorded for the same commit and feature set, which is an independent check that the baseline arm is the commit it claims to be |
| box | x86_64, 16 cores, shared with two sibling agents throughout; load average observed between 11 and 26 |
| raw run documents | `/var/lib/t3/tmp/wf6v1/` on this box (ninety runs, 100–500 KB each; not committed) |
