# The plan a loaded box cannot move

`docs/experiments/replan/` §11.1 retired a claim and left a defect behind it.
The claim was `calibrated-plan` §8.2's *"one plan, one depth, one document per
seed, 60 of 60"*; re-measured on a box that had a competing workload for part of
the window, the **same `plan=10000` arm** produced **2 / 3 / 1 distinct depths
per seed**, and the re-planning arm was worse at 4 / 2 / 3. That round's own
words:

> *"a second process gets the same number"* is a **quiet-box** property, and
> this round is the first time the campaign has looked at it any other way.

This round makes it not a quiet-box property.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f03cd94d-c01-2` |
| base commit | `8e7f82e` (the resumable-m34 re-plan merged into the basin-race branch) |
| requests | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, `tests/fixtures/shapes-17/2000x2700-compact/request.json`, `tests/fixtures/triangle-20/2000x2700-compact/request.json` |
| contract | true 5.0/5.0 exact clearance, search-offset allowance **`0.002`** for every battery and table; the four pinned gates carry their own `0.0005` tail |
| measure | `portfolio.incumbent.rawDepthMm` |
| box | Intel Core Ultra 7 270K Plus, 16 cores, engine pinned at 8 threads (`actualThreads: 8`), **shared with other agents throughout** |
| relevant source tree | sha256 `3fde60dbf544dcfe6ecac2e693492ebb69be677cfb7e5713949b47cd11ba72ec`, which is the committed tree: the last source edit predates the `ship2` build by 91 minutes |
| gate binary (`jagua-experimental`) | `ship2-gate`, sha256 `f535520ed0a4fe6eaeda376a0ea62345569f72478f8104c8f987c4e44383efbb` |
| measurement binary (full combo) | `ship2-meas`, sha256 `487cb97b2179349c657cdec6f1d71b73b1474a163e527c0d243f6ed7e9e3cd8e` |
| battery binary (§8-§11, §14-§15) | `ship-meas`, sha256 `650092ad2ced463c180c60594b6da78099af434c2571b7beb375ee762ac65297`; §18 gates it equal to `ship2-meas` |

All six builds and their hashes are in `evidence/binaries.txt` and §18 says which
measured what.

---

## The headline

| | |
|---|---|
| **The defect, re-measured under a load this round owns** | `plan=10000` on mixed-61, 20 rounds x 3 seeds at box load median **13.9**: **3 / 2 / 2 distinct depths and 3 / 2 / 3 distinct documents** per seed. On seed 1, **ten runs at 177.9079 and ten at 174.1700** - a 3.738 mm coin toss (§9) |
| **In the heavier window it is worse** | seed 1 chose **four different budgets in ten runs**, spanning 12.4 M to 24.9 M units, and published three depths at a modal share of 0.40 (§11) |
| **The fix that ships** | `plancal=<path>`: a persisted per-box calibration **keyed on `probe_work_units`, which is a counter**. One plan, one depth, **one whole document per seed, 60 of 60 under load** (§9), and 30 of 30 again in the heavier window (§11) |
| **And it is deeper, not only steadier** | under load, against `plan`, per seed **-2.792 / -1.869 / 0.000 mm**; median of seed medians **175.3878 against 176.1620** (§9) |
| **On an unstressed box it costs nothing** | identical depth on all three seeds, wall p50 +0.148 s, p95 +0.039 s, both arms 0 of 30 over target - and it still removes the two splits `plan` shows there (§10) |
| **The trade, named rather than argued** | a work-denominated budget converts box load into **wall** exactly as a wall budget converts it into **depth**. `plan` is 0 of 60 over target under load *by under-buying*; `callive` is 14 of 60 (§9) |
| **The dial, priced** | `planhead=0.85`: overruns **11 of 30 → 2 of 30**, p95 11.328 s → 10.365 s, for one rung on two of three seeds - and still 1.746 mm ahead of the incumbent (§11) |
| **Candidate (a), rejected** | the max-of-k bucket probe saturates against its own clamp on **5 of 9 cells** and is **19 of 30 over target on an unstressed box**, 43 of 60 under load (§8-§10) |
| **Candidate (b), a 572 ns null** | phase 0 is entered at 4.22e-07 s, so "the rate from phase 0's own wall" *is* the division the mode already performs (§3) |
| **Confirmation density: a hard negative, with the cause named** | 12 cells x 2 budget modes. **Every cell exits on `bound` and drops exactly 1.6160 mm.** Depth per thousand slice-units falls **25.7x**, monotonically, in both knobs. Nothing is promoted (§13-§15) |
| **Why it cannot work here** | the coordinator's slice is `SCHEDULE_RUNGS = 9` with `continue_past_bound = false` - a walk of **fixed length**. `record-line-cascade`'s millimetre was bought with `past=1` at a pinned budget, which is the opposite condition (§13.1) |
| **The anytime table** | 27 cells, two processes each: **`callive` reproduced 27/27, `plan` 23/27, `wall` 0/27**, with depth identical on 26 of 27 and 1.731 mm better on the 27th (§16) |
| **Gates** | 4 of 4 pinned on both binaries, whole-document digests **identical to the base binary on all four**; two 9/9 equivalence gates, whole document *and* per-step digest (§17-§18) |
| **Determinism** | work mode 9/9; plan mode 9/9 and calibrated 9/9 unstressed; **both 8/9 under load**, and the calibrated arm's one miss is the band, named to three decimals (§12) |
| **Everything ships off** | `plancal`, `plancalwrite`, `plancalband`, `planprobe`, `m34grid1` and `m34confirm1` all default to the previous round's behaviour |
| **The box** | never quiet. Other agents built and tested on it throughout; §7 is the statement and every table carries its own load range |

**Recommendation.** Ship `plancal` as the way the plan mode is *run*, with the
calibration produced by `drivers/calpass.py` as a declared offline pass and the
file treated as part of the deployment rather than as a cache. Leave
`PLAN_HEADROOM` at 0.97 and hand a deployment that must honour a wall under
contention `planhead` plus §11's price list. **Do not** arm `planprobe`: §8 shows
its estimator measuring its own clamp on five of nine cells. **Do not** promote
either density knob: §15's gate fails at all eleven non-baseline cells, and §13.1
says why in one sentence - the coordinator's slice is a walk of fixed length, so
the knob to spend on next is `continue_past_bound`, not `step_grid`.

---

# Part I — what breaks, and the three things that could fix it

## 1. The defect is one division

`BudgetMeter::install_plan` is four lines and one of them reads a clock:

```
rate  = probe_work_units / probe_seconds
units = probe_work_units + (target * headroom - probe_seconds) * rate / bias
units = floor onto anchor * step^k
```

Substitute the first line into the second and every term but one cancels:

```
raw_units = W0 * ( 1 + (T*h/t0 - 1) / bias )
```

`W0` is `probe_work_units`, a **counter**: `calibrated-plan` §6.1 measured
exactly one distinct value over seven runs on each of three seeds. `T`, `h` and
`bias` are constants. So **the entire run-to-run variation of a calibrated plan
is the variation in `t0`**, the probe's wall, and the ladder
(`PLAN_QUANTUM_STEP = 1.15`) is the only thing standing between it and the
published depth.

The sensitivity is worth writing down exactly, because it is what sizes the
problem. Write it as `raw_units = W0 * (C + A/t0)` with `C = 1 - 1/bias` and
`A = T*h/bias`; at the shipped `bias = 1.70`, `headroom = 0.97`, a ten-second
target and mixed-61 seed 0's calibrated `t0 = 2.2054 s`, `C = 0.4118` and
`A = 5.7059`, so `raw = 2.9990 * W0 = 26,326,915` and the ladder floors it onto
rung 23, **24,891,457** - the plan `calibrated-plan` §8.2 published. The probe
wall that moves that by one rung is:

| | `t0` | ratio to the calibrated reading |
|---|---:|---:|
| one rung **up** (a faster reading) | 1.8787 s | **0.852** |
| the calibrated reading | 2.2054 s | 1.000 |
| one rung **down** (a slower reading) | 2.5982 s | **1.178** |

On a quiet box the probe's spread is 1.2-2.5% (`calibrated-plan` §6.1) and the
rung swallows it seven times over. Under a competing workload it does not: §9
measures the live probe running out to **1.36x** its calibrated value inside one
battery, and the plan following it down a rung.

## 2. Three candidates, and what each can promise

| | mechanism | what it is a function of | what it can promise |
|---|---|---|---|
| **(a)** | `planprobe=<k>`: cut phase 0 into k equal-**work** buckets, price the box at the fastest | a clock, still - but the least-loaded reading available | *less* spread, never *no* spread |
| **(b)** | rate from phase 0's own counters against phase 0's own wall | a clock | — |
| **(c)** | `plancal=<path>`: a persisted per-box calibration, keyed on `probe_work_units` | **counters and a file** | the same document under any load inside the band |

They are not three versions of one idea. (a) and (b) make the reading better;
(c) removes the reading from the decision. Only (c) can make a *gate*, and that
is why it is the one that ships.

## 3. Candidate (b) is a measured null, to seven decimal places

(b) is the cheapest of the three and it turns out to be nothing at all, which is
worth one paragraph rather than a round of work.

`probe_seconds` is `meter.seconds()` - the wall since `BudgetMeter::new` - and
the suggestion is that phase 0's *own* wall would be a cleaner numerator,
because the meter is constructed before phase 0 and picks up whatever setup sits
between. Measured on mixed-61 seed 0 - `evidence/phase-zero-entry.json`, which
is one whole run document kept for exactly this claim:

| | |
|---|---:|
| `phases[0].enteredSeconds` | **4.22e-07 s** |
| `phases[0].elapsedSeconds` | 2.218128875 s |
| `plan.probeSeconds` | 2.218129447 s |
| difference | **572 ns** |

The meter is constructed on the line before phase 0 begins. There is no setup
between them to exclude. "The rate from phase 0's own counters against its own
wall" **is** the division `install_plan` already performs, and it is not built
here because it already exists.

## 4. Candidate (a): the fastest of k work buckets

A loaded box does not make every microsecond slower; it makes *some* of them
slower. So the mean rate over a whole probe is a load-weighted average, and the
**maximum** rate over a sub-window is the closest a run can get to what the box
would have done alone. `PLAN_PROBE_BUCKETS = 8`, and the buckets are cut on the
**work** axis:

* work is a counter, so the same run on a quiet and a loaded box compares the
  same eight stretches of the same computation;
* cutting on the wall axis would compare a loaded second against a quiet second
  and call the difference a rate.

Phase 0 is two monolithic calls - `construct_short_side_first` and one
`improve_complete_layout` - with no budget check and no checkpoint between them,
so there is nowhere inside the search to read a rate from. The sampler is
therefore a thread: `PLAN_PROBE_SAMPLE_MILLIS = 20`, a read of
`profiling::counter_totals()` and a push. It **increments no counter**, so it
cannot move the quantity it is measuring; what it does take is the profiling
registry's mutex, which the search itself takes once per worker thread at
registration and never in a hot path. It is armed only under a plan budget and
only when more than one bucket is asked for, so a default run starts no thread.

`PLAN_PROBE_MIN_FRACTION = 0.5` is the guard, and §8 is where it stops being a
guard and starts being the answer: the estimator saturates against it.

## 5. Candidate (c): a calibration keyed on a counter

```json
{ "version": 1, "entries": { "8778573": { "probeSeconds": 2.2054 } } }
```

**The key is the whole design.** `probe_work_units` is bit-identical across
every run of a (request, seed, binary, feature set), so it identifies the cell
exactly - and it identifies it *including the things that would invalidate the
entry*. A file keyed on a request path would need a policy for a changed binary;
this one **misses**, and a miss falls back to the live probe and says so in the
document.

Given a hit, the plan is a function of `W0` (a counter), the file (a constant)
and three settings. Two processes agree whatever the box is doing. That is the
property `plan=<ms>` was introduced for and has never actually had.

Two decisions inside it are worth naming because the mode would be dishonest
without them:

* **the effective probe replaces the live one *everywhere*, including
  `target * headroom - t0`.** A plan that priced its rate off a file constant
  and then subtracted a clock reading would still put the box's load into the
  rung, one term later. What that costs is stated rather than hidden: under load
  the run believes phase 0 was quicker than it was, buys the budget it would
  have bought on a quiet box, and takes longer in wall for it. §9 prices it.
* **read and write are separate keys.** `plancal=<path>` reads;
  `plancalwrite=1` merges this run's own probe back under the **min** rule. A
  measured battery reads a frozen file and a calibration pass writes it. A run
  that did both would make the file a function of the order the battery
  happened to run in - and the calibration pass itself shows why, because its
  own second round plans off its first round's file.

This is Sol review 8 §3 condition 1, in the form he asked for it: *"il probe
hardware dev'essere offline/persistito e il cap parte della spec."*

## 6. The band, which is where the guarantee stops

`PLAN_CALIBRATION_BAND = 2.0`, one number doing two jobs:

* **live much larger than the file** is the case the mechanism exists for, and
  the file is kept right up to this factor. **The band's size is exactly how
  much load the determinism guarantee survives.**
* **live much smaller than the file** cannot be load. It is a file measured on a
  slower box, a different build, or a cell that collided on the key. Keeping it
  would under-buy for ever.

Outside the band the run falls back to its own probe and writes
`plan.calibrationSource: "fileOutOfBand"` into the **deterministic** half of the
document - so a run that fell back is a run whose digest says it fell back, and
a battery can count them instead of arguing about them.

## 7. The competing load, which this round owns

`docs/experiments/replan/` §7 made every wall claim it had against a load it did
not control: another campaign happened to be on the host, so "loaded" meant
whatever that campaign was doing that minute. That is enough to *find* this
defect and not enough to *fix* it, because a fix has to be measured against a
load that is the same in every arm.

`drivers/stress.py`, and the shape is deliberate:

| | |
|---|---|
| workers | **8** - the engine is pinned at 8 threads on this 16-core box, so eight competitors make it exactly oversubscribed |
| duty | **0.7** of a **250 ms** period |
| what each worker does | one pure-arithmetic loop, no allocation, no I/O, so the contention is for CPU and nothing else |

The duty cycle is the point. A steady 100% load is the *easy* case for a rate
probe - it is a constant, and a constant is calibratable. The case that breaks a
single reading is a load that is not there when the probe looks and is there
afterwards.

`run-load.sh` starts it, waits two seconds for steady state, and kills it on the
way out including on failure - a stress process outliving its battery would
poison the quiet battery that runs next. Its own stdout, one load-average line
per second, is kept in `evidence/stress-loaded.log`.

**The box was not otherwise quiet either.** Other agents were building and
testing on the same host throughout; `os.getloadavg()` is recorded before and
after every process in every driver and every table below carries the range. The
"unstressed" battery is the box as this round found it, not a quiet box, and it
is named that way.

---

# Part II — what it measures

## 8. The calibration pass, and where candidate (a) fails

`drivers/calpass.py`, `evidence/calpass.json`. Three rounds x three fixtures x
three seeds x two estimators, 54 runs, load1 min 2.34 / median 5.09 / max 6.69.
Each pass writes its own file under the min rule; the key is the cell's
`probe_work_units`.

| cell | key (`probeWorkUnits`) | `live.json` (s) | `probe.json` (s) | probe / live |
|---|---:|---:|---:|---:|
| mixed-61 s0 | 8778573 | 2.2054 | 1.7471 | **0.792** |
| mixed-61 s1 | 9629453 | 2.2618 | 1.8279 | **0.808** |
| mixed-61 s2 | 8961342 | 2.2748 | 1.7167 | **0.755** |
| shapes-17 s0 | 1160739 | 0.8911 | 0.4875 | **0.547** |
| shapes-17 s1 | 1154177 | 0.9082 | 0.5117 | **0.563** |
| shapes-17 s2 | 1184708 | 0.9490 | 0.4670 | **0.492** |
| triangle-20 s0 | 6376387 | 0.8556 | 0.4838 | **0.565** |
| triangle-20 s1 | 9249221 | 1.0834 | 0.7212 | **0.666** |
| triangle-20 s2 | 7598248 | 0.9813 | 0.5871 | **0.598** |

**The right-hand column is candidate (a)'s obituary.**
`PLAN_PROBE_MIN_FRACTION` is 0.5, so the ratio cannot go below 0.492 - and on
**five of nine cells it is between 0.492 and 0.60**, which is to say the
estimator is not reporting the box, it is reporting its own clamp. On
shapes-17 seed 2 it is *at* the floor to three decimals.

The reason is visible in the mechanism rather than in the number. Phase 0 is a
constructor followed by one mode-0 pipeline, and those two retire work units at
completely different prices; the cheapest eighth of a 0.9-second phase 0 on
shapes-17 is a stretch of preprocessing that nothing in the rest of the run
resembles. **Max-of-k measures the cheapest tier of the probe, not the least
loaded moment of the box** - it cannot separate them, because both look like a
high rate. `PLAN_PROBE_BUCKETS` therefore buys a systematically shorter probe on
every cell, quiet or loaded, and §9 and §10 measure what that costs: a plan one
to two rungs too large, and **43 of 60** runs over a ten-second target.

On mixed-61, where phase 0 is 2.2 s and dominated by one operator, the ratio is
0.755-0.808 and the estimator is at least measuring something. That is the
narrow band in which candidate (a) is a real mechanism, and it is not wide
enough to ship.

`convergedOnLastRound=False`: the min rule was still lowering entries on the
third round. The file the batteries read is a usable calibration, not a
converged one, and every wall number that reads it is slightly conservative for
that reason.

## 9. The loaded battery: twenty rounds under a load this round generated

`drivers/run-load.sh`, `evidence/battery-loaded.json`. mixed-61, target 10 s,
**20 rounds x 3 seeds x 3 arms**, 180 runs, arm order rotated by round, one
binary, one window, `stress.py 8 0.7` running throughout.

**load1 min 2.44 / median 13.92 / max 16.58 over 180 runs** - a 16-core box at
median 13.9.

| arm | n | wall p50 | wall p95 | wall max | over target |
|---|---:|---:|---:|---:|---:|
| `plan` | 60 | **7.340 s** | 8.502 s | 9.030 s | **0 of 60** |
| `probe` | 60 | 10.878 s | 13.541 s | 14.497 s | **43 of 60** |
| `callive` | 60 | 8.932 s | 10.774 s | 11.204 s | **14 of 60** |

| arm | seed | distinct plans | distinct depths | distinct documents | depth, with counts |
|---|---:|---:|---:|---:|---|
| `plan` | 0 | **3** | **3** | **3** | 178.1798 x12 / 179.5869 x3 / 175.3878 x5 |
| `plan` | 1 | **2** | **2** | **2** | 177.9079 x10 / 174.1700 x10 |
| `plan` | 2 | **3** | **2** | **3** | 176.1620 x17 / 177.3430 x3 |
| `probe` | 0 | 1 | **1** | 1 | 175.1357 x20 |
| `probe` | 1 | **2** | **1** | 2 | 171.3620 x20 |
| `probe` | 2 | 1 | **1** | 1 | 174.2800 x20 |
| **`callive`** | 0 | **1** | **1** | **1** | **175.3878 x20** |
| **`callive`** | 1 | **1** | **1** | **1** | **174.1700 x20** |
| **`callive`** | 2 | **1** | **1** | **1** | **176.1620 x20** |

- `plan`: allSeedsPlanStable=**False** allSeedsDocumentStable=**False** seedMedianOfMedians=**176.1620**
- `probe`: allSeedsPlanStable=False allSeedsDocumentStable=False seedMedianOfMedians=174.2800
- `callive`: allSeedsPlanStable=**True** allSeedsDocumentStable=**True** seedMedianOfMedians=**175.3878**

The live probe wall over all 180 runs: min **2.3091** / median 2.5988 / max
**3.0550** s - a spread of **1.323x**, against §1's 1.178x that moves one rung.
Every `callive` run reported `calibrationSource: "file"`; **not one of the sixty
fell outside the band.**

Four things.

**1. The defect is worse than the round that found it said.**
`docs/experiments/replan/` §11.1 measured `plan=10000` at 2 / 3 / 1 distinct
depths per seed. Under a load this round controls it is **3 / 2 / 2 distinct
depths and 3 / 2 / 3 distinct documents**, with modal shares of 0.60 / 0.50 /
0.85. Seed 1 is the one to read: **ten runs at 177.9079 and ten at 174.1700**,
a 3.738 mm coin toss decided by which of two ladder rungs the probe happened to
land on. That is the same command, twenty times, on one binary, in one window.

**2. `callive` is one plan, one depth and one whole document per seed, 60 of 60,
on a box at load 14.** It is the `calibrated-plan` §8.2 headline, restored and
made unconditional. Every seed chose 24,891,457 units - the rung §1's arithmetic
predicts and the rung that round published.

**3. It is also *deeper*, because the load was costing depth and not only
reproducibility.** Per seed medians, `callive` against `plan`:

| seed | `plan` | `callive` | delta |
|---:|---:|---:|---:|
| 0 | 178.1798 | **175.3878** | **-2.792** |
| 1 | 176.0389 | **174.1700** | **-1.869** |
| 2 | 176.1620 | 176.1620 | 0.000 |
| **median of seed medians** | **176.1620** | **175.3878** | **-0.774** |

(Seed 1's `plan` figure is the median of a ten-ten split and is therefore a
number no run produced; the two values are 177.9079 and 174.1700. It is quoted
as a median because that is what every other row is, and the split is stated so
it cannot be read as a measurement.)

A loaded probe reads a slow box, prices the remaining wall at that slow rate,
and buys a smaller plan - so under load the shipping mode does not merely become
unpredictable, it becomes **conservative in the wrong direction**. Its p50 is
7.340 s of a ten-second target: it is 2.7 seconds under budget and three
millimetres short at the same time.

**4. The wall column is the price, and it is a real one.** `plan` is 0 of 60
over target and `callive` is 14 of 60, thirteen of which are seed 2. Nothing is
wrong with either number: **a work-denominated budget converts box load into
wall exactly as a wall budget converts it into depth.** `plan` honours the
ten seconds under load *by under-buying*, which is the same act as giving up the
2.792 mm. §11 is the dial that makes that choice explicitly rather than by
accident.

The `probe` arm is in this table because the round measured it rather than
argued about it, and it is rejected on the wall column: **43 of 60 over target,
p95 13.541 s.** It is also not fully reproducible - seed 1 chose two budgets in
twenty runs - so it does not even buy the property it costs the wall for. §8
names the reason: on mixed-61 its estimate is 0.755-0.808 of the live probe, and
that is not a load correction, it is a systematic shortening of the probe that
buys one to two rungs it cannot afford.

## 10. The unstressed battery: the quiet-box claim does not regress

`evidence/battery-quiet.json`. Same three arms, 10 rounds x 3 seeds, no
`stress.py`. **load1 min 5.21 / median 7.34 / max 11.77** - the box as this
round found it, with other agents on it, and *not* a quiet box.

| arm | n | wall p50 | wall p95 | wall max | over target | seed medians (mm) |
|---|---:|---:|---:|---:|---:|---|
| `plan` | 30 | 7.233 s | 8.686 s | 8.890 s | **0 of 30** | 175.3878 / 174.1700 / 176.1620 |
| `probe` | 30 | 10.403 s | 12.124 s | 12.190 s | **19 of 30** | 175.1357 / 171.3620 / 174.2800 |
| `callive` | 30 | 7.381 s | 8.725 s | 8.736 s | **0 of 30** | 175.3878 / 174.1700 / 176.1620 |

| arm | seed | distinct plans | distinct depths | distinct documents |
|---|---:|---:|---:|---:|
| `plan` | 0 | **2** | **2** | **2** |
| `plan` | 1 | 1 | 1 | 1 |
| `plan` | 2 | **2** | 1 | **2** |
| `callive` | 0/1/2 | **1 / 1 / 1** | **1 / 1 / 1** | **1 / 1 / 1** |

**`callive` costs nothing here and still fixes something.** Depth is identical to
`plan` on all three seeds, to the digit. Wall is p50 7.381 s against 7.233 s and
p95 8.725 s against 8.686 s - **+0.148 s and +0.039 s** - and both arms are 0 of
30 over target. What it changes is that `plan` *still* split, even unstressed:
two plans and two documents on seed 0, two plans on seed 2, off a live probe
spread of only **1.129x** (2.1840 to 2.4658 s). The rung is 1.178x wide and the
box got within 5% of it with nobody deliberately competing.

That is the "must not regress" check, and the answer is stronger than parity: on
the box this campaign actually runs on, the calibrated arm is the only one of
the two that reproduces.

`probe` is 19 of 30 over target on an *unstressed* box, which settles candidate
(a) without needing the loaded column at all.

## 11. The dial: what a wall promise costs once the depth is a number

§9's last paragraph is a trade and not a defect, and a trade needs a dial. The
dial already exists: `PLAN_HEADROOM`, which the plan multiplies the target by
before it prices anything. On a quiet box its job is to absorb a 1% within-seed
wall spread (`calibrated-plan` §6.2) and 0.97 is generous. On a box expected to
run 1.15x slow it is the place to say so, **once, deterministically**, instead of
letting whichever clock reading the probe caught say it per run.

`evidence/battery-head.json`. Same load generator, 10 rounds x 3 seeds x 3 arms,
90 runs. **This window was much heavier than §9's** - load1 min 7.00 / median
13.62 / **max 26.77**, and the live probe ran out to **6.0625 s** against a
calibrated 2.2054, a spread of **2.549x** - because another agent's build landed
in the middle of it. That makes it the more interesting window, not the less.

| arm | spec | n | wall p50 | wall p95 | wall max | over target | seed medians (mm) | stable |
|---|---|---:|---:|---:|---:|---:|---|---|
| `plan` | `plan=10000` | 30 | 7.473 s | 8.512 s | 8.901 s | **0 of 30** | 178.1798 / 177.9079 / 176.1620 | **no** |
| `callive` | `+plancal` | 30 | 9.467 s | 11.328 s | 11.557 s | 11 of 30 | **175.3878 / 174.1700 / 176.1620** | **yes** |
| `calhead` | `+plancal,planhead=0.85` | 30 | 8.070 s | 10.365 s | 10.638 s | **2 of 30** | 178.1798 / 174.1700 / 176.1620 | **yes** |

| arm | seed | distinct plans, with counts | depths, with counts | distinct documents |
|---|---:|---|---|---:|
| `plan` | 0 | **3**: 21,644,745 x6 / 16,366,537 x3 / 18,821,518 x1 | 178.1798 x6 / 179.5869 x4 | **3** |
| `plan` | 1 | **4**: 21,644,745 x3 / 24,891,457 x3 / 18,821,518 x3 / 12,375,453 x1 | 179.6330 x4 / 177.9079 x3 / 174.1700 x3 | **4** |
| `plan` | 2 | **3**: 21,644,745 x6 / 18,821,518 x3 / 10,761,264 x1 | 176.1620 x6 / 177.3430 x3 / 179.6620 x1 | **3** |
| `callive` | 0/1/2 | 1 / 1 / 1, all 24,891,457 | 1 / 1 / 1 | 1 / 1 / 1 |
| `calhead` | 0/1/2 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / 1 / 1 |

Three readings.

**The incumbent does not degrade gracefully; it falls apart.** On seed 1
`plan=10000` chose **four different budgets in ten runs**, spanning
12,375,453 to 24,891,457 - a factor of two - and published three different
depths spanning **5.463 mm** with a modal share of **0.40**. Ten runs of one
command. Seeds 0 and 2 chose three budgets each. §9's 3/2/2 was the *mild*
version of this.

**Both calibrated arms held, all thirty runs, on a box whose probe spread was
2.5x.** Every run reported `calibrationSource: "file"`; the 6.06 s outlier
landed on a `plan` run. The band was not reached.

**The dial does what a dial should.** `planhead=0.85` takes the overruns from
**11 of 30 to 2 of 30** and the p95 from 11.328 s to 10.365 s, and the price is
one ladder rung on two of the three seeds: seed 0 goes from 175.3878 to
178.1798 and seed 2 stays at 176.1620 while its budget drops from 24,891,457 to
21,644,745. Seed 1 is unmoved in both, at 24,891,457 and 174.1700. Median of
seed medians: **175.3878 for `callive`, 176.1620 for `calhead`, 177.9079 for
`plan`.**

`calhead` is not zero overruns and the round does not round it to zero: two runs
in thirty reached 10.365 s and 10.638 s against a ten-second target, both on
seeds whose budget the fraction did not move. On a box where a *single sample*
of phase 0 ran 2.75x its calibrated value, "2 of 30 over by 6%" is the shape of
what a work budget can promise, and `planhead` is where a deployment that needs
0 of 30 buys it - one more rung down, one more time.

So the ordering is the whole result of Part II. Under contention the choice is
between spending the wall and honouring it, and it always was; what the file
changes is that the choice is now **made in the spec and kept, rather than made
by the box and forgotten**. `calhead` is the arm that meets the wall target
*and* reproduces - and it is still 1.746 mm ahead of the incumbent it replaces.

## 12. Determinism, two processes, with and without the load

The campaign's standing hard gate, run four ways on `ship2-meas`. Three requests
x three seeds; a cell passes only when the two processes agree on
`portfolio.plan.units`, on the tranche sequence, **and** produce identical
documents with `planCalibration` stripped.

| gate | load1 median | cells equal |
|---|---:|---|
| `work=30000000` | 10.26 | **9 of 9** |
| `plan=10000` | 9.36 | **9 of 9** |
| `plan=10000,plancal=live.json` | 9.15 | **9 of 9** |
| `plan=10000`, **under `stress.py 8 0.7`** | 12.35 | **8 of 9** |
| `plan=10000,plancal=live.json`, **under `stress.py 8 0.7`** | 14.37 | **8 of 9** |

`work=30000000` is 9 of 9 and unchanged: the campaign's standing gate is not
moved by anything in this round, which §18's equivalences already implied.

**Both loaded gates are 8 of 9, and the honest reading is that nine cells cannot
separate them.** The batteries can and do - 120 mixed-61 runs in which `plan`
split nine ways across three seeds and `callive` split zero - and a two-process
gate over nine cells is simply a much smaller instrument. What each miss *is*,
though, is worth reading, because they are different failures:

* `plan=10000` missed on **triangle-20 seed 0**: the two processes' probes
  straddled a ladder rung and installed 28,625,176 against 32,918,952. That is
  `calibrated-plan` §7's predicted and only failure mode, and it is the failure
  the file exists to remove.
* `plan=10000,plancal` missed on **shapes-17 seed 1**, and it is the **band**,
  named exactly:

| process | live `t0` | stored | ratio | source | plan |
|---|---:|---:|---:|---|---:|
| a | **2.124 s** | 0.9082 | **2.339** | `fileOutOfBand` | 3,517,876 |
| b | 1.718 s | 0.9082 | 1.892 | `file` | 7,075,705 |

`PLAN_CALIBRATION_BAND` is 2.0 and the two readings fell either side of it. Both
processes published **200.34937729570953** - the identical layout, because
shapes-17 saturates - so this is 8 of 9 as documents and **9 of 9 as layouts**,
and the document difference is a run recording that it declined to use the file.

The mechanism is completely visible in the numbers and it points at a real
limitation: **shapes-17's phase 0 is 0.9 s and mixed-61's is 2.2 s, and a short
probe has more relative jitter, so one fixed band cannot serve both.** Across
§9's, §10's and §11's 120 mixed-61 battery runs at loads up to 26.77, *not one*
run left the band. On the fixture whose phase 0 is 2.4x shorter, one process in
eighteen did. A band that scaled with the probe's own length would close it; it
is not built here, and `plancalband` is the key a deployment sets in the
meantime.

---

# Part III — confirmation density on the first m34 slice

## 13. What the lever is, and what it was bought with last time

`docs/experiments/record-line-cascade/` opened with it:

> The compression schedule stepped its frontier by exactly one canonical grid
> unit [...] That is true of a pose and false of the clamp. `strip_depth_mm` is a
> proxy-tier scalar that `boundary_penalty` reads as a continuous number, so a
> sub-grid step is not a finer move - it is a smaller increment of pressure per
> step, and because `confirm_every` counts *steps*, a quarter step asks the exact
> tier four times as often per micron of descent. At a fixed 20M-unit budget on
> the port's own from-scratch state, `step=1` published 159.102 and `step=0.25`
> published **158.668**.

One millimetre, at a time when a confirmation cost **0.80 ms**.
`calibrated-plan` §4 now prices one at **0.257 ms** with the certificate and the
parallel confirmation both armed - **3.11x** cheaper - so Grok review 3 §item 1
ranks re-running the lever first: *"più pressione exact per millimetro di clamp,
stesso repair. Non è più tassa."* With its gate: *"equal-work sullo stesso
parent, depth per query non peggiore, overrun onesto."*

`m34grid1=<f>` and `m34confirm1=<n>`, **first slice only**, because that is
where the descent is steepest and where the coordinator still has budget to pay
for extra pressure; a run that spent it on every slice would be spending it on
the slices that are already refusing to descend.

The result is a **flat-to-negative sweep at every one of the twelve cells, in
both budget modes**, and the cause is a single column of the table that this
round did not expect to be the interesting one.

## 13.1 The cause: the coordinator's slice is a fixed *distance*, not a budget

Every cell of both sweeps exits on **`bound`**, and every cell's first slice
drops **exactly 1.6160 mm**.

That is `SCHEDULE_RUNGS = 9` and `continue_past_bound = false`. The
coordinator's mode-34 dispatch says so in its own comment - *"the slice **is**
the bound"* - and the constant's doc says why: nine rungs of the engine's own
quantum is 1.568 mm on a 174 mm mixed-61 parent and 0.636 mm on triangle-20's
70.7 mm one, which is what makes the slice portable across requests.

So the coordinator's slice is a **walk of a fixed length**. A quarter-grid clamp
does not walk further; it walks *the same distance in four times as many steps*,
and because `confirm_every` counts steps it also asks the exact tier four times
as often along the way. At 0.125/1 that is **12,928 steps and 9,043
confirmations for the same 1.6160 mm**, against 1,616 and 287 - **25.7x the
work for zero extra depth**.

And there is nothing for the extra confirmations to catch:
`confirmationsAttempted == confirmationsAccepted` in **every cell of both
sweeps**. The exact tier refused nothing. A denser cadence can only help a walk
that is being *refused*, and this one is not.

**`record-line-cascade`'s millimetre was bought under the opposite condition.**
Its arms are `past=1,work=20000000,step=1` against
`past=1,work=20000000,step=0.25` - `continue_past_bound` **on**, at a pinned
work budget. There the walk is budget-limited, and a finer clamp converts the
spare budget into extra distance. Inside the coordinator the walk is
bound-limited and there is no spare budget in the slice to convert: it stops at
1.616 mm and hands the rest back to the queue, where the other classes spend it
better than a denser slice would.

So the lever that matters here is **the bound, not the grid**, and this round
does not touch it. That is the honest form of the negative: not "the sweep was
flat" but "the sweep measured a knob that the operator's stopping rule makes
inert, and the stopping rule is the thing to spend on next."

## 14. The sweep, at the budget the user priority names

`evidence/density-plancal.json`. mixed-61, `plan=10000` **with
`plancal=live.json`**, three seeds, two rounds, twelve cells, 72 runs, load1 min
3.36 / median 7.29 / max 9.57.

This is the round's own instrument's first consumer, and it is used here because
without it the sweep is not a sweep: two cells of the grid can be handed
different budgets by the box, and a cell that drew a bigger plan looks like a
cell that bought depth. With the file every cell runs the same work budget per
seed and the twelve cells differ in the lever and in nothing else.

| step_grid | confirm_every | depth (seed median of medians) | Δ vs 1/4 | first-slice drop | steps | confirms | slice units | mm per 1k units | wall p50 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **1.0** | **4** | **175.3878** | **0.000** | 1.6160 | 1,616 | 287 | 3,455,866 | **0.00047** | 7.752 s |
| 1.0 | 2 | 177.9079 | +2.520 | 1.6160 | 1,616 | 568 | 5,882,184 | 0.00027 | 6.171 s |
| 1.0 | 1 | 177.9079 | +2.520 | 1.6160 | 1,616 | 1,128 | 11,006,184 | 0.00015 | 6.175 s |
| 0.5 | 4 | 177.8842 | +2.496 | 1.6160 | 3,232 | 455 | 6,492,587 | 0.00025 | 8.398 s |
| 0.5 | 2 | 177.8842 | +2.496 | 1.6160 | 3,232 | 903 | 10,431,497 | 0.00015 | 7.316 s |
| 0.5 | 1 | 177.8842 | +2.496 | 1.6160 | 3,232 | 1,795 | 18,593,297 | 0.00009 | 7.542 s |
| 0.25 | 4 | 178.0098 | +2.622 | 1.6160 | 6,464 | 1,244 | 13,896,262 | 0.00012 | 7.826 s |
| 0.25 | 2 | 178.0098 | +2.622 | 1.6160 | 6,464 | 2,479 | 25,196,512 | 0.00006 | 8.157 s |
| 0.25 | 1 | 178.0098 | +2.622 | 1.6160 | 6,464 | 4,950 | 47,806,162 | 0.00003 | 8.757 s |
| 0.125 | 4 | 178.0028 | +2.615 | 1.6160 | 12,928 | 2,268 | 26,923,545 | 0.00006 | 12.142 s |
| 0.125 | 2 | 178.0028 | +2.615 | 1.6160 | 12,928 | 4,529 | 47,611,695 | 0.00003 | 12.662 s |
| 0.125 | 1 | 178.0028 | +2.615 | 1.6160 | 12,928 | 9,043 | 88,914,795 | 0.00002 | 14.703 s |

**Twelve cells, every one exits on `bound`, every one drops 1.6160 mm, and the
baseline wins by 2.496 to 2.622 mm.** The `0.125/1` cell's first slice alone
asks for 88,914,795 work units against a plan of 24,891,457 - it is 3.6x the
whole budget in one action, and the wall p50 is 14.7 s against a ten-second
target.

The overrun column is stated honestly rather than hidden: the three `0.125`
cells run 12.1 to 14.7 s at a ten-second target, because the coordinator
dispatches the slice on an estimate and mode 34's own cap
(`docs/experiments/replan/` §4) is not armed in this sweep.

`evidence/density-plan.json` is the same grid at `plan=10000` **without** the
calibration file, run first as a control. It reproduces this table cell for cell
- same twelve depths, same twelve deltas, same drops - so on this fixture the
plan was not in fact confounding the sweep. That is a piece of luck rather than
a property, and the pinned version is the one quoted because it is the one whose
validity does not depend on it.

## 15. Grok's equal-work gate

`drivers/density.py`, `evidence/density-work.json`. `work=30000000`, mixed-61,
three seeds, twelve cells. Equal work, and the first slice's parent is
identical across all twelve cells by construction: nothing the two knobs touch
runs before the first m34 dispatch, and the driver checks it - three distinct
first-slice parents over the whole sweep, **179.753 / 179.638 / 179.008, one per
seed.**

| step_grid | confirm_every | depth | Δ vs 1/4 | first-slice drop | steps | confirms | slice units | **mm per 1k units** | exit |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| **1.0** | **4** | **173.5751** | **0.000** | 1.6160 | 1,616 | 287 | 3,455,866 | **0.00047** | bound 3/3 |
| 1.0 | 2 | 175.1357 | +1.561 | 1.6160 | 1,616 | 568 | 5,882,184 | 0.00027 | bound 3/3 |
| 1.0 | 1 | 177.9079 | +4.333 | 1.6160 | 1,616 | 1,128 | 11,006,184 | 0.00015 | bound 3/3 |
| 0.5 | 4 | 175.0060 | +1.431 | 1.6160 | 3,232 | 455 | 6,492,587 | 0.00025 | bound 3/3 |
| 0.5 | 2 | 175.1588 | +1.584 | 1.6160 | 3,232 | 903 | 10,431,497 | 0.00015 | bound 3/3 |
| 0.5 | 1 | 177.8842 | +4.309 | 1.6160 | 3,232 | 1,795 | 18,593,297 | 0.00009 | bound 3/3 |
| 0.25 | 4 | 177.3640 | +3.789 | 1.6160 | 6,464 | 1,244 | 13,896,262 | 0.00012 | bound 3/3 |
| 0.25 | 2 | 178.0098 | +4.435 | 1.6160 | 6,464 | 2,479 | 25,196,512 | 0.00006 | bound 3/3 |
| 0.25 | 1 | 178.0098 | +4.435 | 1.6160 | 6,464 | 4,950 | 47,806,162 | 0.00003 | bound 3/3 |
| 0.125 | 4 | 178.0028 | +4.428 | 1.6160 | 12,928 | 2,268 | 26,923,545 | 0.00006 | bound 3/3 |
| 0.125 | 2 | 178.0028 | +4.428 | 1.6160 | 12,928 | 4,529 | 47,611,695 | 0.00003 | bound 3/3 |
| 0.125 | 1 | 178.0028 | +4.428 | 1.6160 | 12,928 | 9,043 | 88,914,795 | 0.00002 | bound 3/3 |

**The gate fails on every one of the eleven non-baseline cells, on both of its
halves.** Final depth is worse everywhere, by 1.431 to 4.435 mm. Depth per
thousand of the slice's own work units - Grok's "depth per query", in the
currency the slice charges itself in - falls **monotonically** down the table,
from the unrounded 0.000466 to 0.000018: a factor of **25.7**. And it falls
monotonically in *both* knobs separately, which is what makes it a property of
the lever rather than of one corner of the grid.

The plan-mode sweep in §14 is the same shape at the budget the user priority
names. There is no winner, so **nothing is promoted**: `m34grid1` and
`m34confirm1` ship as spec keys at the module's own `1.0` and `4`, and this
section is the reason.

---

# Part IV — the table, the gates, and what is still open

## 16. The anytime table, and Sparrow

**Three fixtures, three seeds, two processes per cell, three arms, one binary
(`ship2-meas`), one window.** `drivers/anytime.py`, `evidence/anytime.json` and
`evidence/anytime30.json`.

The three arms are not the same measurement and the table must not be read as if
they were: `wall=<ms>` gets the whole target as useful search and is not
reproducible; `plan=<ms>` is `calibrated-plan`'s shipping mode, reproducible on
a quiet box; `plan=<ms>,plancal=<file>` is this round's, reproducible on the box
it was calibrated on.

### 16.1 Three and ten seconds

load1 min 5.00 / median 7.72 / max 13.37 over 108 runs

| fixture | target | arm | seed depths (mm) | median | wall max | **reproduced** | over target |
|---|---:|---|---|---:|---:|---:|---:|
| mixed-61 | 3 s | `plan` | 181.589 / 179.690 / 179.662 | **179.690** | 2.40 s | 3/3 | 0/3 |
| mixed-61 | 3 s | `callive` | 181.589 / 179.690 / 179.662 | **179.690** | 2.41 s | **3/3** | 0/3 |
| mixed-61 | 3 s | `wall` | 179.587 / 179.633 / 179.006 | **179.587** | 2.75 s | 0/3 | 0/3 |
| mixed-61 | 10 s | `plan` | 175.388 / 174.170 / 176.162 | **175.388** | 8.56 s | 3/3 | 0/3 |
| mixed-61 | 10 s | `callive` | 175.388 / 174.170 / 176.162 | **175.388** | 8.51 s | **3/3** | 0/3 |
| mixed-61 | 10 s | `wall` | 171.111 / 165.656 / 174.280 | **171.111** | 10.51 s | 0/3 | 1/3 |
| shapes-17 | 3 s | `plan` | 200.349 / 200.349 / 200.349 | **200.349** | 3.47 s | 2/3 | 2/3 |
| shapes-17 | 3 s | `callive` | 200.349 / 200.349 / 200.349 | **200.349** | 3.48 s | **3/3** | 2/3 |
| shapes-17 | 3 s | `wall` | 200.349 / 200.349 / 200.349 | **200.349** | 1.89 s | 0/3 | 0/3 |
| shapes-17 | 10 s | `plan` | 200.349 / 200.349 / 200.349 | **200.349** | 8.50 s | 2/3 | 0/3 |
| shapes-17 | 10 s | `callive` | 200.349 / 200.349 / 200.349 | **200.349** | 8.50 s | **3/3** | 0/3 |
| shapes-17 | 10 s | `wall` | 200.349 / 200.349 / 200.349 | **200.349** | 11.49 s | 0/3 | 1/3 |
| triangle-20 | 3 s | `plan` | 70.771 / 70.747 / 70.747 | **70.747** | 2.22 s | 3/3 | 0/3 |
| triangle-20 | 3 s | `callive` | 70.771 / 70.747 / 70.747 | **70.747** | 2.33 s | **3/3** | 0/3 |
| triangle-20 | 3 s | `wall` | 70.771 / 70.747 / 70.747 | **70.747** | 3.28 s | 0/3 | 1/3 |
| triangle-20 | 10 s | `plan` | 70.742 / 70.746 / 70.742 | **70.742** | 8.00 s | 2/3 | 0/3 |
| triangle-20 | 10 s | `callive` | 70.742 / 70.746 / 70.742 | **70.742** | 8.04 s | **3/3** | 0/3 |
| triangle-20 | 10 s | `wall` | 70.730 / 70.730 / 70.731 | **70.730** | 9.81 s | 0/3 | 0/3 |

**`callive` reproduced 18 of 18 cells; `plan` reproduced 15 of 18; `wall`
reproduced 0 of 18.** And `callive`'s depth is *identical to `plan`'s on every
one of the eighteen cells, to the digit* - the calibration is free here and buys
the three cells `plan` lost. The wall difference is at most 0.11 s on any row.

That is the shape of the whole result restated at a different scale: on a box
that is not being deliberately loaded, the file costs nothing and removes the
residual splits; under load (§9-§11) it is the difference between one answer and
nine.

### 16.2 Thirty seconds, on all three fixtures

load1 min 2.49 / median 4.25 / max 7.78 over 54 runs

| fixture | target | arm | seed depths (mm) | median | wall max | reproduced | over target |
|---|---:|---|---|---:|---:|---:|---:|
| mixed-61 | 30 s | `plan` | 164.188 / **167.666** / 164.171 | **164.188** | 36.53 s | 3/3 | 1/3 |
| mixed-61 | 30 s | `callive` | 164.188 / **165.935** / 164.171 | **164.188** | 36.53 s | 3/3 | 1/3 |
| mixed-61 | 30 s | `wall` | 163.927 / 160.010 / 166.666 | **163.927** | 41.29 s | 0/3 | 2/3 |
| shapes-17 | 30 s | `plan` | 200.349 / 200.349 / 200.349 | **200.349** | 18.39 s | 3/3 | 0/3 |
| shapes-17 | 30 s | `callive` | 200.349 / 200.349 / 200.349 | **200.349** | 18.39 s | 3/3 | 0/3 |
| shapes-17 | 30 s | `wall` | 200.349 / 200.349 / 200.349 | **200.349** | 17.72 s | 0/3 | 0/3 |
| triangle-20 | 30 s | `plan` | 70.730 / 70.730 / 70.729 | **70.730** | 18.93 s | 2/3 | 0/3 |
| triangle-20 | 30 s | `callive` | 70.730 / 70.730 / 70.729 | **70.730** | 18.91 s | **3/3** | 0/3 |
| triangle-20 | 30 s | `wall` | 70.727 / 70.727 / 70.727 | **70.727** | 29.22 s | 0/3 | 0/3 |

**Over the whole 27-cell table: `callive` reproduced 27 of 27, `plan` 23 of 27,
`wall` 0 of 27.** Depth is identical between the two plan arms on 26 of the 27
cells; the twenty-seventh is mixed-61 seed 1 at thirty seconds, where `callive`
publishes **165.935** against `plan`'s **167.666** - **1.731 mm**, on the one
cell where the box's load moved a rung.

**The thirty-second overrun survives, unchanged and undisguised.** Both plan arms
reach **36.53 s** against a 30 s target on mixed-61. That is
`docs/experiments/replan/` §12.2's finding reproduced on this tree: the overrun
belongs to the work-denominated modes, the re-plan reduced it by four seconds
and did not remove it, and **nothing in this round addresses it either.** The
file makes the thirty-second answer a *number*; it does not make it arrive in
thirty seconds.

### 16.3 Against Sparrow

Sparrow on this same x86_64 box, seed 0, 8 workers, from
`docs/experiments/sparrow-mixed61/` §"x86_64 same-machine addendum" -
**157.971 mm at three seconds and 150.165 mm at ten**, both exact-valid, both
taken on a quiet box.

| budget | Sparrow | this round, `wall` | `plan` | **`callive`** | gap, best arm |
|---|---:|---:|---:|---:|---:|
| 3 s | 157.971 | 179.587 | 179.690 | 179.690 | **21.6 mm** |
| 10 s | 150.165 | 171.111 | 175.388 | **175.388** | **21.0 mm** |
| 30 s | not published | 163.927 | 164.188 | 164.188 | - |

**The gap is not moved by this round and this round does not claim to move it.**
At ten seconds it is 21.0 mm against the un-reproducible `wall` arm and 25.2 mm
against the reproducible one. What this round changes is not the gap but how
much of the left-hand column survives contact with a busy box: §9 measured the
same command producing 178.180 and 174.170 ten times each on one seed, and the
`wall` arm's own 171.111 here is one draw of a distribution
`calibrated-plan` §8.1 measured at 168.484 / 169.588 / 171.111.

## 17. The four pinned gates, and the whole document

Both binaries built from this worktree; the gate binary is
`--features jagua-experimental`, which compiles neither the compression schedule
nor the parallel confirmation, so **none of this round's four spec keys exists in
it** - `m34grid1` and `m34confirm1` are `#[cfg]`-gated off it entirely and it
exits non-zero on them.

| gate | pinned | reproduced | whole-document digest | ship == base |
|---|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | **yes** | `6d22b8a9d4b74455` | **yes** |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | **yes** | `c78b71ee6104de60` | **yes** |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | **yes** | `8a2e364228ffd6e8` | **yes** |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | **yes** | `0b44d5708a0a1381` | **yes** |

`ALL_PASS: true` on both binaries, and the check that matters because a pinned
scalar is four numbers out of a document of thousands: **the whole-document
digest, with the wall-clock and provenance fields stripped, is identical between
this tree's gate binary and the campaign base commit's on all four gates.**

`evidence/gates-ship.json`, `evidence/gates-base.json`, `drivers/gates.py`.

## 18. The two equivalences, and the two binaries this round measured

Six builds, and the round says which measured what rather than rounding them
into one. Every hash is in `evidence/binaries.txt`.

| build | sha256 (16) | what it is | what it measured |
|---|---|---|---|
| `base-gate` | `7a6487613f5de94f` | campaign base `8e7f82e`, `jagua-experimental` | §17's left-hand side |
| `base-meas` | `10e2d46835466aed` | campaign base, full combo | the refactor gate's left-hand side |
| `ship-meas` | `650092ad2ced463c` | this tree before §18's three cleanups | §8-§11's calibration pass and three batteries, §14-§15's sweeps |
| `ship-gate` | `e7781a4bc3467698` | the same, `jagua-experimental` | nothing; superseded |
| **`ship2-gate`** | **`f535520ed0a4fe6e`** | the committed tree, `jagua-experimental` | **§17's four pinned gates** |
| **`ship2-meas`** | **`487cb97b2179349c`** | the committed tree, full combo | **§16's anytime table, §12's determinism, both equivalences** |

`ship-meas` and `ship2-meas` differ by three cleanups made after the batteries
had started and before anything else ran: `planprobe=on` as a name for
`PLAN_PROBE_BUCKETS`, a clamp on the last bucket boundary that is inert for any
power-of-two bucket count, and `#[cfg]`-gating the two `m34*1` spec keys off a
build with no compression schedule. None of the three is reachable from a work
budget - **and the round gates that rather than arguing it.**

`drivers/equiv.py`. One work budget, three fixtures, three seeds; the comparison
is the whole document **and** the per-step FNV digest of every m34 slice in it,
which compares each step's clamp, sweeps, candidate queries, pair and boundary
counts before and after, and the confirmation's three outcomes.

| gate | cells | whole document | every step digest |
|---|---:|---|---|
| `base-meas` vs `ship2-meas`, `work=30000000` | 9 | **equal 9/9** | **equal 9/9** |
| `ship-meas` vs `ship2-meas`, `work=30000000` | 9 | **equal 9/9** | **equal 9/9** |

The first is the round's refactor gate: everything in Part I is semantics
preserving in the work currency, which it has to be, because the whole mechanism
only ever changes **which work budget is installed** and never what a budget
buys. The second is the two-binary join, and it means every number in this
document was produced by one of two binaries that produce the same document.

`evidence/equiv.json`, `evidence/equiv12.json`.

## 19. Suites

`drivers/run-suites.sh`, exit status captured **directly** rather than through a
pipe, because `cargo test ... | tee log` reports `tee`'s status and that is how a
red suite gets written up as green.

| suite | features | exit | tests |
|---|---|---:|---|
| `suite-jagua` | `jagua-experimental` | **0** | 1,281 passed, 0 failed |
| `suite-combo` | the protocol's full combo | **0** | 1,341 passed, 0 failed |

`EXITS jagua=0 combo=0`. Both passed on the first attempt, including the
campaign's known flake
(`free_material_multi_eviction_shrinks_retained_container_capacity`), which did
not need a rerun. Logs: `evidence/suite-jagua.log`, `evidence/suite-combo.log`.

The round's five new tests are in `search::portfolio` and compile in **both**
feature sets, because every mechanism in Part I is `#[cfg]`-free:

| test | what it pins |
|---|---|
| `an_unarmed_plan_is_the_shipped_plan_and_reads_no_file` | a spec that names none of the keys is the mode `calibrated-plan` shipped: source `Live`, effective probe == live probe, no sampler thread, both density knobs `None` |
| `the_max_of_k_probe_takes_the_fastest_bucket_and_is_clamped` | the bucket cut, the interpolation, and both ends of `PLAN_PROBE_MIN_FRACTION` - including a spike the clamp flattens and a milder one it passes through |
| `a_persisted_calibration_makes_two_loaded_probes_install_one_budget` | the central claim, at unit scale: two meters whose probes differ by 1.9x install the same budget through the file, and the same pair without it installs two |
| `a_calibration_outside_the_band_is_refused_in_both_directions` | the band, both ways, and that a refusal is a fallback rather than a failure |
| `the_calibration_file_keeps_the_least_loaded_observation` | the min rule, the write-if-better margin, that a second cell does not disturb the first, and that a corrupt file degrades to the live probe |

## Honest caveats

* **The calibration file is a promise about a box, and this round measured one
  box.** Every entry in `evidence/cal-live.json` is an x86_64, 16-core,
  8-engine-thread number. On a different box the file misses on nothing - the
  key is a counter and the counter does not change - so it *hits* with a wall
  that is wrong for that box, and only the band catches it. The band catches a
  factor of two. A deployment moving between machines calibrates per machine or
  it is running someone else's plan.
* **The determinism guarantee is bounded by the band, and the band is a
  constant fitted to nothing.** `PLAN_CALIBRATION_BAND = 2.0` is a choice, not a
  measurement: it is wide enough for the load this round generated and narrow
  enough to reject a file from a box twice as fast. No experiment here places
  it, and a deployment with a different load distribution should place it with
  `plancalband`.
* **The calibration pass had not converged.** `evidence/calpass.json` reports
  `convergedOnLastRound=False` over three rounds: the min rule was still
  lowering entries on the last round it ran. The file the batteries used is
  therefore a *usable* calibration and not a converged one, and every wall
  number that reads it is very slightly conservative for that reason.
* **A work-denominated budget cannot promise a wall, and no calibration changes
  that.** This is the round's central trade and §9 is where it is priced rather
  than argued. A fixed plan on a box 1.3x slower takes 1.3x the queue wall; a
  wall budget on the same box gives up depth instead. `plancal` does not remove
  the choice, it makes the choice **once and deterministically** instead of
  letting whichever clock reading the probe caught make it per run.
* **The sampler is a thread, and a thread is not free.** It wakes 50 times a
  second and reads the counter registry behind its mutex. Nothing here measures
  its cost separately from the arm that carries it, so `probe`'s wall numbers
  include whatever it is, and it is off by default.
* **Nothing here is wired into a production route.** `plancal`, `planprobe`,
  `m34grid1` and `m34confirm1` are spec keys on the benchmark example, and the
  coordinator that reads them is still `coordinator_v3`, which is still off by
  default.

## Reproducing this

```
cargo build --release --example general_request_benchmark \
  --features jagua-experimental
cargo build --release --example general_request_benchmark --features \
  jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

# the offline calibration pass - three rounds, both estimators, both files
python3 drivers/calpass.py <out>/calpass <meas-binary> 3

# the two batteries; the first starts and owns its own competing load
bash drivers/run-load.sh <out>/battery-loaded <meas-binary> 10000 0,1,2 20 \
     plan,probe,callive 8 0.7
python3 drivers/planbattery.py <out>/battery-quiet <meas-binary> mixed-61 \
     10000 0,1,2 10 plan,probe,callive

# the confirmation-density sweep, and Grok's equal-work gate
python3 drivers/density.py <out>/density-plan <meas-binary> plan 10000 2 0,1,2
python3 drivers/density.py <out>/density-work <meas-binary> work 30000000 1 0,1,2

# the gates, the refactor equivalence, determinism, the anytime table
python3 drivers/gates.py ship <gate-binary> <out>/gates-ship
python3 drivers/equiv.py <out>/equiv <base-meas> <meas-binary> \
     mixed-61,shapes-17,triangle-20 0,1,2 30000000
python3 drivers/determinism.py <out>/det <meas-binary> \
     mixed-61,shapes-17,triangle-20 0,1,2 plan 10000 plancal=<live.json>
python3 drivers/anytime.py <out>/anytime <meas-binary> \
     mixed-61,shapes-17,triangle-20 0,1,2 3000,10000 plan,callive,wall

bash drivers/run-suites.sh
bash drivers/collect.sh                       # summaries into evidence/
python3 drivers/tables.py docs/experiments/robust-plan/evidence
```

`drivers/runlib.py` and `drivers/gatelib.py` carry the pinned CLI tail, the
`0.002` search-offset allowance, the salt sets and the request table; their
`ROOT` points at this worktree.

The levers, one line each:

```
'plan=10000,plancal=/path/live.json,cells=13:15:17:19,v3=1'   # the calibrated plan
'plan=10000,plancal=/path/live.json,plancalwrite=1,...'       # the calibration pass
'plan=10000,planprobe=on,...'                                 # the max-of-k probe
'plan=10000,plancal=...,planhead=0.85,...'                    # the load-budgeted plan
'plan=10000,m34grid1=0.25,m34confirm1=2,...'                  # confirmation density
'work=<units>,cells=...,v3=1'                                 # replay any of them exactly
```

A caller who wants the guarantee without carrying a file still takes
`portfolio.plan.units` and replays it with `work=`. That recommendation is
unchanged from `calibrated-plan` §14 and `replan` §15; what the file adds is
that the *first* run agrees too.
