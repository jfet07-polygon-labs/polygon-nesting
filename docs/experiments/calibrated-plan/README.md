# The shipping configuration, and a wall target that is a work plan

Two things, and they are one thing. The first arms the configuration the
previous three rounds recommended and never turned on. The second makes the
number that configuration produces **reproducible**, which is the property
every millimetre in this campaign has silently lacked.

The second is the honest half. `docs/experiments/sparse-rotation/` §7.2 measured
an *unchanged* arm publishing medians 2-5 mm apart between sessions on the same
fixture and the same seed, and `docs/sol-review-5-se2-and-pose-freedom.md` §5
refuses to accept a work envelope as a wall envelope. A run cannot have a wall
budget and a per-seed-reproducible depth at the same time: the wall budget
converts box load into depth by construction. The fix Sol names is a **fixed
work plan calibrated under the wall**, and that is what `plan=<ms>` is.

It works, it reproduces, and it costs. All three are measured below.

---

## The headline

| | |
|---|---|
| **Promoted defaults, inside v3 only** | `m34pconfirm=1` and the clearance certificate, with `m34pconfirm=0` / `fcv=0` as live opt-outs |
| **Pinned gates** | 4 of 4 hit, and the **whole-document digest is identical** to the base binary on all four |
| **The promotion, as a document** | 24 runs, six arms, one document per cell: `m34pconfirm` and `fcv` are semantics-preserving in the work currency and move only wall |
| **The promotion, as wall** | **3.11x / 3.17x** faster per accepted confirmation on mixed-61 than the base binary's default, **1.73x / 1.61x** on triangle-20 |
| **`plan=<ms>`** | a wall target spent as a work budget the coordinator sizes from its own phase 0, reported so it can be replayed with `work=` |
| **What one command answers at `wall=10000`** | mixed-61 seed 0, twenty runs, one binary, one afternoon: **three different depths**, spanning **2.627 mm** - two of which two previous chapters published as separate results (§8.1) |
| **What `plan=10000` answers** | the same twenty runs: **one plan, one depth, one document, per seed** - and **0 of 60 runs over the target**, against the wall arm's 21 of 60 (§8.2). **Quiet-box only**: under load the same arm splits 2 / 3 / 1 (`replan` §11.1) and the fix is `plancal` (`robust-plan` §9). See §8.2's banner |
| **The canonical production table** | 27 cells, two processes each: **`plan` reproduced 25, `wall` reproduced 0** (§10) |
| **The price of that** | over nine (fixture, budget) rows, a **median of +0.000 mm** - seven rows at parity, one 1.074 mm better, and the entire cost on mixed-61 at ten seconds at +6.904 mm (§10.1) |
| **Determinism, two processes** | 9/9 work mode, 9/9 plan mode |
| **Suites** | both pass first attempt, exits 0 and 0 |

The rest, including what the plan mode costs and the three things that take it,
is measured below and summarised in §9.

---

# Part I — arming the promotion

## 1. What changed, and what a caller sees

`docs/experiments/fast-contract-validator/` §13.2 lists four conditions on
default-on and then declines to flip anything: *"the recommendation is the
deliverable"*. Three of the four were already met by that round. The fourth was
a lever. This round adds the lever and flips the two defaults **inside the v3
coordinator**, which is itself still off by default.

Three changes, and the third is the only one a caller can see without asking
for it:

| change | where | what a caller sees |
|---|---|---|
| `PortfolioSettings::fast_contract_validator`, default `true` | `search::portfolio` | nothing: the certificate was *already* unconditional whenever `fast-contract-validator` was compiled. What is new is that `fcv=0` can now take it off. |
| `set_contract_certificate_armed` + `ContractCertificateArming` | `validation::general_polygon`, `search::portfolio` | nothing, unless they pass `fcv=0`. A process-wide `AtomicBool`, written by a v3 run before it starts and restored by `Drop` on the way out. |
| `compression_schedule_parallel_confirm`, default `false` → **`true`** | `search::portfolio` | **a v3 run that names no key now runs the parallel confirmation.** This is the promotion. |

Four things that did **not** change, each of which is a way this could have gone
wrong:

* **The Cargo features are still off by default.** `fast-contract-validator`
  and `parallel-compression-schedule` are opt-in at build time exactly as
  before. The default build does not compile either field, so this is *a
  default within a flag* - the same shape as `m34wall` and `m34bit`, which have
  shipped `true` inside v3 for three rounds while v3 itself shipped off.
* **`coordinator_v3` is still `false`.** Asserted in
  `the_promoted_defaults_are_on_inside_v3_and_v3_is_off`.
* **The v2 phase schedule cannot read either flag.** The only read of
  `compression_schedule_parallel_confirm` in the crate is `execute_v3_action`'s
  mode-34 dispatch, and the certificate arming is constructed only when
  `settings.coordinator_v3` is set.
* **`compression_schedule_lanes` is *not* promoted** and stays `1`. It is the
  one lever in this family that is not semantics-preserving, and nothing in
  §12-13 of the validator round argued for it.

The **opt-outs stay**, and §13.2(4) is why. `pconfirm`'s 1.5 mm is contingent on
spare cores; on a contended box it decays to parity with the serial arm, whose
depth is a constant. A deployment that cannot promise the cores, or that wants
the serial arm's cross-round reproducibility, sets `m34pconfirm=0` and gets
exactly the previous default back - measured, not asserted, in §3 claim 4.

An **unarmed** binary refuses both keys rather than running the other arm under
their label: `fcv=0` and `m34pconfirm=0` are `#[cfg]`-gated spec keys, and a
build without the feature exits non-zero with `unknown portfolio spec key`.
Measured in §4.

## 2. The four pinned gates, and the whole document

Both binaries built from this worktree; the gate binary is
`--features jagua-experimental`, which compiles **neither** of the promoted
features, so the fields above do not exist in it.

| gate | pinned | reproduced | wall |
|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | yes | 26.34 s |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | yes | 3.20 s |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | yes | 3.43 s |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | yes | 3.08 s |

`ALL_PASS: true`. And the check that actually matters, because a pinned scalar
is four numbers out of a document of thousands: the **whole-document digest,
with the wall-clock and provenance fields stripped, is identical across all
three gate binaries on all four gates** - the base commit's, this tree's as the
batteries ran it, and this tree's as committed:
`012a8eb1b24c3090`, `28b58a25e68fc205`, `11afd25f7989e283`,
`f80c2a17412141ef`. See §12.1 for the three hashes.

`evidence/gates-plan-gate.json`, `evidence/gates-base-gate.json`,
`evidence/gates-ship-gate.json`, `drivers/gates.py`.

## 3. The arming gate: six arms, one document per cell

`drivers/armgate.py`, `evidence/armgate.json`. Two requests x two seeds x six
arms at a **work** budget of 30 M units, so both sides are deterministic and the
box cannot be the difference. 24 runs.

| claim | verdict |
|---|---|
| 1. `<default>` == `m34pconfirm=1,fcv=1` | **PASS**, 4/4 |
| 2. `<default>` == base binary's `m34pconfirm=1` | **PASS**, 4/4 |
| 3. `<default>` == `m34pconfirm=0`, as a document | **PASS**, 4/4 |
| 4. `m34pconfirm=0` == base binary's `<default>` | **PASS**, 4/4 |
| 5. `<default>` == `fcv=0`, as a document | **PASS**, 4/4 |

Claim 2 is the promotion: **the new binary's default is the old binary's
shipping configuration**, field for field. Claim 4 is its complement: opting out
reproduces the old default exactly, so nothing became unreachable.

Claims 3 and 5 are *expected* equalities and it is worth saying why, because a
reader could take them for a broken lever. Neither promoted default touches
`Counter::ExactPairTests`, which is what the work meter is made of: the
certificate is a **proof** of clearance rather than an estimate, and the
parallel confirmation returns the serial function's verdict including its
message. So at a work budget both arms take the same branches and produce the
same layout, to every digit -

| cell | depth, all six arms |
|---|---:|
| mixed-61 seed 0 | 173.57506393348288 mm |
| mixed-61 seed 1 | 171.3619986855876 mm |
| triangle-20 seed 0 | 70.7711336311948 mm |
| triangle-20 seed 1 | 70.74684851410467 mm |

What they change is **wall**, which is why the microbenchmark below is the half
of claims 3 and 5 that carries the weight.

## 4. Both opt-outs are live, and the promotion is worth 3.1x

Milliseconds per **accepted confirmation**, summed over the whole run and taken
from the runs that produced the depths above - the same instrument
`fast-contract-validator/drivers/factorial.py` §12 used.

| arm | mixed-61 s0 | mixed-61 s1 | triangle-20 s0 | triangle-20 s1 |
|---|---:|---:|---:|---:|
| **new default** (both armed) | **0.2573** | **0.2539** | **0.0588** | **0.0614** |
| `m34pconfirm=0` | 0.8052 | 0.8138 | 0.1029 | 0.1009 |
| `fcv=0` | 0.9618 | 0.8559 | 0.0860 | 0.0912 |
| base binary's default | 0.7991 | 0.8056 | 0.1018 | 0.0988 |
| accepted confirmations | 287 | 308 | 74 | 61 |

Read three ways:

* **The promotion.** New default against the base binary's default:
  **3.11x** and **3.17x** on mixed-61, **1.73x** and **1.61x** on triangle-20.
  That is what turning `m34pconfirm` on is worth on top of a certificate that
  was already armed.
* **The opt-outs are live.** Every ratio is above 1.05 on every cell -
  `m34pconfirm=0` costs 3.13x/3.21x/1.75x/1.64x, `fcv=0` costs
  3.74x/3.37x/1.46x/1.48x. A key that changed nothing would show the same
  milliseconds.
* **The two compose, and which of them is larger depends on the request.**
  On mixed-61 removing the certificate costs more than removing the parallel
  confirmation (3.74x / 3.37x against 3.13x / 3.21x); on triangle-20 the order
  reverses (1.46x / 1.48x against 1.75x / 1.64x). The `pconfirm` half of that
  is new - `fast-contract-validator` §12's factorial is mixed-61 only. The
  certificate half is not directly comparable to §10.3's **1.9733x** on
  triangle-20: that number is fcv-on against fcv-off with the confirmation
  **serial**, from a pinned parent at equal walk, and this one is measured on
  top of an already-parallel confirmation from the bare request, which is the
  smaller remaining share by construction.

The unarmed binary refuses both keys: exit 1, `unknown portfolio spec key`.

**This box had 8 threads available** (`actualThreads: 8` on a 16-core host), so
the `pconfirm` numbers here are a spare-cores result and §13.2(4)'s
qualification applies to them unchanged.

---

# Part II — the calibrated work plan

## 5. What `plan=<ms>` is

One new budget, `PortfolioBudget::Plan { target_millis }`, and its whole life is
one window - from the meter's construction to the end of the protected phase 0,
which is the one stretch of a run that is never budget-checked:

```
BudgetMeter::new(Plan { target_millis })      // budget recorded, never spent against
  -> the protected phase 0 runs                // never budget-checked, by design
  -> BudgetMeter::install_plan(..)             // one clock read, one arithmetic, done
  -> PortfolioBudget::Work { units }           // everything after this is a work run
```

The arithmetic is one line:

```
rate  = probe_work_units / probe_seconds                  // phase 0's own rate
units = probe_work_units
      + (target * headroom - probe_seconds) * rate / bias
units = floor onto  anchor * step^k
```

Four properties, and each of them is a decision this document has to defend:

* **The probe is phase 0, not a synthetic slice**, and the choice is made on
  four measured grounds rather than on taste. (i) It is **free**: it runs in
  every coordinator run already, and a dedicated probe would have to be charged
  to the same budget it is measuring, so its cost comes straight out of the
  search. (ii) It is the **longest** sample available at no cost - 2.2 s on
  mixed-61, 0.87-0.92 s on the other two - and the spread of a rate estimate is
  a function of how long you watched. (iii) Its **work** is a counter and
  therefore exactly reproducible (§6.1), so it contributes no variance of its
  own. (iv) It is made of the operators the run is about to spend on, so its
  bias is at least *measurable* against the thing it predicts - which §6.3
  does, and finds `1.12x` to `1.59x`. That number is the price of the choice
  and it is the number any synthetic probe would have to beat; this round did
  not build one to race it, and says so in §13.
* **The probe is charged to the plan.** `work_base` is read *before* phase 0,
  so `work_units()` already contains the probe. `units` is a total, and the
  wall target is a promise about the whole process rather than about the part
  after the measurement.
* **The clock is read once.** Everything downstream is a work run, so the
  trajectory is a function of `units` and of nothing else. This is what makes
  the mode reproducible at all.
* **`units` is quantised**, because "a function of `units`" is only useful if
  two processes agree on `units`. §7.

Three constants, all overridable per run (`planbias`, `planhead`, `planq`), all
derived from measurement below, none of them a guess.

## 6. What had to be measured before the mode could be built

`drivers/calibrate.py`, `evidence/calibrate-base-40M.json`. mixed-61, three
seeds, seven rounds, at a **pinned** 40 M work budget on the base binary - so
this measures the box and the request, not the mode.

### 6.1 The only non-deterministic input is one clock reading

| seed | phase-0 work units, distinct values over 7 runs | phase-0 seconds, median | relative spread |
|---|---|---:|---:|
| 0 | **one**: 8,778,573 | 2.2046 s | 2.23% |
| 1 | **one**: 9,629,453 | 2.2872 s | 1.25% |
| 2 | **one**: 8,961,342 | 2.2861 s | 2.53% |

`probe_work_units` is a **counter**, and it is bit-identical across every run of
a cell. So the entire run-to-run variation of a phase-0-calibrated plan is the
variation in `probe_seconds`, and its size - 1.2 to 2.5% - is the number §7's
ladder has to be coarser than. Nothing else about the plan can move.

Depth over those same 21 runs is one distinct value per seed - 169.891 /
171.362 / 170.155 - which is the work budget's own determinism, unchanged.

### 6.2 The headroom: what a *pinned* plan still cannot control

Process wall at the same pinned 40 M plan:

| | n | p50 | max | min | spread, `(max-min)/p50` |
|---|---:|---:|---:|---:|---:|
| seed 0 | 7 | 12.790 s | 12.839 s | 12.715 s | **0.97%** |
| seed 1 | 7 | 12.301 s | 12.347 s | 12.250 s | **0.78%** |
| seed 2 | 7 | 14.291 s | 14.351 s | 14.191 s | **1.12%** |
| pooled | 21 | 12.790 s | 14.351 s | 12.250 s | p95 = 14.341 s |

The pooled p50/p95 ratio is 0.892, and **it is the wrong number to use**: it is
dominated by the difference *between* seeds (seed 2 genuinely takes 14.3 s where
seed 1 takes 12.3 s for the same work), not by run-to-run noise. The number the
headroom has to cover is the within-seed spread, which is **0.78-1.12%**.
`PLAN_HEADROOM = 0.97` covers it with room, and it can be that close to 1.0 only
because §7's ladder rounds **down** and is the larger margin by an order of
magnitude.

### 6.3 The bias: a probe is only an estimator for the work it resembles

`drivers/biascalib.py`, `evidence/biascalib-10s.json`. The plan's model is one
line, `wall = C + t0 + (T*h - t0) * b_true / b_ship`, and every term but
`b_true` is measured - so one run per cell solves it. Three fixtures x three
seeds x two rounds, `planq=1` so the ladder cannot pollute the fit.

Run twice, hours apart and on two binaries - the one the ladder step was chosen
*with* (`PLAN_QUANTUM_STEP = 1.25`) and the one every battery below ran on
(`1.15`) - so the fit itself has a reproducibility statement. Both arms pass
`planq=1`, so the step is inert in both and the only thing that differs is the
session:

| cell | fitted bias, first fit | fitted bias, battery binary |
|---|---:|---:|
| mixed-61 s0 | **1.118** | **1.205** |
| mixed-61 s1 | 1.296 | 1.300 |
| mixed-61 s2 | **1.586** | **1.587** |
| shapes-17 s0 | 1.527 | 1.520 |
| shapes-17 s1 | 1.478 | 1.424 |
| shapes-17 s2 | 1.423 | 1.472 |
| triangle-20 s0 | **1.192** | **1.313** |
| triangle-20 s1 | 1.460 | 1.459 |
| triangle-20 s2 | 1.373 | 1.370 |
| **over 18 runs** | 1.116 / **1.449** / 1.586 | 1.117 / **1.442** / 1.589 |

(min / median / max on the last row.) Phase 0 retires work units **1.12x to
1.59x faster** than the queue that follows it, on every one of the nine cells and
in both fits. The sign never flips, which is what makes a correction possible at
all; the **1.42x range** is what makes a single constant inadequate, and §9
measures what that costs. The two fits agree on the median to 0.007 and on the
maximum to 0.003, so the range is a property of the cells and not of the session.

The shipped `PLAN_PHASE_ZERO_BIAS = 1.70` is above every fitted value in both
fits, and it is **self-consistent**: `1.70` is the constant those runs were
themselves measured with, so the fit is not an extrapolation from a
differently-configured binary. All eighteen of the first fit's runs landed under
the ten-second target, the worst at 9.233 s.

`evidence/biascalib-10s.json`, `evidence/biascalib-10s-shipping.json`.

The process overhead outside the coordinator - request load and result
serialisation - is 28-36 ms, median 32 ms in both fits, and is inside the
headroom.

## 7. The quantisation ladder, which is the round's central trade

`probe_seconds` is a clock reading. Two processes will not agree on it, so they
will not agree on `raw_units`, so they will not agree on the document - unless
the plan is snapped to a grid coarse enough that the disagreement falls inside
one cell of it. That is the whole trade, and it has two ends:

* a **fine** ladder tracks the wall target closely and disagrees between
  processes whenever a cell's estimate straddles a boundary;
* a **coarse** ladder agrees between processes and throws away up to
  `1 - 1/step` of the budget.

Evaluated against the nine cells of §6.3, with each cell's observed band
**doubled** first so the pilot is not read as tighter than two samples support:

| step | rung width | cells straddling a boundary | floor loss, median | floor loss, worst |
|---|---:|---:|---:|---:|
| 1.05 | 4.9% | **6 of 9** | 4.0% | 4.5% |
| **1.15** | **14.0%** | **0 of 9** | **7.5%** | **11.0%** |
| 1.25 | 22.3% | 1 of 9 | 7.4% | 18.7% |
| 1.40 | 33.6% | 1 of 9 | 15.6% | 26.1% |

`PLAN_QUANTUM_STEP = 1.15` is the smallest step at which no cell in the pilot
straddles, and its rung is about fourteen times the median measured spread and
five times the worst. It **floors** rather than rounds, so the error is
one-sided and a plan is never larger than the probe justified - which is what
lets the headroom be 0.97 instead of 0.8. Note the shape of the table: 1.25 is
not better than 1.15 on either axis, so this is not a straight trade at every
step, and 1.40 buys nothing the coarser rung did not already have.

**All nine pilot cells are at a ten-second target.** §10.3 is what happens at
three and at thirty, and it is not a surprise: it is this table read at budgets
it does not cover.

`planq=1` switches quantisation off. That arm is run in full below, because the
honest form of this round's claim is a trade and not a win.

## 8. The battery: twenty rounds, three arms, one window

`drivers/planbattery.py`, `evidence/battery-10s.json`, `evidence/battery-10s.log`.
mixed-61, three seeds, **twenty rounds**, three arms interleaved with arm order
rotated by round so no arm always runs first into a cold cache. 180 runs, one
binary, one window.

| arm | spec | what it is |
|---|---|---|
| `plan` | `plan=10000` | the shipping mode |
| `planraw` | `plan=10000,planq=1` | the same, quantisation off |
| `wall` | `wall=10000` | the incumbent, and every previous chapter's ten-second number |

| arm | n | wall p50 | wall p95 | wall max | **runs over the 10 s target** |
|---|---:|---:|---:|---:|---:|
| `plan` | 60 | 7.156 s | 8.282 s | 8.306 s | **0 of 60** |
| `planraw` | 60 | 8.000 s | 9.243 s | 9.278 s | **0 of 60** |
| `wall` | 60 | 9.931 s | 10.335 s | 10.382 s | **21 of 60** |

| arm | seed | distinct plans | distinct depths | distinct documents | depth |
|---|---:|---:|---:|---:|---:|
| `plan` | 0 | **1** | **1** | **1** | **175.3878** |
| `plan` | 1 | **1** | **1** | **1** | **174.1700** |
| `plan` | 2 | **1** | **1** | **1** | **176.1620** |
| `planraw` | 0 | 20 | 2 | 20 | 175.1357 / 175.3878 |
| `planraw` | 1 | 20 | 1 | 20 | 171.3620 |
| `planraw` | 2 | 20 | 1 | 20 | 174.8810 |
| `wall` | 0 | n/a | **3** | 20 | 168.4836 / 169.5878 / 171.1110 |
| `wall` | 1 | n/a | **2** | 20 | 165.6558 / 165.8230 |
| `wall` | 2 | n/a | 1 | 20 | 174.2800 |

### 8.1 The wall budget's ten-second answer is three answers, and a fourth next door

**Twenty runs of one command on one seed produced three different depths.**
mixed-61, seed 0, `wall=10000`, the shipping arm, one binary, one afternoon:
`168.4836` thirteen times, `169.5878` six times, `171.1110` once. The spread is
**2.627 mm**.

Two of those three are numbers this campaign has published as *separate
results*:

* **168.484** is `docs/experiments/rotation-tax/` §4.2's ten-second figure for
  this exact configuration, quoted again by `fast-contract-validator` §12.3;
* **171.111** is `fast-contract-validator` §12.1's **loaded-box** figure for the
  same cell, which that round attributed to the box being busier and used to
  retract a verdict.

They are one seed of one battery apart. A first twenty-round battery on the same
tree (`evidence/battery-10s-first-binary.json`, §12.1) found a **fourth** value,
`169.379`, and put `171.111` at four of twenty rather than one - so the
distribution is not even stable in shape between two windows of the same
afternoon.

`sparse-rotation` §7.2 called this out as a session-to-session effect of "the
box, the day, and the seed set" and carried it forward as a caveat. It is
narrower and worse than that: **it is inside a single window, on a single seed,
with nothing changed at all.**

And the arm does not honour its own budget either. `wall=10000` **overran ten
seconds on 21 of 60 runs**, reaching 10.382 s - seed 1 overran on 19 of 20 -
because the deadline is checked between actions and an action in flight finishes.

### 8.2 The plan reproduces, sixty runs out of sixty — **on a quiet box**

> **Qualified, 2026-08-21, by `docs/experiments/replan/` §11.1.** The 60/60
> below is a **quiet-box** property and this section did not test it any other
> way. Re-measured on a box carrying a competing workload, the **same
> `plan=10000` arm** produced **2 / 3 / 1 distinct depths per seed**
> ([`replan/README.md:85`](../replan/README.md)), and
> `docs/experiments/robust-plan/` §9 re-measured it again at box load median
> 13.9 as **3 / 2 / 2 distinct depths and 3 / 2 / 3 distinct documents**, with
> seed 1 splitting ten runs at 177.9079 against ten at 174.1700 - a 3.738 mm
> coin toss.
>
> **The mechanism below is not retracted.** Probe → ladder → work cap does what
> this section says it does; what is conditional on the box is the sentence
> *"a second process gets the same one"*. The fix is `plancal=<path>`, a
> persisted calibration keyed on `probe_work_units`, **which is a counter** -
> and its own 60/60 *is* measured under load
> ([`robust-plan/README.md:41`](../robust-plan/README.md)). A reader quoting
> this section for a determinism claim should quote that one instead.

Every `plan` cell chose **one** plan (`24,891,457` units, ladder rung 23, all
three seeds), produced **one** depth, and produced **one** whole-document digest
over twenty runs. `allSeedsPlanStable` and `allSeedsDocumentStable` are both
true. **Nothing overran**: p95 8.282 s, max 8.306 s against a 10 s target.

That is the deliverable, and it is worth being precise about what it says. It is
not "the plan is close to ten seconds"; it is *"the number this configuration
produces at a ten-second target is a **number**, per seed, and a second process
gets the same one"* - which is the property §8.1 shows the incumbent does not
have, and which the banner above bounds to the box this ran on.

### 8.3 Without the ladder: twenty plans, twenty documents, and a depth that mostly does not care

The `planraw` arm chose **twenty distinct plans and produced twenty distinct
documents** on every seed, exactly as designed: `probe_seconds` is a clock
reading and no two processes get the same one.

But its **depth** split on only one seed of three - seed 0, at
`175.1357` twelve times and `175.3878` eight - and was a single value on the
other two. This is worth stating plainly because it bounds the ladder's value:
a plan that moves by ~1% usually lands in the same place, because depth only
moves when the budget crosses an action boundary. What the ladder buys is not
mostly-the-same depth, which `planraw` already has; it is **the same document**,
which is the only form of the claim a gate can check.

## 9. What the mode costs, decomposed

The plan mode is worse than the wall mode at the same nominal target, and the
whole of the difference is **wall it did not spend**. Per seed, medians:

| seed | `plan` | `planraw` | `wall` | fitted bias (§6.3) |
|---|---|---|---|---:|
| 0 | 175.3878 @ 7.156 s | 175.1357 @ 7.897 s | **168.4836** @ 9.931 s | 1.118 |
| 1 | 174.1700 @ 6.377 s | 171.3620 @ 7.993 s | **165.6558** @ 10.313 s | 1.296 |
| 2 | 176.1620 @ 8.247 s | 174.8810 @ 9.228 s | **174.2800** @ 9.700 s | 1.586 |
| **median of seed medians** | **175.388** | **174.881** | **168.484** | |

Three costs, and they are very different sizes.

**1. A conservative bias constant - the largest.** Read the `planraw` column,
which has no ladder in it: the three seeds spend **7.897 / 7.993 / 9.228 s** of
their ten, and that order is exactly the order of their fitted biases
(1.118 / 1.296 / 1.586). The seed whose true bias is closest to the shipped 1.70
spends 92% of its target and gives up **0.601 mm**; the seed furthest from it
spends 79% and gives up **6.652 mm**. One constant cannot fit a 1.42x range, and
this is what that costs.

**2. The quantisation floor - a median 1.281 mm.** `plan` against `planraw`, same
seed, same window: `+0.252` / `+2.808` / `+1.281` mm, for 0.742 / 1.616 / 0.982 s
of budget the floor gave back. It buys the one property the mode exists for, and
it is the only one of the three that is a dial rather than a limitation.

Note that the floor's cost is *not* ordered like the bias's: seed 1's raw plan
sat highest above its rung, so it lost the most to the floor and ended with the
shortest wall of the three despite a middling bias. Both effects are visible
because the two arms are separated; a battery that ran only `plan` would have
seen one number and attributed it to whichever cause it preferred.

**3. The work counters - unavoidable in this design.** A work budget is a
function of `profiling::counter_totals()`, so the plan mode carries the counters
through the whole of the wall it was given; `search::portfolio`'s own header
prices them at ~17% of throughput. That is a throughput number and this round
needs a depth number, so it was measured - same binary, same `wall=10000`
budget, counters forced on and off through `POLYGON_NESTING_PROFILE`, three
seeds x three rounds, paired and interleaved: `drivers/countertax.py`,
`evidence/countertax.json`.

| seed | counters off | counters on | delta |
|---|---:|---:|---:|
| 0 | 169.5878 | 172.2875 | **+2.700 mm** |
| 1 | 165.6558 | 167.1830 | **+1.527 mm** |
| 2 | 174.2800 | 176.1620 | **+1.882 mm** |
| | | **median** | **+1.882 mm** |

**The work counters cost 1.882 mm of depth at a ten-second wall on mixed-61**,
and that is a floor under the plan mode: any work-denominated budget pays it,
because a work budget is a reading of the counters. It is the price of
denominating a budget in something a clock cannot perturb, and ~~there is no
version of this mode that avoids it~~.

> ## Corrected, 2026-08-22 — the number is right, the last clause is wrong
>
> `docs/experiments/consolidation/` re-ran this exact battery on the same
> fixture at the same budget and **reproduced the 1.882 mm to four decimals**,
> then split it with a third arm the instrument of the day could not express.
> The split is not close:
>
> | | seed 0 | seed 1 | seed 2 | median |
> |---|---:|---:|---:|---:|
> | whole tax (`countersOn` − `countersOff`) | +1.177 | +10.400 | +1.882 | **+1.882** |
> | **the counting** (`meterOnly` − `countersOff`) | +0.000 | +0.000 | +0.000 | **+0.000** |
> | **the timing** (`countersOn` − `meterOnly`) | +1.177 | +10.400 | +1.882 | **+1.882** |
>
> `meterOnly`'s median is identical to `countersOff`'s **to every digit on all
> three seeds**. Stated precisely, because a wall-budget arm has run-to-run
> spread and these two do: the *medians* coincide exactly on 3 of 3, while the
> per-run distributions overlap rather than collapse (seed 0's `meterOnly` runs
> were 169.572 and 171.111). The claim the table supports is that the counting's
> median contribution is **+0.000 mm on every seed**, not that two runs are the
> same run. The tax this section priced is the *spans*, and one flag armed both,
> so no arm here could have separated them.
>
> `profiling::metering_enabled` separates them and
> `PortfolioSettings::lane_local_debit` is the setting that takes the counting
> without the timing. Measured in the currency it is actually paid in - same
> `work=` budget, both arms, **documents identical field for field on 9 of 9
> cells** - the debit retires the same work in **84.9%** of the seconds at
> 24.9 M units and **82.5%** at 120 M, which is `search::portfolio`'s own
> "~17% they cost" header, confirmed.
>
> End to end at a calibrated ten-second plan: **the same depth, the same
> document, and p95 8.89 s → 7.40 s**. So the mode does have a version that
> avoids it, and §13.1's ordering is unchanged - the bias constant is still the
> largest of the three costs.

**The three, on the medians, on mixed-61 at ten seconds:**

| cost | mm | measured by |
|---|---:|---|
| the bias constant | **3.741** | the remainder |
| the work counters | **1.882** | `drivers/countertax.py`, directly |
| the ladder floor | **1.281** | `plan` against `planraw`, §9 above |
| **total** | **6.904** | `plan` against `wall`, §10.1 |

They arrive at the total, but that is arithmetic rather than a proof of
independence: only two of the three were measured directly and the bias is what
is left over. What the table is for is the **ordering** - the bias is roughly
twice either of the others - and §13.1 names the fix for that one.

---

# Part III — the re-baseline

## 10. The canonical production table

**This is the campaign's canonical production number.** Three fixtures, three
budgets, three seeds, **two processes per cell**, the armed defaults, one
binary, one window. `drivers/anytime.py`, `evidence/anytime.json`.

| fixture | target | arm | seed medians (mm) | median | wall max | **cells a second process reproduced** | over target |
|---|---:|---|---|---:|---:|---:|---:|
| mixed-61 | 3 s | `plan` | 181.589 / 179.690 / 179.662 | **179.690** | 2.33 s | **3/3** | 0/3 |
| mixed-61 | 3 s | `wall` | 179.587 / 179.633 / 179.006 | **179.587** | 2.64 s | 0/3 | 0/3 |
| mixed-61 | 10 s | `plan` | 175.388 / 174.170 / 176.162 | **175.388** | 8.31 s | **3/3** | 0/3 |
| mixed-61 | 10 s | `wall` | 168.484 / 165.656 / 174.280 | **168.484** | 10.31 s | 0/3 | 1/3 |
| mixed-61 | 30 s | `plan` | 164.188 / 167.666 / 164.171 | **164.188** | 36.39 s | 2/3 | 1/3 |
| mixed-61 | 30 s | `wall` | 165.262 / 160.010 / 166.666 | **165.262** | 41.23 s | 0/3 | 1/3 |
| shapes-17 | 3 s | `plan` | 200.349 / 200.349 / 200.349 | **200.349** | 3.47 s | 2/3 | 2/3 |
| shapes-17 | 3 s | `wall` | 200.349 / 200.349 / 200.349 | **200.349** | 1.90 s | 0/3 | 0/3 |
| shapes-17 | 10 s | `plan` | 200.349 / 200.349 / 200.349 | **200.349** | 8.40 s | **3/3** | 0/3 |
| shapes-17 | 10 s | `wall` | 200.349 / 200.349 / 200.349 | **200.349** | 9.78 s | 0/3 | 0/3 |
| shapes-17 | 30 s | `plan` | 200.349 / 200.349 / 200.349 | **200.349** | 18.28 s | **3/3** | 0/3 |
| shapes-17 | 30 s | `wall` | 200.349 / 200.349 / 200.349 | **200.349** | 17.76 s | 0/3 | 0/3 |
| triangle-20 | 3 s | `plan` | 70.771 / 70.747 / 70.747 | **70.747** | 2.16 s | **3/3** | 0/3 |
| triangle-20 | 3 s | `wall` | 70.771 / 70.747 / 70.743 | **70.747** | 3.21 s | 0/3 | 2/3 |
| triangle-20 | 10 s | `plan` | 70.742 / 70.746 / 70.742 | **70.742** | 7.86 s | **3/3** | 0/3 |
| triangle-20 | 10 s | `wall` | 70.730 / 70.730 / 70.729 | **70.730** | 9.99 s | 0/3 | 0/3 |
| triangle-20 | 30 s | `plan` | 70.730 / 70.730 / 70.729 | **70.730** | 18.96 s | **3/3** | 0/3 |
| triangle-20 | 30 s | `wall` | 70.727 / 70.727 / 70.727 | **70.727** | 29.17 s | 0/3 | 0/3 |

**`plan` reproduced 25 of 27 cells. `wall` reproduced 0 of 27.** That is the
table's first claim and it is not close.

### 10.1 The price of reproducibility, in millimetres

| fixture | target | `plan` | `wall` | plan − wall |
|---|---:|---:|---:|---:|
| mixed-61 | 3 s | 179.690 | 179.587 | **+0.103** |
| mixed-61 | 10 s | 175.388 | 168.484 | **+6.904** |
| mixed-61 | 30 s | 164.188 | 165.262 | **−1.074** |
| shapes-17 | 3 s | 200.349 | 200.349 | **+0.000** |
| shapes-17 | 10 s | 200.349 | 200.349 | **+0.000** |
| shapes-17 | 30 s | 200.349 | 200.349 | **+0.000** |
| triangle-20 | 3 s | 70.747 | 70.747 | **+0.000** |
| triangle-20 | 10 s | 70.742 | 70.730 | **+0.012** |
| triangle-20 | 30 s | 70.730 | 70.727 | **+0.003** |
| | | | **median** | **+0.000** |

**Over nine (fixture, budget) rows the median price of reproducibility is zero,
and the whole of the cost is one fixture at one budget.** Seven rows are at
parity to within 0.012 mm; one row is 6.904 mm worse; one row is 1.074 mm
*better*.

The reason the 6.904 mm sits where it does is §9: mixed-61 at ten seconds is the
only cell in the table where the run has enough budget for the bias error to
matter and not so much that the fixture has saturated. shapes-17 saturates at
200.349 mm at three seconds and never moves again, in either arm - which is
`fast-contract-validator` §10.3's finding from the other side, and it is why
that fixture contributes nothing to any comparison at any budget. triangle-20
moves by hundredths.

### 10.2 Where the wall promise holds, and where it does not

Neither arm bounds its wall at every budget, and the honest statement is per
budget:

| target | `plan` cells over target | `wall` cells over target | longest `plan` wall | longest `wall` wall |
|---|---:|---:|---:|---:|
| 3 s | 2 of 9 | 2 of 9 | 3.47 s (+16%) | 3.21 s (+7%) |
| **10 s** | **0 of 9** | 1 of 9 | 8.40 s (**−16%**) | 10.31 s (+3%) |
| 30 s | 1 of 9 | 1 of 9 | 36.39 s (+21%) | 41.23 s (+37%) |

* **At ten seconds - the budget the user priority names and the budget the
  constants were fitted at - the plan mode overruns nothing on any fixture, and
  the wall mode does.**
* **At three seconds the limit is action granularity, not calibration.** Neither
  mode can stop an action in flight, and at three seconds one action is a large
  fraction of the budget: `plan` overruns on shapes-17 (3.47 s) and `wall` on
  triangle-20 (3.21 s). No constant fixes that; a preemptible operator would.
* **At thirty seconds the limit is the bias constant**, exactly where §13
  predicts it: the fitted bias rises with the budget, so a constant fitted at
  ten seconds is no longer conservative at thirty, and mixed-61 seed 2 runs
  36.39 s. The `wall` arm is worse in the same cell - **41.23 s against a 30 s
  budget** - so this is a shared failure that the plan mode reduces rather than
  one it introduces.

### 10.3 The two cells the plan mode failed to reproduce, and why

Both are **exactly one ladder rung**, which is the failure mode §7 predicts and
the only one it predicts:

| cell | process A | process B | ratio | depth |
|---|---:|---:|---:|---|
| shapes-17, 3 s, seed 2 | 2,313,060 | 2,660,019 | **1.150** | identical, 200.349 both |
| mixed-61, 30 s, seed 1 | 66,211,771 | 76,143,537 | **1.150** | 167.666 / 165.935 |

The first cost nothing at all - the two plans produced the same layout and
differ only as documents. The second cost 1.73 mm.

**And both are at budgets §7's pilot never covered.** That pilot found 0 of 9
cells straddling at a step of 1.15, and all nine of its cells were at a
ten-second target; every ten-second cell in this table reproduced, 9 of 9, as
did all 60 runs of §8's battery and all 9 cells of §12.3's determinism gate.
The two failures are at three seconds and at thirty. So the honest statement is
narrower than "1.15 straddles 7% of the time": **the step was chosen against
ten-second cells and it holds on ten-second cells; it was not fitted at three or
thirty seconds and it does not hold there.** A ladder step is a per-budget
choice and this round only earned one of them - and §7's table is the price
list for the others: a coarser step would buy the missing budgets at a cost in
unspent wall, a finer one would lose ten seconds too.


## 11. Against Sparrow, and what the comparison is now worth

Sparrow on this same x86_64 box, seed 0, 8 workers, from
`docs/experiments/sparrow-mixed61/` §"x86_64 same-machine addendum" - 157.971 mm
at three seconds and 150.165 mm at ten, both exact-valid. Against §10, on the
same fixture and the same box:

| budget | Sparrow | this round, `wall` | this round, `plan` | gap, best arm |
|---|---:|---:|---:|---:|
| 3 s | 157.971 | 179.587 | 179.690 | **21.6 mm** |
| 10 s | 150.165 | 168.484 | 175.388 | **18.3 mm** |
| 30 s | not published | 165.262 | 164.188 | - |

**The gap is not moved by this round and this round does not claim to move
it.** At ten seconds it is 18.3 mm against the `wall` arm, which is
`fast-contract-validator` §13.3's 18.6 mm reproduced to within the spread
§8.1 measures, and 25.2 mm against the reproducible `plan` arm. This round
does not re-adjudicate that comparison; it is the one
`fast-contract-validator` §13.3 makes when it puts its best configuration at
168.756 mm against 150.165 mm and calls the gap 18.6 mm. What this round changes
is not the gap but **what the left-hand number means**.

Before: "168.484 mm at ten seconds" was one draw from a distribution the
campaign had never characterised, and §8.1 shows that distribution spanning
2.6 mm on one seed in one window - so a 0.3 mm improvement reported against it
was, at 20 runs, indistinguishable from re-running the same command.

After: the plan mode's ten-second number is a **single value per seed that a
second process reproduces**, and the wall arm's is a distribution whose shape is
now published. A future round claiming a millimetre against either has something
to claim it against.

## 12. Determinism, equivalence and the suites

### 12.1 The binaries, and the one thing that is not tidy about them

`evidence/binaries.txt` carries every content hash. Six binaries: a gate build
and a measurement build of the campaign base commit `a131a72`, of this worktree
as the batteries ran it, and of this worktree as it is committed.

**The batteries and the committed tree are a few comment-only edits apart**, and
this document says so rather than rounding it away. After the batteries started,
three doc comments were extended - on `PLAN_ANCHOR_UNITS`, on
`parse_portfolio_spec` and on the plan's `profiling::set_enabled` site - and one
`assert!` was added to `examples/contract_validator_shadow.rs`, which is a
different example binary. None of them is executable code in the measurement
path, but comments shift line numbers, and rustc embeds line numbers in panic
locations, so the binary's hash moves. The equivalence is therefore **measured**
rather than argued: `evidence/shipping-binary-reproduces.json` runs the shipping
measurement binary through the plan mode on all three seeds and requires the
same plan, the same depth and the same whole-document digest as the battery
binary produced.

The build is reproducible: rebuilding the same tree into a fresh
`CARGO_TARGET_DIR` gives a byte-identical binary, which is what makes the hash
worth recording at all - and is how the comment-only shift above was identified
as a comment-only shift rather than assumed to be one.

| binary | features | sha256 |
|---|---|---|
| `base-gate` | `jagua-experimental` | `511afd201eb1dcac4afb0af97d75c3151165f8e2e5735a17b83bb4ce68d32c6a` |
| `base-meas` | full combo | `4b791db4f1a59a092e70aedf5afee4264e5d7b988b8ab70d3ddfe89e885db891` |
| `plan-gate` | `jagua-experimental` | `4449326979f1ba48b3bc0b1556fb9e1542eeedf1631fea27179ce5457770d47a` |
| `plan-meas` (every battery) | full combo | `1182b02ba5d78cbd23e87273bacf4feb38816f4d7d52cb2944ea2f1ed20fb24b` |
| `ship-gate` (committed tree) | `jagua-experimental` | `a2ad9bad87cc3325ae326a71c0d6e0f9baa5f085063571dee7c9a9098fbb9488` |
| `ship-meas` (committed tree) | full combo | `5681046a61fc665e0448eec75c68cd163849792b084759d58410b25ecd3f7cc0` |

Full combo is
`jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator`.
Base is the campaign base commit `a131a72`.

**The four pinned gates were run on three of these** - `base-gate`, `plan-gate`
and `ship-gate` - and all four gates hit on all three with the **same four
whole-document digests**: `012a8eb1b24c3090`, `28b58a25e68fc205`,
`11afd25f7989e283`, `f80c2a17412141ef`.

**And the equivalence, measured**: `ship-meas` run through the plan mode on all
three seeds returns the battery binary's plan, depth and document digest,
identically:

| seed | plan units | depth | document |
|---|---:|---:|---|
| 0 | 24,891,457 | 175.3877782649107 | `8f2949e8bd9e5b8c` |
| 1 | 24,891,457 | 174.17000000000002 | `c2c07a40c5ffdfa2` |
| 2 | 24,891,457 | 176.16200000000003 | `b7a7fafdcec25b10` |

`evidence/shipping-binary-reproduces.json`, `evidence/gates-ship-gate.json`.

### 12.2 Determinism across two processes, work mode

`drivers/determinism.py`, three requests x three seeds, `work=30000000`, whole
documents. **9 of 9 identical, `allEqual: true`.**
`evidence/determinism-work.json`. This is the campaign's standing hard gate and
the plan mode does not change it.

### 12.3 Determinism across two processes, plan mode

The same shape, `plan=10000`, and the claim is **two** claims because the plan
reads the clock once: the two processes must agree on `portfolio.plan.units`,
and *given that*, the documents must be identical with `planCalibration`
stripped. A cell counts as equal only if both hold - a driver that checked only
the second would pass vacuously on two runs that chose different plans and then
agreed about nothing else.

**9 of 9 on both halves: nine cells agreed on the plan, and nine cells produced
the same document.** `evidence/determinism-plan.json`.

> **Quiet-box, like §8.2, and for the same reason**: agreeing on the plan is
> agreeing on a clock reading's ladder rung. `docs/experiments/robust-plan/`
> §12 re-runs this under load and gets **8 of 9** on both the plan and the
> calibrated arms. The work-mode gate in §12.2 is *not* conditional - a work
> budget reads no clock - and it is the one to quote when the claim has to hold
> on an arbitrary box.

Note what this does and does not add to §10.3, where two of 27 cells straddled.
These nine cells are three requests x three seeds at **ten seconds**; §10.3's
two failures are at three and at thirty. Nine clean cells at the calibrated
budget and two straddles at the uncalibrated ones is one consistent picture, not
two, and §7 is where the dial that governs both sits.

### 12.4 Suites

`drivers/run-suites.sh`, exit status captured directly rather than through a
pipe, because `cargo test ... | tee log` reports `tee`'s status and that is how
a red suite gets written up as green.

| suite | features | exit | tests |
|---|---|---:|---|
| `suite-jagua` | `jagua-experimental` | **0** | 1267 passed, 0 failed |
| `suite-combo` | the protocol's full combo | **0** | 1315 passed, 0 failed |

`EXITS jagua=0 combo=0`. Both passed on the first attempt, including the
campaign's known flake
(`free_material_multi_eviction_shrinks_retained_container_capacity`), which did
not need a rerun. Logs: `evidence/suite-jagua.log`, `evidence/suite-combo.log`.

The 48-test difference between the two is the feature-gated tests the combo
compiles and the gate build does not - including this round's own
`the_certificate_arming_is_restored_on_the_way_out`, which exists only under
`fast-contract-validator`.

## 13. Honest caveats, and what would fix the largest one

* **One box, and it is the same box every previous round used.** Every second
  in this document comes from one shared x86_64 machine with `actualThreads: 8`.
  The bias, the headroom and the ladder step are all fitted to it. On another
  target the *shape* of the argument survives - phase 0 is faster than the queue,
  a clock reading has a spread, a ladder trades accuracy for agreement - and
  none of the three constants does. `planbias`, `planhead` and `planq` are spec
  keys for exactly that reason, and `drivers/biascalib.py` is the driver that
  refits them.
* **Three seeds, and nine cells, and one budget.** The ladder step was chosen
  against a nine-cell pilot with each cell's band doubled, all nine at a
  ten-second target. Nine cells cannot measure a straddle probability of a few
  percent; what they can do is reject the steps that straddle obviously, and
  that is what the table in §7 is. §10.3 is what the pilot did not cover
  arriving.
* **The bias is fitted at a ten-second target and it grows with the budget.**
  Measured: mixed-61 seed 0 fits 1.118 at a ten-second target and 1.40 at a
  pinned 40 M plan (§6.1's battery), because the queue's late actions cost more
  per unit than its early ones. A thirty-second target therefore sits further
  from the shipped constant than a ten-second one, and the 30 s row of §10 is
  the one to read with that in mind.
* **The plan mode does not make a *number* portable, only reproducible.** Two
  boxes will calibrate to two different plans and produce two different depths.
  What it removes is the *session* variance on one box, which is what §8.1
  shows is currently 2.6 mm and was being quoted to three decimals.
* **`fcv=0` and `m34pconfirm=0` were measured as documents and as
  milliseconds, not as depth at a wall budget.** The depth claim for those two
  levers is `fast-contract-validator` §12's and this round does not re-make it;
  what this round adds is that at a *work* budget they are provably free, which
  is the statement §12 could not make.
* **The plan mode is not wired into any production route.** `plan=<ms>` is a
  spec key on the benchmark example, and the coordinator that reads it is still
  `coordinator_v3`, which is still off by default. Nothing in a shipping build
  reaches this code. Deciding whether the napi and CLI surfaces should expose a
  wall target is a product question this round does not answer.
* **`PLAN_ANCHOR_UNITS` is a lattice and lattices can be gamed.** The rungs are
  `1e6 * 1.15^k` and nothing stops a future round from moving the anchor until
  a favourable fixture lands mid-rung. It is a round number chosen before any
  cell was measured, and it stays that way, but the reader should know the
  degree of freedom exists.

### 13.1 The largest cost has a name and a fix

Of §9's three costs, only the first - the unspent wall a conservative bias
leaves - has an obvious fix, and it is **not a better constant**. One number
cannot fit a bias that ranges 1.42x across nine cells and rises with the budget;
what fixes that is a **second clock reading**. Install a provisional plan from
phase 0, run to a deterministic work checkpoint, then re-price the remaining wall at the rate the
*queue* is actually retiring units at - which is the quantity the bias exists to
guess. The estimator's bias then goes to ~1 by construction and the second
reading is over a longer window, so the ladder could also be finer.

It is not in this round because it is not free: `v3_loop`'s `run.deadline` and
`Coordinator::protected_fraction` are both fractions of the plan that was
installed when the phase was entered, so a mid-run re-plan has to recompute
both, and those two lines are the ones every previous chapter's schedule
numbers rest on. Named, priced and left, rather than done badly.


## 14. Reproducing this

Every number above comes from a driver in `drivers/`, and every driver takes the
binary as an argument so a paired A/B can hold two of them side by side.
`drivers/runlib.py` and `drivers/gatelib.py` carry the pinned CLI tail, the
`0.002` search-offset allowance, the salt sets and the request table, and their
`ROOT` points at this worktree.

```
# the two binaries
cargo build --release --example general_request_benchmark \
  --features jagua-experimental
cargo build --release --example general_request_benchmark --features \
  jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

python3 drivers/gates.py       ship OUT_GATE_BIN OUT                 # 2, 12.1
python3 drivers/armgate.py     OUT NEW_BIN BASE_BIN GATE_BIN \
                               mixed-61,triangle-20 0,1 30000000     # 3, 4
python3 drivers/calibrate.py   OUT BASE_BIN mixed-61 40000000 0,1,2 7 \
                               'm34lanes=1,m34pconfirm=1'            # 6.1, 6.2
python3 drivers/biascalib.py   OUT MEAS_BIN 10000 0,1,2 2            # 6.3
python3 drivers/planbattery.py OUT MEAS_BIN mixed-61 10000 0,1,2 20  # 8
python3 drivers/countertax.py  OUT MEAS_BIN mixed-61 10000 0,1,2 3   # 9
python3 drivers/anytime.py     OUT MEAS_BIN \
                               mixed-61,shapes-17,triangle-20 \
                               0,1,2 3000,10000,30000                # 10
python3 drivers/determinism.py OUT MEAS_BIN \
                               mixed-61,shapes-17,triangle-20 \
                               0,1,2 plan 10000                      # 12.3
bash    drivers/run-suites.sh                                        # 12.4
python3 drivers/summarize.py   battery OUT/planbattery.json          # the tables
```

`drivers/summarize.py` regenerates §8's, §10's and §12's tables straight out of
the JSON, because a table typed by hand from a JSON file is a table that can
disagree with it.

**A caller who wants the guarantee without the calibration** takes the `units`
the plan reports and replays it:

```
'plan=10000,cells=13:15:17:19,v3=1'     ->  portfolio.plan.units = 24891457
'work=24891457,cells=13:15:17:19,v3=1'  ->  the same document, always
```

Measured, in `evidence/smoke-plan-modes.json`, mixed-61 seed 0 on the shipping
binary - four specs, one run each:

| spec | wall | budget the run spent | depth |
|---|---:|---|---:|
| `plan=10000` | 7.115 s | `work` 24,891,457 | **175.3877782649107** |
| `work=24891457` | 7.183 s | `work` 24,891,457 | **175.3877782649107** |
| `plan=10000,planq=1` | 7.874 s | `work` 26,485,639 | 175.1357388323935 |
| `wall=10000` | 9.891 s | `wall` 10,000 ms | 168.4836008374388 |

The first two are the same run. The third is the same run with a plan the ladder
did not floor - a 6.4% larger budget for 0.252 mm - and the fourth is the
incumbent, whose 168.4836 is one of the three values §8.1 shows that command
producing.

That second line is the recommendation for anything that has to be
bit-reproducible across boxes, sessions and rounds: **calibrate once, pin the
plan, ship the number.** It is also, in one line, what
`docs/sol-review-5-se2-and-pose-freedom.md` §5 asked for - a fixed work plan,
with the wall envelope measured rather than assumed.
