# The re-plan, and a mode 34 that can stop

> ## Correction, 2026-08-21 — §12.3 and the `m34cap` headline row are retracted
>
> **`m34cap` did not stop anything at the HEAD this round measured, and this
> document's claim that it did is withdrawn.** Sol review 9's P0 names the line:
> `ScheduleSliceRun::advance` recorded a checkpoint and left `finished = false`,
> and its caller looped `while !slice.finished` to the end of the monolith, so
> **the coordinator never regained control at a checkpoint.** The cap changed the
> checkpoint *report* and nothing else.
>
> The retraction has been reproduced rather than argued.
> `docs/experiments/real-interruption/` §2 replays `m34cap=0` against
> `m34cap=1` at `work=30000000` on mixed-61 seeds 0/1/2, on a binary built from
> the committed base tree: **identical raw depth, identical incumbent
> fingerprint, identical total work, identical operator-call count and identical
> per-slice step digest on all three seeds.** Seed 1 is 171.3619986855876 mm at
> 28,636,653 units over 8 operator calls on both arms. The whole document is
> equal once the `scheduleSlice` block is dropped, and unequal only because of
> the checkpoint list.
>
> Specifically withdrawn:
>
> * **the headline row** *"The checkpoint's consumer — `m34cap=1` at thirty
>   seconds: p50 32.64 s → 25.91 s, overruns 4 of 6 → 2 of 6, for 3.1 mm on one
>   seed"*;
> * **§12.3 in full**, including *"It works, and it is priced"*, the 162.846 →
>   165.935 mm attribution on seed 1, and *"That is the whole argument for the
>   batch mechanism in one table"*.
>
> The wall numbers in that table are real measurements of real processes. What
> does not survive is the attribution: they are two runs of **one trajectory**,
> and §7 of this same document already records that the box was not quiet.
>
> There is a second, independent defect, and it is a provenance break rather
> than a semantic one: **the committed driver cannot generate the arms whose
> specs the evidence file carries.** `drivers/trancheq.py:44` parses any
> fraction other than `off` as `replan=1,planfirst=<value>`, so `capon` would
> have produced `replan=1,planfirst=capon` — not the `replan=1,m34cap=1` that
> appears in every `spec` field of `evidence/cap-30s.json`. Source, driver and
> measured binary do not agree. `evidence/cap-30s.json` now carries a
> `SUPERSEDED` block saying so.
>
> **What replaces it.** `docs/experiments/real-interruption/` makes the
> interruption real: `advance_one_batch() -> Checkpoint|Finished` returns to a
> driver that consults a policy at every checkpoint, and the coordinator may
> stop with the exact-valid incumbent the checkpoint holds, or suspend the slice
> alive and resume it after another action. Everything else in this document —
> the concatenation gate, the struct, the step digest, the re-plan, the
> quiet-box retraction of `calibrated-plan` §8.2 — stands.

Two mechanisms, one budget, and they are the same idea at two scales.

`docs/experiments/calibrated-plan/` made the ten-second number reproducible and
priced what that cost: **+6.904 mm** on mixed-61 at ten seconds, decomposed into
a conservative bias constant (3.741), the work counters (1.882) and the
quantisation floor (1.281). Its §13.1 named the fix for the largest of the three
and declined to build it -

> Install a provisional plan from phase 0, run to a deterministic work
> checkpoint, then re-price the remaining wall at the rate the *queue* is
> actually retiring units at [...] It is not in this round because it is not
> free: `v3_loop`'s `run.deadline` and `Coordinator::protected_fraction` are
> both fractions of the plan that was installed when the phase was entered.

Sol review 8 §3 condition 4 named the other half, one level down: **mode 34 is
atomic and has no internal work cap**, so a coordinator that has decided to stop
cannot; and §4 spend 1 named the gate - *N concatenated batches must reproduce
the monolith at equal work.*

This round builds both, and the join is the point: **the deterministic work
checkpoint §13.1 needs is the batch boundary Sol asked for.**

---

## The headline

| | |
|---|---|
| **Sol's concatenation gate** | **1,741 batches over 21 cells, three batch sizes and two budgets: every cell equal as a whole document *and* as a per-step digest** (§10) |
| **The resumable slice** | `m34batch=<units>`: the slice stops at a checkpoint holding an exact-valid incumbent and resumes with frontier, deepest-confirmed slot, rng, weights and every surrogate cache intact |
| ~~**The checkpoint's consumer**~~ | ~~`m34cap=1` at thirty seconds: p50 **32.64 s → 25.91 s**, overruns 4 of 6 → 2 of 6, for 3.1 mm on one seed (§12.2)~~ **RETRACTED — see the correction at the top of this file.** `m34cap` could not stop the slice: the caller looped to the end of the monolith, so the coordinator never regained control. The two arms are one trajectory. |
| **The in-run re-plan** | `replan=1` recovers **2.808 mm on mixed-61 seed 1 at ten seconds - exactly `calibrated-plan` §9's ladder-floor cost for that seed** - and 0.252 mm on the median of seed medians, at the same overrun count (§11) |
| **What it does *not* fix** | the thirty-second overrun. The `plan` arm ran **41.15 s** against a 30 s target and re-planning brought it to **37.14 s**: reduced by 4 s, not removed (§12.1) |
| **A retraction-grade finding** | `calibrated-plan` §8.2's *"one plan, one depth, one document per seed, 60 of 60"* is a **quiet-box** property. Re-measured under a competing workload the same arm produced **2 / 3 / 1** distinct depths per seed (§11.1) |
| **Two bugs shipped and caught** | an unbounded extrapolation that turned a 30 s target into 36.74 s (§9.1) and a stranded run that left 5.7 s of ten unspent (§9.2), both found by this round's own gates |
| **Everything ships off** | `replan`, `m34batch` and `m34cap` all default to the previous round's behaviour |
| **Gates** | 4 of 4 pinned, whole-document digests identical to the base binary; refactor equivalence 9/9; both suites pass |
| **The box** | **not quiet** for part of the window. §7 is that caveat and §14 carries it |

---

# Part I — the resumable slice

## 1. What was atomic, and what a batch is

`drive_compression_schedule` was one function with one loop and about twenty
`let mut` bindings around it. It entered at the parent, walked its clamp down
one canonical grid unit at a time, and returned when its bound, its step budget
or its work cap stopped it. There was no other exit. Sol review 8 §3 condition 4
is one sentence about that shape - *"mode 34 oggi è atomico e senza work cap
interno"* - and it matters for two different reasons:

* **the coordinator cannot stop.** A slice is dispatched on a *price estimate*
  and charged when it returns, so a budget with 2 M units left can dispatch a
  slice that spends 20 M and the affordability rule finds out afterwards.
* **the wall promise is only as good as that estimate.** Every overrun in
  `calibrated-plan` §10.2 is an action that was in flight when the deadline
  passed.

A **batch** is the same loop, stopped at a **checkpoint**. A checkpoint is a
step boundary, and the property that makes it the right granularity is that at
one the slice always holds an **exact-valid layout it may hand back**:
`published_depth_mm` starts at the parent - which the caller validated before
dispatching - and only ever moves onto a layout the exact confirmation accepted.
So "stop here and keep the incumbent" is always available, which is Sol review 8
§3 condition 3's anytime contract in the work currency rather than in seconds.

The checkpoint carries the deepest-confirmed slot explicitly:

```
{ batch, stepsTaken, workUnits, frontierMm, floorMm,
  confirmationsAccepted, publishedDepthMm, finished }
```

## 2. Why the implementation is a struct, and why that is the load-bearing part

The risk Sol names in the same sentence as the gate is *"batching or cache
reconstruction changes the trajectory"*. A gate can only find that if the
implementation is capable of it, so the implementation is deliberately the one
that is: `ScheduleSliceRun::advance` **returns to its caller** between batches,
and everything the next batch reads has to be a field on the struct.

Three groups of fields, and the boundaries are the design:

* **the request** - fixed for the whole slice, read by every batch, written by
  none;
* **the frontier and its caches** - `search`, `repair_workers`, `state`,
  `score`, `schedule` - the part a naive "rebuild and resume" gets wrong,
  because a worker's surrogate and pair-NFP caches are what make its second step
  cheaper than its first and its **rng** is what makes the second step the *same*
  step;
* **the account** - the step rows, the timings, the parallel report, the witness
  counters - carried so a batched slice reports one slice and not N.

Two things are deliberately **not** carried, and both are correctness
statements rather than omissions:

* **design B's `stall_loss`** is initialised at the top of every step in both
  arms, so it cannot cross a batch boundary because it does not cross a step
  boundary either;
* **the tail confirmation** - the one that catches a frontier the cadence never
  asked about - runs when the *slice* ends and never when a *batch* does. A
  batched slice that confirmed at every boundary would ask the exact tier N
  times where the atomic slice asks once, and would neither reproduce the
  monolith nor be honest about what it spent.

One asymmetry is worth stating because it is the only ordering decision in the
batch loop: the barren probe is tested **before** the batch boundary. A batch
boundary is a place the slice *may* be interrupted; the probe is a reason the
slice is *over*. The probe has to win, or a batched slice would carry a step
further than the atomic one.

## 3. The gate: N batches reproduce the monolith

Two instruments, because the obvious one is not enough.

`ScheduleSliceReport` - what the coordinator's document carries - is an
**aggregate**: it deliberately drops the per-step rows, which are thousands of
entries per call. A comparison made on it alone can say that two slices took the
same number of steps and reached the same depth. It cannot say they took the
*same* steps, and a batched slice that diverged at step 700 and re-converged by
step 1,616 would pass it.

So the slice computes a **step digest**: FNV-1a over every row - the step index,
the clamp, the sweeps, the candidate queries, the pair and boundary counts
before and after, the confirmation's three outcomes, and the raw depth an
accepted confirmation measured. Floats go in through `to_bits`. ~~Two slices with
the same digest walked the same walk.~~ It is reported as one scalar on **both**
arms, because the whole use of it is a comparison between them.

> **Corrected, 2026-08-21.** The struck sentence is false and
> `docs/sol-review-9-m34cap-provenance.md` §P1 is why: the payload carries the
> clamp, the counts and aggregate loss, and it carries **no** placement
> fingerprint, no pair identity, no weights, no RNG state and no winning lane -
> so *"due cammini differenti possono avere lo stesso payload senza alcuna
> collisione FNV"*. FNV-1a over that payload is an adequate regression
> checksum and not a certificate, which is all this section is entitled to
> claim for it.
>
> `docs/experiments/real-interruption/` §4 is the repair: three SHA-256
> fingerprints - the frontier's geometry, the tracker's per-boundary and
> per-pair state, and the lane's RNG position and guided weights - computed at
> the instant a slice ends, `#[serde(skip)]` so they exist in the gate and in
> no document. The concatenation gate asserts all three against the monolith at
> three batch sizes.

The gate is then: whole document equal **and** step digests equal, from the bare
request, at a pinned work budget, with the batch budget as the only difference
between the two specs.

## 4. Interruptibility, and what is deterministic about it

`m34cap=1` is the policy that consumes the mechanism: before dispatching a
slice the coordinator hands it `batch_work_units = remaining_to(deadline)`, and
the slice gives itself back at the first checkpoint past it with its last
exact-valid incumbent intact.

It is denominated in **work and not in seconds**, and that is the whole reason
it is worth having: the slice's own meter is a counter, so two processes stop at
the same checkpoint and the document is unchanged. Sol review 8 §3 condition 3
asks for a *wall* stop between checkpoints; a wall stop cannot be deterministic,
and this round does not ship one. What it ships is the mechanism that makes one
possible - the checkpoints exist, and a wall stop is now a policy over them
rather than a change to the operator.

The two currencies line up by construction: `settle_operator_charge` charges
`max(global_units, operator_self_units)`, so what the coordinator pays for a
slice *is* the slice's own meter whenever the slice's meter is the larger of the
two, which on the measured band it is by about 18x.

---

# Part II — the in-run re-plan

## 5. What a tranche is

```
plan=<ms>                     ->  the probe, phase 0, one clock read
  install_plan(first_tranche)     ->  Work { units }, on the ladder
  run_v3_tranche("schedule")      ->  spends it, exits on affordability
  replan()                        ->  ONE clock read: the queue's own rate
  run_v3_tranche("replan1")       ->  spends the top-up
  ...                             ->  until a tranche would not buy a rung
```

The arithmetic is three lines and the third is the one that matters:

```
queue_rate = (work_now - probe_work) / (seconds_now - probe_seconds)
horizon    = min(target*headroom - seconds_now, queue_seconds * HORIZON)
raw_units  = work_now + horizon * queue_rate        // no bias divisor
units      = floor onto anchor * step^k
```

**There is no bias term.** `PLAN_PHASE_ZERO_BIAS = 1.70` exists to correct phase
0's rate onto the queue's; a tranche measures the queue's rate directly, so the
estimator's bias is 1 by construction and its window is the whole of the run so
far rather than the protected phase.

`§13.1` prices two lines as the reason it left this undone - `v3_loop`'s
`run.deadline` and `Coordinator::protected_fraction` are fractions of the budget
installed when the phase was entered. This round does not patch them. Each
tranche recomputes `protected_fraction` from the new total and enters a **new
phase**, so every deadline a tranche runs against is a fraction of the budget
actually in force. The phases are visible in the report as `schedule`,
`replan1`, `replan2`.

Three guards, and each of them is a way this could have been dishonest:

* **the queue's policy state is carried across tranches.** Every `let mut` that
  used to live inside `v3_loop` - the barren patience, the sterile bit's single
  audition, the diversify slot - is now a `V3QueueState` the tranche loop owns.
  A tranche that reset them would give the compression-schedule class a fresh
  audition it has already failed and reset a patience coordinator v3 §4.2
  measured at eight. The budget grew; the run's history did not.
* **a tranche follows a budget exit and not a work exit.** A queue that stopped
  on `keysExhausted` will stop again for the same reason, and a report carrying
  five empty `replanN` phases would describe a run that re-planned five times
  when what happened is that it finished early.
* **`replan=1` under `work=` or `wall=` is inert**, because there is no plan to
  re-price.

And one guard that is a **regression test for a bug this round shipped and
caught**. At a three-second target phase 0 on mixed-61 is 2.2 s, so a first
tranche aimed at `0.6 * 3 * 0.97 = 1.75 s` is already behind by the time it is
computed. The first cut bought a plan of exactly the probe; the schedule phase
was then skipped, and the re-plan could not rescue it - **with no queue there is
no rate to measure**, so no tranche was taken and the run published phase 0's
own layout. A re-planning run was *worse than the mode it improves* at the one
budget where the margin is thinnest. The fraction now degrades to the whole
target when the probe has outrun it, and `plan.firstTranche` reports the
fraction that was **applied** rather than the one that was asked for, so a
reader can see the degrade instead of inferring it from a wall.
`a_first_tranche_the_probe_outran_degrades_to_the_whole_target` pins it.

## 6. What is deterministic about a tranche, and what is not

The same split `calibrated-plan` §5 makes for the plan, one level down, and it
has to be read carefully because this is the one place in the mode where a clock
reading can change *how many* decisions a run makes rather than only how big one
of them is.

| deterministic | a clock reading |
|---|---|
| `tranches[].index`, `.rung`, `.units` | `atSeconds`, `queueSeconds`, `queueRateUnitsPerSecond`, `remainingSeconds`, `horizonSeconds`, `rawUnits` |

`trancheCalibration` is stripped from every digest in this round for exactly the
reason `planCalibration` is; `tranches` is **kept**, because two processes that
took different tranches ran different searches and the digest must say so.

The clock's influence is bounded by the ladder in **two** separate ways, and
both are needed:

* on **size**, because the installed total is snapped to the rung, so two
  processes whose readings differ by less than a rung install the same budget;
* on **count**, because a tranche is refused unless the re-priced total clears
  the *next* rung - a 15% growth at the shipped step - so two processes whose
  readings differ by less than a rung also agree on whether there is a tranche
  at all.

That second one is what makes the honest statement possible: **a re-planning run
whose remaining wall does not buy a rung produces exactly the document a
non-re-planning run produces.** The decision threshold is not a constant chosen
to be coarse; it is the ladder the mode already ships, re-read as a decision.

What is **not** bounded, and this is the limit of the claim: a box loaded
differently between two runs by more than a rung's worth of rate. No
work-denominated budget can bound that. It is the same limit `install_plan` has,
one reading later and over a longer window - measured at 4.37 s of queue against
a 2.52 s probe on mixed-61 seed 0 at a ten-second target, so about **1.7x**, not
the order of magnitude that would make the reading's spread negligible.

---

# Part III — what it measures

## 7. The box was not quiet, and that is the first number

Every wall number below was taken while a **second measurement campaign**
(`docs/experiments/basin-race/`) was running benchmarks on the same host. This
is not a footnote discovered afterwards: `drivers/runlib.py` records
`os.getloadavg()` immediately before and after every process, every driver
carries a `boxLoad` block, and `drivers/loadwatch.sh` sampled the whole window
into `evidence/boxload.tsv`.

Two consequences, and they point in opposite directions:

* **absolute wall numbers here are not comparable to `calibrated-plan`'s.**
  That round's batteries had the box to themselves. A number in this document
  that is worse than the same number there may be this round's mechanism or may
  be the box, and mostly it is the box.
* **arm-against-arm comparisons in one window are still valid**, and they are
  what every claim below rests on. Both batteries interleave their arms by
  round for exactly this reason, and the anytime table runs the three arms
  adjacently per cell.

There is a third consequence and it is a finding rather than a caveat: a loaded
box is where the single-plan mode is *weakest*, because its whole calibration is
one clock reading taken on the loaded box. §9 shows the re-plan recovering the
ladder rung that load cost.

### 7.1 What the load bought

Sol review 8 §3 condition 2 asks for something this campaign has never had:

> Un probe non stima un p95. Servono distribuzioni ripetute sotto carico
> deployment e margine esplicito.

A probe does not estimate a p95; you need repeated distributions under
deployment load and an explicit margin. This round did not set out to provide
that and does not claim to have designed it. What it has is an accident with the
right shape: **a twenty-round battery and a twenty-seven-cell anytime table, run
under a real competing workload, with the load recorded per run.** It is one
box, one competing workload and one afternoon, so it is a sample and not a
distribution - but it is the first time in this campaign that a wall claim has
been made against anything other than a quiet box, and where the mode degrades
under load is written down below rather than left to be discovered.

---

## 8. The four pinned gates, and the whole document

Both binaries built from this worktree; the gate binary is
`--features jagua-experimental`, which compiles **neither** the compression
schedule nor the parallel confirmation, so none of this round's three levers
exists in it. It is run against the campaign base commit's gate binary in the
same window.

| gate | pinned | reproduced | whole-document digest, ship == base |
|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | yes | `a93339cd73bceb00` |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | yes | `702f8d57d4a38db5` |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | yes | `37b41e060c263d81` |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | yes | `cd7144a1cdb39af1` |

`ALL_PASS: true` on both. And the check that matters, because a pinned scalar is
four numbers out of a document of thousands: **the whole-document digest, with
the wall-clock and provenance fields stripped, is identical between this tree's
gate binary and the base commit's on all four gates.**

`evidence/gates-ship.json`, `evidence/gates-base.json`, `drivers/gates.py`.

### 8.1 The binaries, and which battery ran on which

Four builds of this worktree were measured, and the round says which rather than
rounding them into one. Every hash is in `evidence/binaries.txt`.

| build | sha256 (16) | what it is | what it measured |
|---|---|---|---|
| `base-meas` | `5681046a61fc665e` | campaign base `29d5780`, full combo | the refactor gate's left-hand side |
| `base-gate` | `a2ad9bad87cc3325` | campaign base, `jagua-experimental` | the four pinned gates' left-hand side |
| early | `0c61e161d5d133a2` | the batching code as it shipped, before either `replan` fix | §10.1's `m34batch=100000` block |
| pilot | `15514f314505a97a` | re-plan with the extrapolation **unbounded** | §9.1's pilot only |
| stranded | `554044c3082f9184` | horizon bounded, stranding not yet fixed | §9.2's determinism gate only |
| batched | `8201c5718b6c80f9` | stranding fixed, `PLAN_FIRST_TRANCHE = 0.6` | §10's concatenation gates, §13.1's work and plan determinism, §9.3's sweep |
| **`ship-meas`** | **`9c049366385ecee2`** | the committed tree | §8's gates, §10's refactor gate, §11, §12, §13.1's re-planning determinism |
| **`ship-gate`** | **`5a58fb0ed6d3af18`** | the committed tree, `jagua-experimental` | §8's four pinned gates |

The one join that has to be argued rather than shown is `8201c571` against
`9c049366`, and the argument is short because the delta is one line: the value of
`PLAN_FIRST_TRANCHE`. It is read in exactly one place - `install_plan` - and only
when `plan_replan` is set. So:

* **the concatenation gates** run at `work=30000000`. `install_plan` is never
  called under a work budget; the constant is unreachable.
* **`determinism-work.json`** is a work budget: same argument.
* **`determinism-plan.json`** is `plan=10000` with `replan=0`, and
  `install_plan` forces `first_tranche` to `1.0` whenever re-planning is off, so
  the constant is read and discarded.
* **§9.3's sweep** passes `planfirst=` explicitly on both re-planning arms and
  `replan=0` on the third, so no arm reads the default at all.

And the gates that *can* be affected - the four pinned ones and the refactor
equivalence - were **re-run on `ship-meas`/`ship-gate`** rather than argued
about.

## 9. The first tranche, and the constant the pilot forced

### 9.1 The pilot: what an unbounded tranche does

The first cut of the re-plan had no horizon: a tranche priced **all** of the
remaining wall at the rate it had measured. `evidence/cal-pilot-unbounded.json`
is that arm, stopped part-way through its second round once the failure was
identified, and the failure is one row:

| cell | tranche priced at | window watched | wall it predicted | bought | run took |
|---|---:|---:|---:|---:|---:|
| mixed-61 s2, 30 s, `planfirst=0.6` | 13.64 s | 11.1 s | **15.46 s** | 66,211,771 | **36.74 s** |

An extrapolation 139% beyond its own window, on a quantity this campaign has
already measured as *not* holding over that range - `calibrated-plan` §13: *"the
fitted bias rises with the budget, because the queue's late actions cost more
per unit than its early ones"*. The rate fell 42% below the reading and the run
took 36.74 s against a 30 s target, which is the single plan's own failure
arrived at from the other side.

The fix is not a safety factor on the rate - that would be a second bias
constant guessing the same thing the first one guessed. It is to **stop
extrapolating**: `PLAN_TRANCHE_HORIZON = 1.0` caps the horizon at the window
already watched, buys what that justifies, and lets the next tranche re-measure.

It is **inert at ten seconds**, where the remaining wall is already shorter than
the window, and this was checked rather than assumed - the two arms reach the
same final budget by different routes (mixed-61 seed 0, one run each, on the
pre-freeze build `8fcb3974dc635ed4`):

| arm | plan | tranches | depth |
|---|---:|---|---:|
| `replan=1` (horizon 1.0) | 16,366,537 | 21,644,745 → 28,625,176 | 175.136 |
| `replan=1,planhorizon=1000` | 16,366,537 | 24,891,457 → 28,625,176 | 175.136 |

### 9.2 The second bug the round shipped and caught: a stranded run

The horizon cap has a failure mode of its own and the determinism gate found it
before the batteries did. `evidence/determinism-replan-stranded.json`, mixed-61
seed 2 at `plan=10000,replan=1`:

| | |
|---|---:|
| first plan | 9,357,620 (two rungs below its neighbours' 16,366,537) |
| tranches taken | **none** |
| depth | 179.662 / 179.006, against the `plan` arm's 175-176 |
| wall left unspent | ~5.7 s of ten |

The chain: a loaded probe put the first plan two rungs low; a low first plan is a
**short first tranche**; a short tranche leaves a **short queue window**; and the
horizon cap prices the tranche at that window, which on this cell bought 6% when
a rung is 15%. The growth test then refused the tranche, the loop broke, and the
run stopped with more than half its wall unspent — **worse than the mode it
improves**, at the budget the user priority names.

Refusing was the wrong answer to the right question. A tranche below one rung is
genuinely not a tranche - it floors straight back onto the budget the run already
has - so the question is not *"does the window justify a rung?"* but ***"can the
remaining wall pay for one?"***. It buys exactly that and never more:
`PLAN_TRANCHE_HORIZON` is deliberately exceeded in this one place, and the excess
is bounded by a **single rung** rather than by the whole remaining wall, so
§9.1's 36.74 s failure cannot come back through this door.
`a_window_too_short_for_a_rung_still_buys_one_when_the_wall_pays` pins it.

And the fix had a one-line arithmetic bug of its own, which is worth recording
because it is the kind that hides: a budget is a `u64`, so a rung has already
lost its fractional part, and `floor(rung) * 1.15` lands a fraction of a unit
*below* the next rung and quantises straight back onto the one it started from.
The next rung is now derived from the rung **index**, with the log round-trip's
error nudged at both ends;
`the_next_rung_is_derived_from_the_index_and_not_from_a_multiplication` pins that
too.

### 9.3 The first tranche: a negative result

`drivers/trancheq.py`, `evidence/cal-first-tranche.json`. mixed-61, three seeds,
two rounds, two targets, three arms - `replan=0`, `replan=1,planfirst=1.0` and
`replan=1,planfirst=0.6`. This is the sweep that was supposed to justify
`PLAN_FIRST_TRANCHE = 0.6`, and it justifies **1.0** instead.

mixed-61, 2 rounds x 3 seeds - load1 min 2.93 / median 4.32 / max 5.81 over 36 runs

| target | planfirst | n | wall p50 | wall max | worst / target | over target | depth median | per-seed depth | tranches |
|---:|---|---:|---:|---:|---:|---:|---:|---|---|
| 10 s | `off` | 6 | 7.13 s | 8.30 s | 0.830 | **0 of 6** | 175.388 | 175.388 / 174.170 / 176.162 | 0 |
| 10 s | `1.0` | 6 | 8.05 s | 8.31 s | 0.831 | **0 of 6** | 175.136 | 175.136 / 171.362 / 176.162 | 0/1 |
| 10 s | `0.6` | 6 | 7.98 s | 8.28 s | 0.828 | **0 of 6** | 175.136 | 175.136 / 171.362 / 176.162 | 2/3 |
| 30 s | `off` | 6 | 26.03 s | 36.54 s | 1.218 | **2 of 6** | 164.188 | 164.188 / 165.935 / 164.171 | 0 |
| 30 s | `1.0` | 6 | 25.99 s | 36.61 s | 1.220 | **2 of 6** | 164.188 | 164.188 / 165.935 / 164.171 | 0/1 |
| 30 s | `0.6` | 6 | 33.15 s | 34.13 s | 1.138 | **4 of 6** | 164.188 | 164.188 / 162.846 / 166.157 | 1/2 |

Read it in two halves.

**At ten seconds the fraction does nothing and the re-plan does everything.**
`0.6` and `1.0` produce the *same three depths* - 175.136 / 171.362 / 176.162 -
against the non-re-planning arm's 175.388 / 174.170 / 176.162. Both are 0 of 6
over target and both produce **one document per seed**. The gain is
`-0.252 / -2.808 / 0.000` mm per seed, and it belongs to the re-plan, not to the
shrunken first tranche.

That middle number is worth naming: **2.808 mm on seed 1 is exactly
`calibrated-plan` §9's ladder-floor cost for that seed** (`+0.252 / +2.808 /
+1.281`, the `plan` column against `planraw`). The mechanism recovered the thing
it was designed to recover, on the cell it was measured on, to three decimals.

**At thirty seconds the fraction moves the overrun rather than removing it.**
`0.6` bounds the worst case - 34.13 s against 36.54 s, so the previous round's
36.39 s does come down - and makes the *typical* case worse: **4 of 6 runs over
target at a p50 of 33.15 s**, against `1.0`'s 2 of 6 at 25.99 s. A smaller first
tranche spends more of the wall, which is what it is for; at this budget it
spends past the end of it.

So the honest statement of the sub-goal this constant was introduced for is a
**negative one**: re-planning does not fix the thirty-second overrun. It reduces
the worst case by 2.4 s and it does not make the mode honour a thirty-second
target, and the fraction that reduces the worst case makes the median worse. The
shipped value is `1.0` because at the budget the user priority names the two are
indistinguishable and at thirty seconds `1.0` is the one that does not make
things worse. `planfirst` stays a spec key so a deployment that cares more about
the tail than the median can have the other trade.


## 10. Sol's gate: N batches reproduce the monolith

`drivers/equiv.py`. One binary, one work budget, three fixtures, three seeds;
the two specs differ only in `m34batch`. A cell passes only when **both**
instruments agree - the whole document and the per-step digest of every slice in
it.

Two batch sizes at the pinned 30 M budget, chosen to bracket the mechanism
rather than to flatter it - one that splits a slice into a handful of batches
and one that splits it into hundreds - plus three cells at a **120 M** budget,
where the slices are longer and there are more of them. **21 cells, 1,741 batch
boundaries.**

A fourth block, `m34batch=100000` at 30 M, was run on an earlier build during
the round and is reported below rather than dropped: it is nine more cells and
299 more boundaries, and the batching code is identical in every build this
round produced.


### 10.1 The gate, at three batch sizes and two budgets

**`m34batch=400000`** - a handful of batches per slice

`bin/ship-meas` vs `bin/ship-meas`, work=30000000, extraB=`m34batch=400000`

| cell | document | step digest | m34 slices | batches | depth |
|---|---|---|---:|---:|---:|
| mixed-61-s0 | equal | equal | 1 | 9 | 173.5751 |
| mixed-61-s1 | equal | equal | 1 | 9 | 171.3620 |
| mixed-61-s2 | equal | equal | 2 | 18 | 174.2800 |
| shapes-17-s0 | equal | equal | 1 | 3 | 200.3490 |
| shapes-17-s1 | equal | equal | 1 | 3 | 200.3494 |
| shapes-17-s2 | equal | equal | 1 | 3 | 200.3490 |
| triangle-20-s0 | equal | equal | 1 | 10 | 70.7711 |
| triangle-20-s1 | equal | equal | 1 | 11 | 70.7468 |
| triangle-20-s2 | equal | equal | 1 | 11 | 70.7473 |

allEqual=True allStepDigestsEqual=True totalBatches=77 - load1 min 2.91 / median 4.15 / max 5.31 over 18 runs

**`m34batch=25000`** - hundreds

`bin/ship-meas` vs `bin/ship-meas`, work=30000000, extraB=`m34batch=25000`

| cell | document | step digest | m34 slices | batches | depth |
|---|---|---|---:|---:|---:|
| mixed-61-s0 | equal | equal | 1 | 120 | 173.5751 |
| mixed-61-s1 | equal | equal | 1 | 129 | 171.3620 |
| mixed-61-s2 | equal | equal | 2 | 252 | 174.2800 |
| shapes-17-s0 | equal | equal | 1 | 47 | 200.3490 |
| shapes-17-s1 | equal | equal | 1 | 45 | 200.3494 |
| shapes-17-s2 | equal | equal | 1 | 47 | 200.3490 |
| triangle-20-s0 | equal | equal | 1 | 117 | 70.7711 |
| triangle-20-s1 | equal | equal | 1 | 133 | 70.7468 |
| triangle-20-s2 | equal | equal | 1 | 132 | 70.7473 |

allEqual=True allStepDigestsEqual=True totalBatches=1022 - load1 min 2.80 / median 3.47 / max 4.88 over 18 runs

**`m34batch=100000`** - tens of batches per slice. Run on build
`0c61e161d5d133a2`, an earlier build of this worktree: the batching code in it is
the code that shipped, and the gate is `work=`-denominated, so neither of the two
`replan` fixes that came later is reachable from it.

`bin/replan-meas` vs `bin/replan-meas`, work=30000000, extraB=`m34batch=100000`

| cell | document | step digest | m34 slices | batches | depth |
|---|---|---|---:|---:|---:|
| mixed-61-s0 | equal | equal | 1 | 33 | 173.5751 |
| mixed-61-s1 | equal | equal | 1 | 35 | 171.3620 |
| mixed-61-s2 | equal | equal | 2 | 68 | 174.2800 |
| shapes-17-s0 | equal | equal | 1 | 12 | 200.3490 |
| shapes-17-s1 | equal | equal | 1 | 12 | 200.3494 |
| shapes-17-s2 | equal | equal | 1 | 12 | 200.3490 |
| triangle-20-s0 | equal | equal | 1 | 39 | 70.7711 |
| triangle-20-s1 | equal | equal | 1 | 44 | 70.7468 |
| triangle-20-s2 | equal | equal | 1 | 44 | 70.7473 |

allEqual=True allStepDigestsEqual=True totalBatches=299 - load not recorded

**At a 120 M budget**, where the slices are longer and there are more of them:

`bin/ship-meas` vs `bin/ship-meas`, work=120000000, extraB=`m34batch=100000`

| cell | document | step digest | m34 slices | batches | depth |
|---|---|---|---:|---:|---:|
| mixed-61-s0 | equal | equal | 4 | 124 | 163.9270 |
| mixed-61-s1 | equal | equal | 5 | 159 | 162.1610 |
| mixed-61-s2 | equal | equal | 10 | 359 | 164.0040 |

allEqual=True allStepDigestsEqual=True totalBatches=642 - load1 min 3.21 / median 4.00 / max 4.72 over 6 runs

**Twenty-one cells, three batch sizes, two budgets, 1,741
batch boundaries, and every cell equal on both instruments.** The step digests
are the half that matters: they compare the clamp, the sweeps, the candidate
queries, the pair and boundary counts before and after, the confirmation's three
outcomes and the raw depth of **every step of every slice**, and a slice that
diverged at step 700 and re-converged by step 1,616 would fail there while
passing the aggregate.

The gate is not vacuous, and that is a property of the implementation rather
than of the driver: `advance` returns to its caller between batches, so a
loop-carried value that is not a field of `ScheduleSliceRun` is a value the
second batch starts over from. The batch counts are also not trivial - 9 to 11
per slice at 400 k, 3 to 18 at 25 k on the shorter fixtures, and 642 over three
runs at 120 M.

And the refactor that made it possible is itself gated: the resumable-slice
binary reproduces the **base commit's whole document** on all nine cells at a
pinned work budget, with real m34 slices in every one of them.

## 11. The twenty-round battery

`drivers/planbattery.py`, `evidence/battery-10s.json`. mixed-61, three seeds,
**twenty rounds**, three arms interleaved with arm order rotated by round so no
arm always runs first into a cold cache. 180 runs, one binary, one window.

| arm | spec | what it is |
|---|---|---|
| `plan` | `plan=10000` | `calibrated-plan`'s shipping mode |
| `replan` | `plan=10000,replan=1` | this round |
| `wall` | `wall=10000` | the incumbent, and every chapter before that one |

Two questions, and they are not the same question. **Does it land?** - process
wall against the target, and the count of runs over it. **Does it reproduce?** -
per seed, how many distinct plans, tranche counts, depths and whole-document
digests came out of twenty runs.


### 11.1 What twenty rounds say

mixed-61, target 10.0 s, 20 rounds x 3 seeds - load1 min 3.74 / median 5.76 / max 15.17 over 180 runs

| arm | n | wall p50 | wall p95 | wall max | over target |
|---|---:|---:|---:|---:|---:|
| `plan` | 60 | 7.159 s | 8.409 s | 10.923 s | **1 of 60** |
| `replan` | 60 | 8.114 s | 9.004 s | 10.441 s | **1 of 60** |
| `wall` | 60 | 9.836 s | 10.377 s | 11.030 s | **24 of 60** |

| arm | seed | distinct plans | tranches | distinct depths | distinct documents | depth |
|---|---:|---:|---|---:|---:|---|
| `plan` | 0 | 2 | 0 | 2 | 2 | 175.3878 / 178.1798 |
| `plan` | 1 | 3 | 0 | 3 | 3 | 174.1700 / 177.9079 / 179.6330 |
| `plan` | 2 | 2 | 0 | 1 | 2 | 176.1620 |
| `replan` | 0 | 4 | 0/1 | 4 | 4 | 175.1357 / 175.3878 / 178.1798 / 179.5869 |
| `replan` | 1 | 2 | 0/1 | 2 | 3 | 171.3620 / 174.1700 |
| `replan` | 2 | 4 | 0/1 | 3 | 5 | 176.1620 / 177.3430 / 179.0060 |
| `wall` | 0 | n/a | 0 | 8 | 20 | 168.4836 / 168.7560 / 169.3790 / 169.5118 / 169.5878 / 171.1110 / 171.5878 / 176.3094 |
| `wall` | 1 | n/a | 0 | 4 | 20 | 165.6558 / 165.8230 / 167.1830 / 169.4588 |
| `wall` | 2 | n/a | 0 | 2 | 20 | 174.2800 / 179.0060 |

- `plan`: allSeedsPlanStable=False allSeedsDocumentStable=False seedMedianOfMedians=175.3878
- `replan`: allSeedsPlanStable=False allSeedsDocumentStable=False seedMedianOfMedians=175.1357
- `wall`: allSeedsPlanStable=True allSeedsDocumentStable=False seedMedianOfMedians=169.4454

Three things, and the second is the one that costs this campaign a claim.

**1. The re-plan spends the wall it was given, and does not overrun for it.**
`replan`'s p50 is **8.114 s** against `plan`'s 7.159 s - a second of the target
the single plan was leaving unspent - and its p95 is 9.004 s against 8.409 s.
Both arms are **1 of 60 over target**; the `wall` arm is 24 of 60. On the median
of seed medians `replan` is **175.1357** against `plan`'s **175.3878**, a gain of
**0.252 mm**, and on seed 1 the modal depth moves from 174.1700 to 171.3620 -
**2.808 mm**, which is `calibrated-plan` §9's ladder-floor cost for that seed to
three decimals.

**2. Neither plan arm reproduced twenty runs out of twenty, and that is a
finding about the previous round rather than about this one.**
`calibrated-plan` §8.2's headline is *"one plan, one depth, one document per
seed"* over sixty runs. Re-run here, on a box that had a competing workload for
part of the window, the **same `plan=10000` arm** produced **2 / 3 / 1** distinct
depths per seed and 2 / 3 / 2 distinct documents. Its modal depth still holds
85-100% of the runs, so the mode is not broken - but *"a second process gets the
same number"* is a **quiet-box** property, and this round is the first time the
campaign has looked at it any other way.

`replan` is worse on this axis, and for a reason the mechanism makes obvious:
4 / 2 / 3 distinct depths, modal share 80-90%. A re-planning run has **two**
clock readings that can cross a ladder rung instead of one. What it does not do
is turn into the `wall` arm, which produced **eight** distinct depths on seed 0
with a modal share of 40% and twenty distinct documents on every seed.

**3. The ordering is the same at every level.** Distinct depths per seed,
summed over three seeds: `plan` 6, `replan` 9, `wall` 14. Distinct documents:
7, 12, 60. Runs over target: 1, 1, 24. The re-plan buys a millimetre and a
second of spent wall for three extra depth values across sixty runs, and the
incumbent is a different order of variance from either.

## 12. The anytime table, and Sparrow

**Three fixtures, three seeds, two processes per cell, three arms, one binary,
one window**, at three and ten seconds; and the **thirty-second cell on
mixed-61**, which is the fixture the previous round's 36.39 s overrun happened
on and the only one of the three whose depth moves at that budget at all.
`drivers/anytime.py`, `evidence/anytime.json` and `evidence/anytime30.json`.

That is a trim against `calibrated-plan` §10's 27 cells and it is a trim against
the clock, not against the argument: the box was shared (§7) and a
thirty-second row on shapes-17 - which saturates at 200.349 mm at three seconds
and never moves again in any arm - would have cost 36 runs to reproduce a
constant. It is stated here rather than left for a reader to notice from the
row count.

The three arms are not the same measurement and the table must not be read as if
they were:

* `wall=<ms>` gets the whole target as useful search and is not reproducible;
* `plan=<ms>` is `calibrated-plan`'s shipping mode: reproducible, and paying the
  bias, the counters and the floor;
* `plan=<ms>,replan=1` is this round's: the same ladder, the bias replaced by a
  measurement, and the floor recovered by iteration.

### 12.1 Three fixtures at three and ten seconds

load1 min 2.72 / median 4.42 / max 5.68 over 108 runs

| fixture | target | arm | seed medians (mm) | median | wall max | reproduced | over target | tranches |
|---|---:|---|---|---:|---:|---:|---:|---|
| mixed-61 | 3 s | `plan` | 181.589 / 179.690 / 179.662 | **179.690** | 2.37 s | 3/3 | 0/3 | 0 |
| mixed-61 | 3 s | `replan` | 181.589 / 179.690 / 179.662 | **179.690** | 2.34 s | 3/3 | 0/3 | 0 |
| mixed-61 | 3 s | `wall` | 179.587 / 179.633 / 179.006 | **179.587** | 2.68 s | 0/3 | 0/3 | 0 |
| mixed-61 | 10 s | `plan` | 175.388 / 174.170 / 176.162 | **175.388** | 8.34 s | 3/3 | 0/3 | 0 |
| mixed-61 | 10 s | `replan` | 175.136 / 171.362 / 176.162 | **175.136** | 8.32 s | 3/3 | 0/3 | 0/1 |
| mixed-61 | 10 s | `wall` | 170.453 / 165.656 / 174.280 | **170.453** | 10.33 s | 0/3 | 2/3 | 0 |
| shapes-17 | 3 s | `plan` | 200.349 / 200.349 / 200.349 | **200.349** | 3.45 s | 1/3 | 2/3 | 0 |
| shapes-17 | 3 s | `replan` | 200.349 / 200.349 / 200.349 | **200.349** | 3.46 s | 3/3 | 2/3 | 0 |
| shapes-17 | 3 s | `wall` | 200.349 / 200.349 / 200.349 | **200.349** | 1.90 s | 0/3 | 0/3 | 0 |
| shapes-17 | 10 s | `plan` | 200.349 / 200.349 / 200.349 | **200.349** | 8.42 s | 3/3 | 0/3 | 0 |
| shapes-17 | 10 s | `replan` | 200.349 / 200.349 / 200.349 | **200.349** | 8.55 s | 2/3 | 0/3 | 0/1 |
| shapes-17 | 10 s | `wall` | 200.349 / 200.349 / 200.349 | **200.349** | 9.68 s | 0/3 | 0/3 | 0 |
| triangle-20 | 3 s | `plan` | 70.771 / 70.747 / 70.747 | **70.747** | 2.26 s | 3/3 | 0/3 | 0 |
| triangle-20 | 3 s | `replan` | 70.771 / 70.747 / 70.747 | **70.747** | 4.96 s | 2/3 | 2/3 | 1/2 |
| triangle-20 | 3 s | `wall` | 70.771 / 70.747 / 70.743 | **70.747** | 3.18 s | 0/3 | 2/3 | 0 |
| triangle-20 | 10 s | `plan` | 70.742 / 70.746 / 70.742 | **70.742** | 7.90 s | 3/3 | 0/3 | 0 |
| triangle-20 | 10 s | `replan` | 70.740 / 70.746 / 70.742 | **70.742** | 8.36 s | 3/3 | 0/3 | 0/1 |
| triangle-20 | 10 s | `wall` | 70.730 / 70.730 / 70.729 | **70.730** | 9.49 s | 0/3 | 0/3 | 0 |

| fixture | target | `plan` | `replan` | `wall` | plan-wall | replan-wall | **replan-plan** |
|---|---:|---:|---:|---:|---:|---:|---:|
| mixed-61 | 3 s | 179.690 | 179.690 | 179.587 | 0.103 | 0.103 | **0.000** |
| mixed-61 | 10 s | 175.388 | 175.136 | 170.453 | 4.935 | 4.683 | **-0.252** |
| shapes-17 | 3 s | 200.349 | 200.349 | 200.349 | 0.000 | 0.000 | **0.000** |
| shapes-17 | 10 s | 200.349 | 200.349 | 200.349 | 0.000 | 0.000 | **0.000** |
| triangle-20 | 3 s | 70.747 | 70.747 | 70.747 | 0.000 | 0.000 | **0.000** |
| triangle-20 | 10 s | 70.742 | 70.742 | 70.730 | 0.012 | 0.012 | **-0.000** |

median `replan` - `plan` over 6 rows: **0.000 mm**

allPlanCellsReproduced=False allReplanCellsReproduced=False allWallCellsReproduced=False

The load here was **2.72 to 5.68**, the quietest window of the round, and the
table is correspondingly clean: `plan` reproduced 16 of 18 cells and `replan` 15
of 18, against `wall`'s **0 of 18**.

Two rows carry everything and the rest are nulls that are worth stating anyway.

**mixed-61 at ten seconds is the only cell where the re-plan is visible**, and it
is the cell `calibrated-plan` §10.1 put the entire +6.904 mm in: `replan`
175.136 against `plan` 175.388, per seed **175.136 / 171.362 / 176.162** against
**175.388 / 174.170 / 176.162**. Three of three cells reproduced in both arms, no
overruns in either, and the same 2.808 mm on seed 1.

**shapes-17 contributes nothing at any budget**, saturating at 200.349 mm in all
three arms - which is `fast-contract-validator` §10.3's finding and
`calibrated-plan` §10.1's, unchanged.

**triangle-20 at three seconds is where the re-plan is worst**, and it is worth
naming rather than burying: `replan` reached **4.96 s against a 3 s target**
where `plan` reached 2.26 s. The fixture's phase 0 is under a second, so the
first plan is large and the re-plan then adds tranches whose last one overshoots
- the same "the rate falls as the budget grows" mechanism as §9.1, at a budget
where one prediction horizon is a large fraction of the whole target. **The
re-plan is calibrated at ten seconds and should not be armed at three.**

Over the six (fixture, budget) rows the median `replan` − `plan` is **0.000 mm**,
which is the honest summary: the mechanism is worth 0.252 mm on one row and
exactly nothing on the other five.


### 12.2 The thirty-second cell

load1 min 4.06 / median 4.84 / max 8.13 over 18 runs

| fixture | target | arm | seed medians (mm) | median | wall max | reproduced | over target | tranches |
|---|---:|---|---|---:|---:|---:|---:|---|
| mixed-61 | 30 s | `plan` | 164.188 / 167.666 / 165.190 | **165.190** | 41.15 s | 1/3 | 1/3 | 0 |
| mixed-61 | 30 s | `replan` | 164.188 / 165.935 / 164.171 | **164.188** | 37.14 s | 2/3 | 2/3 | 0/1 |
| mixed-61 | 30 s | `wall` | 165.262 / 160.010 / 167.752 | **165.262** | 29.22 s | 0/3 | 0/3 | 0 |

| fixture | target | `plan` | `replan` | `wall` | plan-wall | replan-wall | **replan-plan** |
|---|---:|---:|---:|---:|---:|---:|---:|
| mixed-61 | 30 s | 165.190 | 164.188 | 165.262 | -0.072 | -1.074 | **-1.002** |

median `replan` - `plan` over 1 rows: **-1.002 mm**

allPlanCellsReproduced=False allReplanCellsReproduced=False allWallCellsReproduced=False

**This is the cell the task asked about, and the answer is "reduced, not
removed".**

`calibrated-plan` §10.2 reported the plan mode running **36.39 s against a 30 s
target**. Re-measured here the same arm reached **41.15 s** - worse, on a quieter
box, which is itself a comment on how little of that number was ever a property
of the mode rather than of the draw. Re-planning brought the worst case to
**37.14 s**, took the median depth from 165.190 to **164.188**, and reproduced
2 of 3 cells against `plan`'s 1 of 3.

So the overrun **survives**. It is 4 s smaller and the mode is better on depth
and on reproduction in the same cell, and it is still 24% over a thirty-second
target. §9.3 is the attempt to fix it properly and its result is negative.

The `wall` arm, in this window, honoured thirty seconds on all three cells
(max 29.22 s) - which it did not do in `calibrated-plan` §10 (41.23 s). Three
cells is not a distribution; what it does establish is that **the thirty-second
overrun belongs to the work-denominated modes**, and this round does not close
it.


### 12.3 The checkpoint's consumer, priced — **RETRACTED**

> **This section is withdrawn in full.** See the correction at the top of this
> file. `m34cap` could not change a trajectory at this HEAD: `advance` recorded
> a checkpoint and left `finished = false`, and the caller looped
> `while !slice.finished` to the end of the monolith. The two arms below are two
> processes running **one** trajectory, and the difference between their walls
> is a measurement of the box.
>
> The replay is `docs/experiments/real-interruption/evidence/capreplay-30M.json`:
> at `work=30000000` on mixed-61 seeds 0/1/2, `m34cap=0` and `m34cap=1` produce
> identical depth, fingerprint, work, operator-call count and per-slice step
> digest. Nothing below is deleted, so the retraction can be audited against
> what it retracts.

mixed-61, 2 rounds x 3 seeds - load1 min 2.92 / median 4.36 / max 9.11 over 12 runs

| target | m34cap | n | wall p50 | wall max | worst / target | over target | depth median | per-seed depth | tranches |
|---:|---|---:|---:|---:|---:|---:|---:|---|---|
| 30 s | `capoff` | 6 | 32.64 s | 36.69 s | 1.223 | **4 of 6** | 164.171 | 164.188 / 162.846 / 164.171 | 0/1 |
| 30 s | `capon` | 6 | 25.91 s | 36.47 s | 1.216 | **2 of 6** | 164.188 | 164.188 / 165.935 / 164.171 | 0/1 |

`m34cap=1` is the policy that reads the batch boundary: the coordinator hands
each slice its own remaining budget and the slice gives itself back at the first
checkpoint past it, holding its last exact-valid incumbent. Both arms re-plan, so
the cap is the only difference between them.

**RETRACTED — the paragraph below is what was claimed, and it is wrong.** The two
arms are one trajectory; nothing here is caused by the cap.

**It works, and it is priced.** The p50 falls from **32.64 s to 25.91 s** and the
overruns from **4 of 6 to 2 of 6**; the cost is on seed 1, where the depth goes
from 162.846 to 165.935 - **3.089 mm** - because the slices that were paying for
that depth are the ones the cap stops. Seeds 0 and 2 are unchanged to the
micron. Both arms produce **one document per seed**, which is the property that
makes this a usable lever rather than a source of jitter: the cap is denominated
in the slice's own counter, so two processes stop at the same checkpoint.

That is the whole argument for the batch mechanism in one table. An atomic slice
cannot be stopped, so the only way to bound its wall is to refuse to dispatch it,
and the affordability rule can only refuse on an *estimate*. A slice that can
stop at a checkpoint is bounded by what the budget actually has left.

### 12.4 Against Sparrow

Sparrow on this same x86_64 box, seed 0, 8 workers, from
`docs/experiments/sparrow-mixed61/` §"x86_64 same-machine addendum" -
**157.971 mm at three seconds and 150.165 mm at ten**, both exact-valid. Those
numbers were taken on a **quiet** box; every number in the column beside them
was not, so the comparison below is if anything unkind to this engine and is
still not close.

| budget | Sparrow | this round, `wall` | `plan` | **`replan`** | gap, best arm |
|---|---:|---:|---:|---:|---:|
| 3 s | 157.971 | 179.587 | 179.690 | 179.690 | **21.6 mm** |
| 10 s | 150.165 | 170.453 | 175.388 | **175.136** | **20.3 mm** |
| 30 s | not published | 165.262 | 165.190 | 164.188 | - |

**The gap is not moved by this round and this round does not claim to move it.**
At ten seconds it is **20.3 mm** against the `wall` arm - `calibrated-plan` §11
put the same comparison at 18.3 mm, and the difference is that arm's own spread,
which §11.1 measures at eight distinct depths on one seed - and 25.0 mm against
the reproducible `replan` arm. What this round changes is not the gap but how
much of the left-hand column is a *number* and how much is a draw, and §11.1's
answer to that is less flattering than the previous round's.

## 13. Determinism and the suites

### 13.1 Two processes, three budget modes

The campaign's standing hard gate, plus this round's addition. Three requests x
three seeds each.

`work=30000000` is the unchanged gate: a work budget is a function of counters,
so two processes must produce identical documents, full stop.

`plan=10000` is `calibrated-plan` §12.3's two-part claim: the two processes must
agree on `portfolio.plan.units` **and**, given that, produce identical documents
with `planCalibration` stripped.

`plan=10000,replan=1` is the same claim with a third part, and the third part is
this round's: the two processes must agree on the **tranche sequence** - how
many they took and what each installed - not merely on the final total. A run
that took one tranche of 24 M and one that took two summing to 24 M ran
different searches, and a driver that compared only the total would call them
equal.


| cell | plans agree | tranches agree | document equal | depth |
|---|---|---|---|---:|
| mixed-61-s0 | True | True | True | 173.5751 |
| mixed-61-s1 | True | True | True | 171.3620 |
| mixed-61-s2 | True | True | True | 174.2800 |
| shapes-17-s0 | True | True | True | 200.3490 |
| shapes-17-s1 | True | True | True | 200.3494 |
| shapes-17-s2 | True | True | True | 200.3490 |
| triangle-20-s0 | True | True | True | 70.7711 |
| triangle-20-s1 | True | True | True | 70.7468 |
| triangle-20-s2 | True | True | True | 70.7473 |

allEqual=True allPlansAgree=True allTranchesAgree=True - load1 min 2.61 / median 3.55 / max 5.14 over 18 runs

**plan mode**

| cell | plans agree | tranches agree | document equal | depth |
|---|---|---|---|---:|
| mixed-61-s0 | True | True | True | 175.3878 |
| mixed-61-s1 | True | True | True | 174.1700 |
| mixed-61-s2 | True | True | True | 176.1620 |
| shapes-17-s0 | True | True | True | 200.3490 |
| shapes-17-s1 | True | True | True | 200.3494 |
| shapes-17-s2 | True | True | True | 200.3490 |
| triangle-20-s0 | True | True | True | 70.7420 |
| triangle-20-s1 | True | True | True | 70.7455 |
| triangle-20-s2 | True | True | True | 70.7416 |

allEqual=True allPlansAgree=True allTranchesAgree=True - load1 min 3.23 / median 3.81 / max 4.60 over 18 runs

**plan mode, re-planning**

| cell | plans agree | tranches agree | document equal | depth |
|---|---|---|---|---:|
| mixed-61-s0 | True | True | True | 175.1357 |
| mixed-61-s1 | True | True | True | 171.3620 |
| mixed-61-s2 | False | False | False | 176.1620 |
| shapes-17-s0 | True | True | True | 200.3490 |
| shapes-17-s1 | True | True | True | 200.3494 |
| shapes-17-s2 | True | True | True | 200.3490 |
| triangle-20-s0 | True | True | True | 70.7404 |
| triangle-20-s1 | True | True | True | 70.7455 |
| triangle-20-s2 | True | True | True | 70.7416 |

allEqual=False allPlansAgree=False allTranchesAgree=False - load1 min 3.15 / median 3.92 / max 4.28 over 18 runs

`work=30000000` is **9 of 9**, unchanged: the campaign's standing gate is not
moved by anything in this round.

`plan=10000` is **9 of 9** on both halves - the two processes chose the same
plan, and given that produced the same document.

`plan=10000,replan=1` is **8 of 9**, and the one miss is worth reading rather
than counting. On mixed-61 seed 2 the two processes' *initial* plans straddled a
ladder rung - 21,644,745 against 24,891,457, which is `calibrated-plan` §7's
predicted and only failure mode - and then **the re-plan brought them back
together**: one took a tranche to 24,891,457 and the other did not need to, and
both published **176.16200000000003**. The layouts are identical; the documents
differ because one of them records a tranche.

So the honest count is **8 of 9 as documents and 9 of 9 as layouts**, and the
mechanism that cost the document also repaired the depth.

The same gate at the rejected `planfirst=0.6`
(`evidence/determinism-replan-planfirst06.json`) is also 8 of 9, with all nine
initial plans agreeing and three-tranche sequences reproducing exactly - which
is the other half of §9.3's claim that the two fractions are indistinguishable
at ten seconds.

### 13.2 Suites

`drivers/run-suites.sh`, exit status captured **directly** rather than through a
pipe, because `cargo test ... | tee log` reports `tee`'s status and that is how
a red suite gets written up as green.

| suite | features | exit | tests |
|---|---|---:|---|
| `suite-jagua` | `jagua-experimental` | **0** | 1,275 passed, 0 failed |
| `suite-combo` | the protocol's full combo | **0** | 1,329 passed, 0 failed |

`EXITS jagua=0 combo=0`. Both passed on the first attempt, including the
campaign's known flake
(`free_material_multi_eviction_shrinks_retained_container_capacity`), which did
not need a rerun. Logs: `evidence/suite-jagua.log`, `evidence/suite-combo.log`.

The counts are **+8** and **+14** against `calibrated-plan` §12.4's 1,267 and
1,315, and the split is the round's own tests: the eight in `search::portfolio`
compile in both builds, and the six that need a compression schedule - five on
the batch budget and the step digest, plus
`concatenated_batches_reproduce_the_monolithic_slice` - compile only in the
combo. That last one is §10's gate at unit scale, in every feature configuration
on every commit: three batch sizes against the monolith, comparing the step
digest and asserting that the batching actually happened, because a budget large
enough to run the slice in one batch would make the test vacuous.

**The suites ran after every battery, on the committed tree.** An earlier
attempt was started alongside §10's work-denominated gates and discarded when the
source changed underneath it; nothing in this document is a number taken while a
compiler was running.

---

## 14. Honest caveats

* **The box was not quiet, and this is the largest caveat in the document.**
  §7 is the statement and `evidence/boxload.tsv` is the trace. Every wall
  number here is a loaded-box number. The *gates* are not affected - a work
  budget is a function of counters - and the arm-against-arm comparisons are
  interleaved, but any reader comparing an absolute second in this document to
  an absolute second in `calibrated-plan/` is comparing two boxes.
* **The round's own headline claim about the previous round is a
  re-measurement, not a refutation.** §11.1 shows `plan=10000` producing 2 / 3 /
  1 distinct depths per seed where `calibrated-plan` §8.2 measured 1 / 1 / 1. It
  is one box, one competing workload and one afternoon against another. What it
  establishes is that the property is **conditional on the box**, which that
  round did not claim and did not test; it does not establish a distribution,
  and a reader should not take 2 / 3 / 1 as a number either.
* **The thirty-second overrun is not fixed, and the fix that was meant to fix
  it was rejected on its own evidence.** §12.2 and §9.3. The round reduces the
  worst case by four seconds and records the rest as open.
* **`replan=1` is calibrated at ten seconds and is worse than the single plan at
  three.** Measured on triangle-20: **4.96 s against a 3 s target**, where the
  single plan reached 2.26 s (§12.1). One prediction horizon is a large fraction
  of a three-second budget, so a tranche that mis-prices the tail has nowhere to
  correct. It should not be armed there, and nothing in the code stops a caller
  doing so.
* **The two new constants were fitted on that box, at two budgets, on one
  fixture.** `PLAN_FIRST_TRANCHE` and `PLAN_TRANCHE_HORIZON` are mixed-61
  numbers at ten and thirty seconds. `planfirst` and `planhorizon` are spec
  keys for exactly that reason, and `drivers/trancheq.py` is the driver that
  refits them. The *shape* of both arguments survives a different box - a
  first tranche that aims at the whole target cannot recover from its own
  error, and a tranche that extrapolates past its window mis-prices a falling
  rate - and neither value does.
* **The re-plan does not make a number portable, only reproducible, and it does
  not make it reproducible under arbitrary load.** Two processes agree when
  their clock readings fall inside one ladder rung. §6 states the bound
  precisely and §9 shows where it held and where it did not.
* **The tranche boundary is a scheduling decision, not only a bookkeeping one.**
  Measured, not assumed: the affordability rule refuses an action the *current*
  tranche cannot afford, so where a boundary falls decides which actions the
  queue may buy before it. Two fractions that arrive at the same final budget by
  different routes can publish different depths.
* **`m34cap` is a work stop and not a wall stop.** Sol review 8 §3 condition 3
  asks for a wall stop between checkpoints. This round ships the checkpoints and
  a deterministic work-denominated policy over them; a wall stop remains
  unbuilt, and it cannot be deterministic when it is built.
* **The resumable slice is resumable *within* one operator call.** The batches
  are real - `advance` returns to its caller and every loop-carried value has to
  be a field - but nothing carries a suspended slice *across* coordinator
  actions yet. That is what would let the coordinator interleave a slice with
  another class, and it is the next step rather than this one.
* **Sol review 8 §3 condition 4's other half is untouched.** The condition names
  two things: mode 34's atomicity, which this round fixes, and *"serve un debit
  lane-local economico"* - the ~17% the profiling counters cost, which is
  `calibrated-plan` §9's 1.882 mm and is still paid in full. This round does not
  attempt it and does not claim any part of that 1.882 mm.
* **Nothing here is wired into a production route.** `replan`, `m34batch` and
  `m34cap` are spec keys on the benchmark example, and the coordinator that
  reads them is still `coordinator_v3`, which is still off by default.

## 15. Reproducing this

```
cargo build --release --example general_request_benchmark \
  --features jagua-experimental
cargo build --release --example general_request_benchmark --features \
  jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

bash drivers/run-rest.sh       # the gates, the concatenation gate, determinism,
                               #   the first-tranche sweep
bash drivers/run-final.sh      # the batteries, on the committed binary
bash drivers/run-suites.sh     # the two suites
bash drivers/collect.sh        # the summaries into evidence/
python3 drivers/tables.py docs/experiments/replan/evidence   # every table above
```

`drivers/runlib.py` and `drivers/gatelib.py` carry the pinned CLI tail, the
`0.002` search-offset allowance, the salt sets and the request table, and their
`ROOT` points at this worktree.

The two levers, as one line each:

```
'plan=10000,replan=1,cells=13:15:17:19,v3=1'   # the re-planning mode
'plan=10000,m34batch=100000,cells=...,v3=1'    # the batched slice
'work=<units>,cells=...,v3=1'                  # replay any of them exactly
```

A caller who wants the guarantee without the calibration takes the **last**
tranche's `units` - `portfolio.tranches[-1].units`, or `portfolio.plan.units`
when no tranche was taken - and replays it with `work=`. That is the same
recommendation `calibrated-plan` §14 makes, with one more place to read the
number from.
