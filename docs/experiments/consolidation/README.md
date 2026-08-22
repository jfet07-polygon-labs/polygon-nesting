# Consolidation: the map, the millimetre, the wall, and the retractions

The owner's instruction after the two strategic verdicts was to stop opening
levers and close the shipped surface. Four things, in the order Grok review 5's
"Prossime 3 spese" lists them plus the evidence-hygiene debt both reviewers
named:

1. **the map** - [`docs/shipped-surface.md`](../../shipped-surface.md): every
   Cargo feature and every spec key, with a verdict and the evidence that
   earned it;
2. **the millimetre** - the lane-local debit, which both reviewers call the one
   remaining engineering spend;
3. **the wall** - the thirty-second stop extended past the one class it bound;
4. **the hygiene** - four claims later rounds contradicted, marked at the
   claim; the merge-surgery's gates re-run and committed; `cargo fmt`.

Three of the four produced a result that is **not** what the instruction
expected, and each is stated as such below.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-1` |
| branch | `worktree-wf_d252e868-756-1`, on campaign branch `engine/topology-archive-search` |
| base commit | `40852e6` (both strategic reviews recorded) |
| governing documents | `docs/grok-review-5-stop-and-consolidate.md`, `docs/sol-review-10-governor-or-new-action.md` |
| requests | mixed-61 exact-clearance, shapes-17, triangle-20 |
| contract | from-request allowance `0.002`; record lineage `''` `0.0005` for the four gates |
| measure | `portfolio.incumbent.rawDepthMm` |
| box | 16 cores, engine pinned at 8 threads. **Never quiet** - a second measurement campaign ran on it throughout; every table carries its own load range |

### Binaries

`evidence/binaries.txt`, `drivers/run-build.sh`. The combo is
`jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator`.

| label | features | commit | sha256 (16) | what it measured |
|---|---|---|---|---|
| `base-gate` | `jagua-experimental` | `40852e6` | `d186b73e5514dc00` | the gates' left-hand side |
| `base-combo` | the combo | `40852e6` | `84dccd2529e23a99` | the head-equivalence gate's left-hand side |
| `ship-gate` | `jagua-experimental` | `9ead181` | `20d1aff857495163` | the gates, this round's code |
| `ship-meas` | the combo | `9ead181` | `87ff8ded175009e1` | **every battery below** |
| `fmt-gate` | `jagua-experimental` | `0a30453` | `83b381349ee21934` | the gates after `cargo fmt` |
| `fmt-meas` | the combo | `0a30453` | `6191dcdd369b1f5e` | the format-equivalence gate |
| `final-gate` | `jagua-experimental` | `2c5d687` | `281fa3715493c629` | the gates at the round's head |
| `final-meas` | the combo | `2c5d687` | `99c62fc84eb3ccee` | the head-equivalence gate, and both suites |

Six binaries because the round has three code commits and each has to be shown
not to have moved anything: `9ead181` is the batteries' binary, `0a30453` is
`cargo fmt`, and `2c5d687` fixes a test race this round introduced. §12 gates
all three against the base and against each other, so the batteries measured on
`ship-meas` are attributable to the head.

**The provenance gate this round opens with**: a clean build of the base commit
reproduces `docs/experiments/real-interruption/evidence/binaries.txt`'s two
sha256s **byte for byte** - `d186b73e5514dc00…` and `84dccd2529e23a99…`. Sol
review 9 required *"clean rebuild + evidence rigenerata"* before any promotion;
this is the rebuild, and it says the previous round's binaries and the committed
tree agree. Every number that round published is therefore comparable to every
number below.

No `se2-rigidity-certificate` build. Nothing this round touches
certificate-gated code.

---

## The headline

| | |
|---|---|
| **The map** | `docs/shipped-surface.md`. Every flag and key, three verdicts, and the sentence every round states separately: the shipping stack is default-off at the Cargo level and its three "default on" parts are on **inside v3**, which is itself off |
| **The base binaries reproduce** | a clean build of `40852e6` is byte-identical to `real-interruption`'s `gate-meas` and `ship-meas`. The provenance break Sol review 9 opened is closed by rebuild |
| **`calibrated-plan` §9 re-measured** | **+1.882 mm reproduced to four decimals**, then split: the counting is **+0.000 mm median on all three seeds** and the *timing* is the whole of it. §9's *"there is no version of this mode that avoids it"* is false, and it is false because one flag armed both halves |
| **The debit, in its own currency** | same `work=` budget, both arms, **documents identical field for field on 9 of 9 cells**: the debit retires the same work in **84.9%** of the seconds at 24.9 M and **82.5%** at 120 M. `search::portfolio`'s own "~17%" header, confirmed |
| **The debit, end to end** | a calibrated plan whose file the debit arm wrote: **−1.108 mm** median of seed medians at ten seconds, **one plan / one depth / one document per seed**, 0 of 9 over target, p95 8.87 s → 8.66 s |
| **Or, at the same depth** | the same file the profiler arm wrote: **identical depth and identical document on 3 of 3 seeds**, p50 7.31 s → 6.27 s |
| **The wall stop, on a forced overrun** | worst overrun **+26.63 s → +20.05 s (checkpoint) → +0.99 s (queue)**, 6 of 6 exact-valid, 6 of 6 exiting `wallStop` |
| **The wall stop, calibrated at 30 s** | **the deliverable was 0 of 9 and it was not met.** The *count* does not fall - 3/9 → 4/9, inside the load's own noise - and the *size* collapses: worst overrun **+12.38 s → +1.31 s**, wallMax 42.38 s → 31.31 s, at **+0.000 mm** of median depth and 9 of 9 exact-valid |
| **The reserve is a negative** | `m34wallreserve=1` is **worse** than the plain admission rule (+1.87 s against +0.99 s on the forced overrun), and §3.3 names the mechanism: it diverts the queue away from the one class that can stop itself mid-action |
| **Gates** | 4 of 4 on all four binaries, whole-document digests **identical across the base commit, this round's code, the reformatted tree and the head** |
| **Equivalence** | 9/9 four ways: this head with every key off against the base binary; the debit arm against the profiler arm; the reformatted tree against the pre-format one; the head against the batteries' binary |
| **Determinism** | work mode 9/9 unarmed and 9/9 with `lanedebit=1` |
| **A suite gap, found by arithmetic** | the suite counts decompose to **+1 / +2** exactly, and the third test this round added is in neither: `cargo test` builds an example and does not run its harness, so **no spec-key round-trip test in this repository has ever been reachable from either suite** - including the one added to catch the `m34cap` failure mode. A third suite now runs them (§15.1) |
| **Both suites, and neither passed first time** | the combo failed on a race **this round introduced** and the jagua on the campaign's known flake. Both attempts reported (§15) |
| **Retractions** | four claims marked at the claim, one review record annotated, one driver documented |
| **Everything ships off** | `m34wallstopall`, `m34wallreserve` and `lanedebit` all default to the previous round's behaviour, and a default document is unchanged key for key |

---

# Part I — the millimetre

## 1. What the previous two rounds were looking at, and why they could not see it

`calibrated-plan` §9 measured the work counters at **+1.882 mm** on mixed-61 at
a ten-second wall and closed with a sentence that is a claim about the design
rather than about a measurement:

> *"It is the price of denominating a budget in something a clock cannot
> perturb, and there is no version of this mode that avoids it."*

`work-currency` §6 tried to take it back with the parallel currency, got
**median 0.000 mm**, and correctly diagnosed why - under a wall budget
`debit_self_metered` returns zero by construction. It then named the spend that
would work:

> *"Lifting that counter out of the compression schedule and onto the relaxed
> lane, so mode 22 and mode 23 can self-report the way mode 34 already does, is
> what would let a work budget run with `profiling::set_enabled(false)` and take
> the 1.882 mm back."*

Both reviewers endorsed that spend. Grok review 5 §2 priced it at *"**Sì,
~tutto**"* of the 1.882; Sol review 10 put wall engineering at 175 → ~169.

**The specified lift would not have worked, and the round found out by
measuring the meter before building anything.** `work_units_from` is
`CandidateQueries + 5 x ExactPairTests`, and on a measured mixed-61 plan run
those are **7,859,321** and **586,787**, so the exact half is
`5 x 586,787 = 2,933,935` of `10,793,256` units - **27%**, not the ~4% the
compression schedule's own share suggested. `Counter::ExactPairTests` is
incremented in `kernel::exact`, which is a free function with no lane and no
lane to give it. A lane-local candidate-query counter alone would have
under-charged every work budget in the engine by a quarter.

## 2. The thing that was actually costing the millimetre

The lane's `score_placement` does two instrumented things on the same two
lines:

```rust
let _span = profiling::span(Phase::ScorePlacement);
profiling::count(Counter::CandidateQueries, 1);
```

The first is two `Instant::now` reads against a call that costs about a
microsecond. The second is one relaxed add on a thread-local block. **One flag
armed both**, which is why no arm of `calibrated-plan` §9 could separate them,
and why its conclusion attached to the wrong half.

`profiling::metering_enabled` is a second flag. The two counters a work budget
is denominated in - and only those two, at only those two sites - consult
`enabled() || metering_enabled()`; every span keeps consulting `enabled()`
alone. `PortfolioSettings::lane_local_debit` (`lanedebit=1`) is what a work or
plan budget uses to arm the second instead of the first.

**The budget is numerically unchanged.** The same counters are incremented at
the same sites by the same amounts, so `work_units_now` returns what it always
returned; every plan rung, every `plancal` key and every pinned `work=` replay
is on the same scale. That is not a nicety - it is what makes the A/B below a
comparison rather than two different experiments.

Three counters are deliberately left behind. `NeighborTests`,
`CollisionPolygonBuilds` and `FullRescores` are priced by
`search::work_currency` and by no budget; `NeighborTests` alone is **30,880,834**
against the meter's 7.86 M candidate queries on the run above, so arming it here
would put back the cost the flag exists to remove. A run that arms `cur2`
beside `lanedebit` therefore **defers** to the profiler and says so in
`workMeterArming` rather than computing a class price from three zeros.

### 2.1 The re-measurement, and the split

`drivers/metertax.py`, `evidence/metertax-10s.json`. Three arms at
`wall=10000`, three seeds, three rounds, arm order rotated. All three are
wall-budgeted, so all three spend the same seconds and the difference is what
the instrument ate.

binary `87ff8ded175009e1`  load1 min 4.748 / median 6.952 / max 12.486 over 27 runs

| seed | counters off | meter only | counters on | whole tax | **the counting** | **the timing** |
|---|---:|---:|---:|---:|---:|---:|
| 0 | 171.1110 | 171.1110 | 172.2875 | +1.177 | **+0.000** | +1.177 |
| 1 | 165.6558 | 165.6558 | 176.0560 | +10.400 | **+0.000** | +10.400 |
| 2 | 174.2800 | 174.2800 | 176.1620 | +1.882 | **+0.000** | +1.882 |
| | | | **median** | **+1.882** | **+0.000** | **+1.882** |

The whole tax reproduces `calibrated-plan` §9's headline **to four decimals**,
on the same fixture at the same budget, three sessions later. The counting's
median contribution is **+0.000 mm on every seed**.

Two things this table is not. It is **not** a claim that a metered run and an
uninstrumented run are the same run: a wall-budget arm has run-to-run spread
and these two do overlap rather than collapse (seed 0's `meterOnly` runs were
169.572 and 171.111). And seed 1's +10.400 is that spread, not a finding -
`work-currency` §6 measured **exactly +10.400 on seed 1** in its own session,
which is how reproducible the *sign* is and how unreproducible the magnitude
is. The median is what §9 quoted and the median is what reproduces.

## 3. The debit priced in the currency it is paid in

The instrument above is the least reproducible configuration this campaign has,
and both previous rounds said so. `drivers/workwall.py` measures the same thing
without that problem: **both arms run the same `work=<units>` budget**, so the
counters are the same counters, the trajectory is identical, the depth is
identical *by construction* - and the driver asserts it - and what is left over
is seconds.

`evidence/workwall-25M.json`. mixed-61, 3 seeds x 4 rounds, arm order rotated,
paired per round.
binary `87ff8ded175009e1`  load1 min 5.502 / median 6.586 / max 7.604 over 24 runs

| seed | depth (both arms) | work units (both arms) | profiler | debit | ratio |
|---|---:|---:|---:|---:|---:|
| 0 | 175.3877782649107 | 23,356,444 | 7.305 s | 6.209 s | **0.8490** |
| 1 | 174.17000000000002 | 22,152,474 | 6.497 s | 5.562 s | **0.8517** |
| 2 | 176.16200000000003 | 23,859,055 | 8.401 s | 7.118 s | **0.8425** |
| | | | | **median** | **0.8490** |

**`allDocumentsEqual: true`.** Every cell's two arms produce the same whole
document, so the depths above are one number and not two that agree.

`evidence/workwall-120M.json`, the same at the thirty-second band, 2 rounds:

| seed | depth | work units | profiler | debit | ratio |
|---|---:|---:|---:|---:|---:|
| 0 | 163.927 | 114,305,364 | 37.715 s | 32.910 s | 0.8729 |
| 1 | 162.161 | 121,474,651 | 44.566 s | 36.778 s | 0.8252 |
| 2 | 164.004 | 112,574,865 | 52.364 s | 41.312 s | 0.7889 |
| | | | | **median** | **0.8252** |

So the instrument costs **15.1%** of the wall at 24.9 M units and **17.5%** at
120 M. `search::portfolio`'s own header has said *"pays the ~17% they cost"*
since the meter was written; this is the first time the campaign has measured
it as a **paired ratio at identical work** rather than as a millimetre at a
noisy wall.

An independent confirmation falls out of the calibration files. A pass run with
the debit armed observes a shorter phase-0 wall for the same
`probe_work_units`:

| `probeWorkUnits` | `cal-live.json` | `cal-debit.json` | ratio |
|---|---:|---:|---:|
| 8,778,573 | 2.2042 s | 1.9097 s | 0.866 |
| 8,961,342 | 2.3173 s | 1.9323 s | 0.834 |
| 9,629,453 | 2.3137 s | 1.9715 s | 0.852 |

Three numbers measured by a different driver in a different window, landing on
the same ratio.

## 4. What a caller actually gets

`drivers/planbattery.py`, `evidence/planbattery-10s.json` and
`evidence/planbattery-10s-debitfile.json`. mixed-61, 3 seeds x 3 rounds.

The debit buys **two different things** depending on where the plan comes
from, and a round that reported only one of them would be misleading in either
direction.

### 4.1 With the budget held fixed: the wall

binary `87ff8ded175009e1`  load1 median 7.198 over 36 runs

| arm | median of seed medians | wall p50 | wall p95 | over target | plan stable | document stable |
|---|---:|---:|---:|---:|:--:|:--:|
| `callive` | **175.3878** | 7.31 s | 8.89 s | 0/9 | yes | yes |
| `caldebit` | **175.3878** | **6.27 s** | **7.40 s** | 0/9 | yes | yes |

`plancal` keys on `probe_work_units`, which is a **counter** and is identical in
both arms, so both read the same entry and install the same 24,891,457-unit
plan. Per seed the depths are the same three numbers - 175.3878 / 174.1700 /
176.1620 - and each arm produces one document per seed. **The output does not
move and the wall falls by a sixth.**

### 4.2 With the wall held fixed: the depth

`caldebitfile` is the shipping shape: the calibration pass itself ran with the
debit armed (`drivers/calpass.py` with `PLAN_CAL_EXTRA=lanedebit=1`), so the
file records the rate the run will actually retire work at.

load1 median 7.649 over 27 runs

| arm | plan | seed 0 | seed 1 | seed 2 | median | p95 | over | plan stable | doc stable |
|---|---:|---:|---:|---:|---:|---:|---:|:--:|:--:|
| `callive` | 24,891,457 | 175.3878 | 174.1700 | 176.1620 | **175.3878** | 8.87 s | 0/9 | yes | yes |
| `caldebitfile` | 28,625,176 | 175.1357 | 171.3620 | 174.2800 | **174.2800** | 8.66 s | 0/9 | yes | yes |
| | | −0.252 | −2.808 | −1.882 | **−1.108** | | | | |

**−1.108 mm at ten seconds, with the reproducibility property fully intact**:
one plan, one depth, one whole document per seed, 0 of 9 over the target, and a
p95 that is *lower* than the incumbent's rather than paid for.

The three per-seed deltas are one ladder rung - 24,891,457 x 1.15 =
28,625,176 - which is why they are 0.252 / 2.808 / 1.882 rather than a smooth
17%: depth moves when the budget crosses an action boundary, and each seed
crosses a different one. Those exact three numbers appear in
`calibrated-plan` §9's own decomposition, which is the same arithmetic seen
from the other side.

### 4.3 The arm that shows why the file matters

`plandebit` is the debit with a *live* plan - no `plancal` - and it is the
cautionary row:

| arm | median | plan stable | document stable |
|---|---:|:--:|:--:|
| `plan` | 175.3878 | yes | yes |
| `plandebit` | 174.2800 | **no** | **no** |

It reaches the same −1.108 mm, and seed 1 straddles the rung: two plans
(24,891,457 and 28,625,176) and two documents in three runs. A live plan reads
a clock, the debit moves the clock, and a faster clock near a rung boundary is
a coin toss. **The debit must be calibrated into the file, not left to the
live probe**, and that is the whole difference between §4.2 and this row.

## 5. What the debit does not do

* It does not approach Sparrow. mixed-61 at ten seconds goes from 175.388 to
  174.280 against Sparrow's 150.165: the gap is **25.223 mm → 24.115 mm**.
  Grok review 5 §2's split - *"~1.9 mm ingegneria, ~5 mm strutturali"* - is
  unchanged in shape and this is the 1.9 half, measured at 1.108.
* It does not touch the bias constant, which `calibrated-plan` §9 measures at
  **3.741 mm** and which remains the largest of the three costs.
* It does not change any pinned number. Every gate, every `work=` replay and
  every `plancal` entry is on the same scale, which §3's `allDocumentsEqual`
  is the proof of.
* It is **inert under a wall budget**, which reads no counter at all.

---

# Part II — the wall

## 6. What `m34wallstop` binds, and what it does not

`real-interruption` §13 states the defect precisely:

> *"The policy only binds the mode-34 checkpoint it is consulted at; it cannot
> stop an operator class that never asks it a question, and it cannot
> retroactively shorten a batch already in flight when the deadline is crossed
> mid-batch."*

Two reasons. `m34wallstopall` answers the first and **not** the second, and the
round is careful to keep them apart because the second is what decides whether
the deliverable is met.

The mechanism is an admission rule, not an interruption: at the top of the v3
queue loop, and in `Coordinator::affordability` for every class in both
coordinators, a run whose wall target has passed exits on a new
`PhaseExitCause::WallStop`. It is asked **in seconds**, which is the whole
point: under a plan or work budget every other stopping rule in the coordinator
is denominated in a counter, and a counter cannot see a box under load.

`WallStop` is deliberately outside the re-plan loop's `budget_bound` set. A
tranche buys *work*; a run that stopped because it was out of *seconds* must
not be handed more of it.

## 7. Does it fire? The forced overrun

The calibrated thirty-second battery has a weakness as a test of a mechanism:
a calibrated plan under-buys, so on most cells nothing crosses the deadline and
the policy has nothing to do. A battery in which the policy never fires cannot
tell *"it works"* from *"it is unreachable"* - which is exactly the trap
`replan` §12.3 fell into with `m34cap`.

`drivers/forcedoverrun.py`, `evidence/forced-overrun-10s.json`. `planhead=3.0`
buys a plan the ten-second wall cannot pay for. mixed-61, 3 seeds x 2 rounds.
binary `87ff8ded175009e1`  load1 min 6.106 / median 6.919 / max 11.072 over 24 runs

| arm | worst overrun | wall p50 | wall max | exact-valid | exit causes |
|---|---:|---:|---:|---:|---|
| `off` | **+26.63 s** | 26.52 s | 36.63 s | 6/6 | affordability 5, deadline 1 |
| `checkpoint` (`m34wallstop=1`) | **+20.05 s** | 23.46 s | 30.05 s | 6/6 | affordability 6 |
| `all` (`m34wallstopall=1`) | **+0.99 s** | 10.52 s | 10.99 s | 6/6 | **wallStop 6** |
| `reserve` (`+m34wallreserve=1`) | +1.87 s | 11.39 s | 11.87 s | 6/6 | wallStop 6 |

The mechanism is not in doubt. A ten-second contract that was being missed by
**26.63 seconds** is missed by **0.99**, every output is exact-valid, and every
run exits on the cause the key installs. The `checkpoint` row is the shipped
policy and is the measurement of §6's first sentence: it takes 6.6 s off a
26.6 s overrun and leaves 20.

## 8. The calibrated thirty seconds, where the deliverable is not met

`evidence/wallstop-30s.json`. mixed-61, 3 seeds x 3 rounds, every arm on the
same calibrated plan so the budget is one number and the only variable is which
classes the deadline binds.
binary `87ff8ded175009e1`  load1 min 7.414 / median **9.569** / max 21.708 over 36 runs

| arm | depth (median of seed medians) | wall p50 | wall p95 | wall max | over target | **worst overrun** | exact-valid | exits |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| `callive` | 164.1880 | 22.71 s | 42.38 s | 42.38 s | 3/9 | **+12.38 s** | 9/9 | affordability 6, deadline 3 |
| `calwallstop` | 164.1880 | 28.11 s | 43.80 s | 43.80 s | 3/9 | **+13.80 s** | 9/9 | affordability 6, deadline 3 |
| `calwallstopall` | **164.1880** | 28.27 s | 31.31 s | 31.31 s | **4/9** | **+1.31 s** | 9/9 | affordability 2, deadline 3, **wallStop 4** |
| `calwallreserve` | 164.2440 | 25.10 s | 32.30 s | 32.30 s | 3/9 | +2.30 s | 9/9 | affordability 3, deadline 3, wallStop 3 |

Per seed, so a median of three is not read as three agreeing runs:

| arm | seed 0 | seed 1 | seed 2 | wallMax | over |
|---|---|---|---|---:|---:|
| `callive` | 164.188 | 167.666 | 164.171 | 42.38 s | 0/0/3 |
| `calwallstop` | 164.188 | 167.666 | 164.171 **and** 174.28 | 43.80 s | 0/1/2 |
| `calwallstopall` | 164.188 | 167.666 | 164.186 **and** 164.244 | 31.31 s | 0/1/3 |
| `calwallreserve` | 164.188 | 167.666 | 164.244 | 32.30 s | 0/0/3 |

**The deliverable was "overruns 0 of 9 at 30 s with exact-valid outputs" and it
was not met.** Said plainly rather than reframed:

* **the count does not fall.** 3/9 → 4/9. That difference is inside the load's
  own noise - the box ran at median load 9.57 with a spike to 21.7, and the
  arm that gained a crossing gained it on seed 1, whose base runs happened to
  land at 21.46 s in their window;
* **the size collapses.** Worst overrun **+12.38 s → +1.31 s**, wallMax
  **42.38 s → 31.31 s**, p95 **42.38 s → 31.31 s**. That is the residual §6's
  second sentence predicts: one action in flight when the deadline passes;
* **the depth does not move.** Median of seed medians is **164.1880** on both,
  and `+0.000 mm` is what Grok review 5 §3 predicted for this spend;
* **every output is exact-valid**, 9 of 9 on all four arms.

`calwallstop`'s seed-2 cell producing **two** depths (164.171 and 174.28) is the
non-determinism that key's own doc comment promises: a wall stop reads a clock
at a checkpoint, and two processes agree only while they cross the deadline
between the same two checkpoints. `calwallstopall`'s seed 2 does the same thing
at a much smaller amplitude (164.186 / 164.244).

## 9. The reserve is a negative, and the mechanism is nameable

`m34wallreserve=1` refuses a class whose own measured mean seconds in this run
would not fit in what is left. It is **worse** than the plain admission rule on
the instrument that can see it: **+1.87 s against +0.99 s** worst overrun on the
forced battery, and a p50 of 11.39 s against 10.52 s.

The rows say why, and it is not noise. At the same plan, the reserve arm
consistently runs **more** actions and **more** work:

| cell | `all` | `reserve` |
|---|---|---|
| s0-r0 | 10 actions, 32,937,500 units, 10.99 s | 10 actions, 36,532,422 units, 11.87 s |
| s1-r0 | 8 actions, 27,981,096 units, 10.05 s | 9 actions, 33,397,312 units, 11.70 s |
| s2-r0 | 8 actions, 27,391,297 units, 10.38 s | 8 actions, 30,437,804 units, 11.13 s |

**The reserve diverts the queue away from the one class that can stop itself.**
`m34wallstopall` arms the mode-34 checkpoint stop as well as the queue rule, so
a schedule action started just before the deadline gives its turn back at its
next checkpoint. Every other class runs to completion once started. The reserve
prices classes by their mean seconds, mode 34's mean is the largest, so near
the deadline the reserve refuses mode 34 and the queue buys an **uninterruptible**
class instead - which then straddles the deadline and runs to the end.

So the recommendation is `m34wallstopall=1` with the reserve at its `0.0`
default, and the reserve is recorded here as a measured negative with its cause
named rather than as a dial nobody tried.

---

# Part III — the map and the hygiene

## 10. The shipped-surface map

[`docs/shipped-surface.md`](../../shipped-surface.md). Every Cargo feature and
every spec key, with one of three verdicts - ships-on, ships-off-available,
retired-with-negative - and the evidence pointer that earned it, plus a fourth
state (instrument) for code that exists to measure.

The load-bearing sentence in it is not a verdict but a scope note: **the
shipping stack is default-off at the Cargo level and its three "default on"
components are on inside v3, which is itself off**, so the campaign has been
measuring a configuration rather than shipping one. Every round says that in
its own caveats section; none of them said it in a place a reader arrives at
first.

## 11. The retractions

Four claims were carrying a correction three documents away, or none. The
discipline this campaign uses is a banner **at the claim**.

| document | claim | what was done |
|---|---|---|
| `calibrated-plan` §8.2 | *"one plan, one depth, one document per seed, 60 of 60"* | banner: it is a **quiet-box** property (`replan` §11.1: 2/3/1 under load; `robust-plan` §9: 3/2/2). Mechanism not retracted; `plancal`'s own 60/60 **is** measured under load. The headline row and §12.3 carry the same pointer |
| `calibrated-plan` §9 | *"there is no version of this mode that avoids it"* | struck, with §2.1's split as the correction |
| `replan` §3 | *"two slices with the same digest walked the same walk"* | struck. `sol-review-9` §P1: FNV-1a over clamp, counts and aggregate loss carries no placement fingerprint, no RNG state, no winning lane. `real-interruption` §4's three SHA-256 fingerprints are the repair |
| `basin-race` head | *"the criteria are a landslide"* as the reason for 0/21 | banner: the verdict stands, the explanation does not. `sol-review-9` §P0 names four defects, each sufficient on its own, including a ranker that is not dense and a `confirmations_attempted == 0` that scores the **maximum** |
| `basin-race` §5.4 | *"the witness composes"* | banner: `descendant` is defined as `final(adopt) < final(publish)`; the arms are 2.8% apart in work on seed 1; the adoption writes `confirmed_state` before the composite gate |
| `grok-review-4` head | quotes the retracted `m34cap` price | header note. The verbatim text is **not** edited - it is a record of what was said - and the note says the review's own conclusions do not rest on it |
| `replan/drivers/trancheq.py` | the driver that could not emit `m34cap` | docstring. Left functionally unchanged so a reader can check the claim |

**The m34cap chain, verified complete**: `replan/README.md`'s head correction,
its §12.3 and headline row struck in place, `replan/evidence/cap-30s.json`'s
`SUPERSEDED` block, `next-generation-engine-plan.md:6824`'s *"Corrected by"*
note, `real-interruption` §2's replay, and now `grok-review-4`'s header and
`trancheq.py`'s docstring, which were the two open ends.

## 12. The gates the merge surgery never committed

Sol review 9 §"Chirurgia di merge" closed on:

> *"I gate post-merge descritti dall'agente non sono presenti come artefatto
> identificato da HEAD `8e7f82e`. […] Il dichiarato 9/9 armato con quattro
> adozioni non è auditabile dal repository."*

This round does not reconstruct that claim - the arm it described is retired
(§11, witness adoption) and re-running it would be re-opening a closed lever.
What it does instead is commit the gates that make the *current* tree
auditable, which is what the finding is actually about:

| gate | left | right | result | file |
|---|---|---|---|---|
| four pinned, base | `base-gate` | - | **4/4 hit** | `evidence/gates-base.json` |
| four pinned, this round | `ship-gate` | - | **4/4 hit** | `evidence/gates-ship.json` |
| four pinned, reformatted | `fmt-gate` | - | **4/4 hit** | `evidence/gates-fmt.json` |
| four pinned, head | `final-gate` | - | **4/4 hit** | `evidence/gates-final.json` |
| head equivalence | `base-combo` | `ship-meas`, no keys | **9/9**, step digests equal | `evidence/equiv-head.json` |
| debit equivalence | `ship-meas` | `ship-meas`, `lanedebit=1` | **9/9**, step digests equal | `evidence/equiv-debit.json` |
| format equivalence | `ship-meas` | `fmt-meas` | **9/9**, step digests equal | `evidence/equiv-fmt.json` |
| head equivalence | `ship-meas` | `final-meas` | **9/9**, step digests equal | `evidence/equiv-final.json` |

**The whole-document digest is identical on all four gates across all four
binaries** - `a4729eaed6a7d750`, `fb5eca9d78d9ef79`, `86100e4c02eaa99d`,
`9fc5649818e9731a` - so the base commit, this round's code, the reformatted
tree and the head all produce the same four documents.

The four pinned values, unchanged: `206.869` / `8a7737381238fa4d`,
`159.09233022733062` / `fa01012af1d559ae`, `159.07876040364795` /
`e28fba007f8031d4`, `164.0375677990678` / `49f094d7e59a9008`.

## 13. Determinism

`drivers/determinism.py`. The hard gate is the work-budget one: a work budget is
a function of counters and not of the clock, so two processes must produce the
same document.

| arm | cells | result | file |
|---|---|---|---|
| `work=30000000`, unarmed | 3 requests x 3 seeds | **9/9 identical** | `evidence/determinism-work.json` |
| `work=30000000`, `lanedebit=1` | 3 requests x 3 seeds | **9/9 identical** | `evidence/determinism-work-debit.json` |
| `plan=30000`, `plancal`, `m34wallstopall=1` | mixed-61 x 3 seeds | 3/3 identical | `evidence/determinism-wallstop.json` |

**The third row is not a determinism claim and must not be read as one.** A
wall stop reads a clock; three cells in which it did not fire are three cells
in which nothing was tested. §8's per-seed table is the honest statement:
`calwallstopall` produced two depths on seed 2 across nine runs. The row is
here because a *regression* in the unarmed path would have shown up in it.

## 14. `cargo fmt`

`cargo fmt --check` was **158 diffs across 17 files** at this round's base, and
all seventeen are files the campaign touched - the workspace's other crates are
already clean, so the pass is exactly the campaign's own drift.

It took two passes to converge: the first left one `match` arm in
`general_request_benchmark.rs` unstable, because its error string exceeds
`max_width` either way. The tree is now `cargo fmt --check` clean at **0**.

**It is not whitespace-only, and the claim is stated to match.** Ten of the
seventeen files differ after every whitespace character is removed, in three
classes, each semantics-preserving by Rust's grammar rather than by inspection:
trailing commas added to or removed from argument lists and array literals;
braces added around a closure body that was a bare `if`; and `use` items
reordered within their group.

So the claim is **"the documents are identical"** and not "the bytes are". The
binaries are not bit-identical and were never going to be - a release build
embeds `panic::Location` line numbers, so moving a line moves the binary
(`ship-gate` `20d1aff8…` against `fmt-gate` `83b38134…`). What is proven is in
§12: 4 of 4 gates hit with digests identical to every other binary in the
table, and the work-budget equivalence gate 9 of 9 with step digests equal.

## 15. The suites, and the one this round broke

`drivers/run-suites.sh`, exit status read on the line after the redirect rather
than through a pipe, because `cargo test … | tee log` reports `tee`'s status and
that is how a red suite gets written up as green.

| suite | features | exit | tests |
|---|---|---:|---|
| `suite-jagua` | `jagua-experimental` | **0** | **1,294 passed**, 0 failed, 2 ignored, over 55 test binaries |
| `suite-combo` | the protocol's full combo | **0** | **1,358 passed**, 0 failed, 2 ignored, over 55 test binaries |

`EXITS jagua=0 combo=0`. Logs: `evidence/suite-jagua.log`,
`evidence/suite-combo.log`.

**Both attempts are reported, because neither passed first time and the two
failures are different in kind.**

* **The combo suite ran red on the first attempt, and the failure was this
  round's own.** `profiling::tests::enabled_recording_accumulates_and_resets`
  asserted a count of 5 and read 0, at 947 passed / 1 failed. The cause is the
  trap this file's own comment describes and then handles by convention: the
  recording flags and the thread blocks are process-global, `cargo test` runs
  the crate's tests in parallel threads of one process, and
  `recording_is_inert_while_disabled` calls `reset()`. The convention - *"keep
  exactly one enabling test"* - held only while the enabling test was the only
  thing recording anything. `profiling::recording_test_lock` replaces it, and
  it is `pub(crate)` rather than private to `mod tests` because the same bug
  has a cross-module form: `portfolio`'s flag test flips `ENABLED` and would
  break the span assertion from another module. Commit `2c5d687`.

  Worth saying plainly: **the first version passed the `jagua-experimental`
  suite and failed the combo's on the same tree.** It is a race, so a green run
  proves nothing.

* **The jagua suite then ran red on the campaign's known flake**, at 883
  passed / 1 failed on the lib binary:
  `free_material_multi_eviction_shrinks_retained_container_capacity`, asserting
  `cache.entries.capacity() < entries_capacity_before`. It is unrelated to this
  round - it is an allocator-capacity assertion that has flaked across several
  rounds - and it passed on the rerun, which is the protocol's rule for it.
  The failed attempt is kept as `evidence/suite-jagua-attempt1.log`; note its
  totals are over **5** binaries rather than 55, because `cargo test` stops at
  the first failing binary.

### 15.1 The counts decompose exactly, and that is how a gap was found

The previous round's own logs, counted the same way, are **1,293** and
**1,356**. So this round is **+1** and **+2**, and those decompose exactly:

| test | where | jagua | combo |
|---|---|:--:|:--:|
| `the_work_meter_arms_one_flag_and_restores_both` | `search::portfolio` | yes | yes |
| `the_wall_stop_reads_the_requested_wall_and_only_when_one_was_named` | `search::portfolio`, `compression-schedule`-gated | - | yes |
| `the_consolidation_keys_reach_their_fields` | the benchmark **example** | **-** | **-** |

**The third row is a finding, not a bookkeeping note.** `cargo test` builds an
example but does not run its test harness, so *no spec-key round-trip test in
this repository has ever been reachable from either suite the protocol names* -
including `the_interruption_keys_reach_their_fields`, which the previous round
added specifically to catch the `m34cap` failure mode of a key nobody parses.
That is exactly the defect `basin-race` §9 had to write up and that
`work-currency` §7.1 claimed to have avoided:

> *"**No test in this round is unreachable from a suite the protocol names**,
> which is the failure `basin-race` §9 had to write up and this one does not."*

That sentence was true of the tests that round added and is **not** true of the
repository. This round does not restructure the example into a test target -
that is a build-layout change with its own blast radius and it is not what the
owner asked for - but it does stop the tests being invisible: a third suite,
run explicitly, and its log committed beside the other two.

| suite | features | exit | tests |
|---|---|---:|---|
| `suite-example` | the combo, `--example general_request_benchmark` | **0** | **21 passed**, 0 failed |

`evidence/suite-example.log`. Both spec-key round trips are in it -
`the_consolidation_keys_reach_their_fields` and
`the_interruption_keys_reach_their_fields` - and both pass. A future round
should either add `test = true` to the example's `[[example]]` stanza or move
the parser into the library; until then, **`drivers/run-suites.sh` runs three
suites and not two**.

---

## 16. Honest caveats

* **The thirty-second deliverable was not met.** "0 of 9" was the target and
  4 of 9 is the number. §8 is the result and §6 is why: this key answers one of
  the two reasons `real-interruption` §13 names and leaves the other, which is
  the action in flight when the deadline passes. Bounding *that* needs an
  operator that can be interrupted mid-action, which is Sol review 10's
  governor round and which the owner deferred.

* **The box was never quiet.** A second measurement campaign ran on it
  throughout. Every table carries its own load range and the thirty-second
  battery's is the worst of them - median 9.57, max 21.71 - which is why its
  overrun *counts* are not comparable to `real-interruption` §9's and its
  overrun *sizes* are.

* **The counter tax's magnitude does not reproduce per seed, and never has.**
  §2.1's median lands exactly on `calibrated-plan` §9's, and its per-seed
  numbers (1.177 / 10.400 / 1.882) are as far from §9's (2.700 / 1.527 / 1.882)
  as `work-currency` §6's were (7.553 / 10.400 / 4.006). A wall-budget arm is
  the least reproducible configuration this campaign has. §3 exists because of
  that, and §3's numbers are paired ratios at identical work, which is the
  instrument this claim should have had from the beginning.

* **`lanedebit` changes what a plan buys, and a live plan can straddle.** §4.3
  is that measurement: the same −1.108 mm, with two plans and two documents on
  one seed of three. The shipping shape is the calibrated one, and it needs its
  **own** calibration file - a file written by a profiler-armed pass and read
  by a debit-armed run under-buys by exactly the ratio §3 measures. Two files,
  named by the arm that wrote them, is the only honest arrangement and
  `drivers/calpass.py`'s `PLAN_CAL_EXTRA` is how the second is produced.

* **The debit's three left-behind counters are a real restriction.** A run that
  wants `search::work_currency` cannot have the debit; it defers and the
  document says so. Nothing ships with `cur2` armed, so nothing shipped is
  affected, but a future round that wants both will have to decide what
  `NeighborTests` costs.

* **The reserve is off, and §9's explanation is a reading of six rows.** The
  mechanism - the reserve diverting the queue away from the only interruptible
  class - is consistent with the action and work counts in every cell, and it
  is not independently instrumented. It is a hypothesis with a signature, not a
  measurement of the diversion itself.

* **`m34wallstopall` cannot be deterministic and is not.** Same trade as
  `m34wallstop`, same reason, and §8's seed-2 cell shows it at 164.186 against
  164.244. A caller who needs one document per seed leaves it off and accepts
  the overrun.

* **Nothing here is wired into a production route.** `m34wallstopall`,
  `m34wallreserve` and `lanedebit` are spec keys on the benchmark example, the
  coordinator that reads them is `coordinator_v3`, and `coordinator_v3` is off
  by default. See `docs/shipped-surface.md` §4.

* **The example's spec-key tests are still not in `cargo test`'s default
  target set.** §15.1 makes them *run*, by naming a third suite; it does not
  make them *unmissable*, which would need `test = true` on the `[[example]]`
  stanza or the parser moved into the library. A future round that adds a key
  and forgets the third suite is in the same position the previous rounds were.

* **The ledger is three rounds behind this one.**
  `docs/next-generation-engine-plan.md`'s last chapter is `replan`'s;
  `robust-plan`, `work-currency` and `real-interruption` never got one. This
  round's chapter says so rather than being written as though they had, and
  `docs/shipped-surface.md` is the map that stands in for the missing three.

## 17. Reproducing this

```
bash drivers/run-build.sh      # refuses a dirty tree; writes evidence/binaries.txt
python3 drivers/gates.py base  /var/lib/t3/tmp/consol/bin/base-gate
python3 drivers/gates.py ship  /var/lib/t3/tmp/consol/bin/ship-gate
bash drivers/run-measure.sh    # calibration, then every battery and both gates
bash drivers/run-rest.sh       # the debit's own calibration pass, the forced overrun
bash drivers/run-suites.sh     # three suites, exits captured directly
```

The keys, as one line each:

```
'plan=10000,plancal=<file>,cells=...,v3=1'                     # the incumbent
'plan=10000,plancal=<file>,lanedebit=1,cells=...,v3=1'         # same depth, less wall
'plan=10000,plancal=<debitfile>,lanedebit=1,...'               # the millimetre
'plan=30000,plancal=<file>,m34wallstopall=1,...'               # the wall, all classes
'plan=10000,planhead=3.0,m34wallstopall=1,...'                 # the forced overrun
'work=<units>,cells=...,v3=1'                                  # replay any of them
```
