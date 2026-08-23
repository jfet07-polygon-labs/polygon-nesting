# Wave 4, amended — the gate ran, and it says no on quality

**§0 was answered.** All six clauses have numbers behind them for the first time
in this round, and the verdict is a **FAIL on clauses (1) and (2)**: at a
ten-second work budget on bare mixed-61, **zero of nine seeds** reach 168.484 mm
on either arm, and the median is 179.076 mm. Clauses (3), (4) and (5) pass —
including the one the amendment fought over, `p95 ≤ 10.000 s`, at **9.527 s**.
Clause (6) is a **draw**: the impatient work-quanta policy is **not promoted**
and the frozen `200/3/100/5/0.98` remains the member.

The budget the gate spent is the **declared fallback**, because `U'` was
rejected by its own rule exactly as `U` was. The reject rule fired at
**291.50 % / 301.47 % / 383.68 %** over three runs, six of six ordered pairs
every time.

| | |
|---|---|
| base | `e4da8c5` · the currency amendment |
| §0 verbatim | `SECTION0_VERBATIM: true`, 13 lines, 779 bytes · `AMENDMENT_VERBATIM: true` · exit 0 |
| rider (i), the counter | `COUNTER_BIT_IDENTICAL: true`, exit 0, **before any coefficient was fitted** |
| rider (ii), collinearity | **did not fire** — `E`/`P` separable, two prices fitted |
| `U'` reject rule | **REJECTED, exit 1 × 3 runs**, 6/6 pairs, all three fixtures |
| budget | the declared fallback: *single-fixture work plan, no transfer claim* |
| §0 two-arm gate | **RUN.** `GATE_PASS: false` — clauses (1) and (2) fail |
| §0 clause (5) | **PASS**, p95 9.527 s (control) / 9.419 s (treatment) over 5 × 9 |
| attribution | **draw** — control stays member |

The rest of this file is the document as it was written **before the first gate
cell ran** — §0, the amendment, and every criterion — followed by what the
batteries measured. `section0.py` still re-extracts those thirteen lines from
`docs/economics-round-spec.md` by their heading and requires the block quote
below to match them exactly; the exit status is the verdict.

---

## §0 — the pre-committed gates

*Copied verbatim from [`../../../../economics-round-spec.md`](../../../../economics-round-spec.md)
§0, before this directory carried a single gate number.*

> ## §0 — the pre-committed gates
>
> 10 s calibrated-work, bare mixed-61, seeds 0..=8, workers=8, quiet box.
> PASS iff ALL: (1) ≥5/9 exact-valid ≤168.484 mm; (2) median ≤168.484;
> (3) every publication Exclusive r=2.500 + contract-valid; (4) per-seed
> two-process bit identity; (5) quiet-box p95 ≤10.000 s over 5×9;
> (6) attribution vs the control arm as above. 30 s: median ≤163.00461;
> ≥7/9 ≤168.484 (no-regression); paired ≥1.000 mm; shapes-17/triangle-20
> within 1 mm at equal work; zero invalid publications. 60 s reported, never
> gated. **150.165 is the horizon, not a clause.** No clause may require
> seeds 7/8 (the different-basin pair). Failure license: one named line-level
> defect with red/green vector → one identical rerun; a valid miss closes
> this funding.

### …as amended

[`docs/currency-amendment.md`](../../../../currency-amendment.md) is the only
thing that touches the block above, and it touches it in exactly one place — the
denomination of the budget, and what happens to the three clauses that depend on
it. Quoted verbatim, because a paraphrase of an amendment is an amendment:

> - Fallback if U' fails: the transferring pacer closes; the 10s two-arm
> mixed-61 gate runs on a mixed-61-only shelf-probed work budget labeled
> "single-fixture work plan, no transfer claim"; clauses (1)(2)(3)(4)(6)
> bind unchanged AND **clause (5) p95<=10.000s STILL BINDS as a claim** (the
> budget is not retuned after seeing p95); the 30s shapes-17/triangle-20
> equal-work clauses are inevaluable and recorded not-run.

So on the fallback path, and only there:

* clauses **(1) (2) (3) (4) (6)** bind unchanged;
* clause **(5)** binds as a *claim*: `p95 ≤ 10.000 s` over 5 × 9 is a pass/fail,
  and if it fails the gate **fails clause (5)** and this document says so;
* the 30 s **mixed-61** clauses (median ≤ 163.00461, ≥ 7/9 ≤ 168.484, paired
  ≥ 1.000 mm) bind unchanged;
* the 30 s **shapes-17 / triangle-20 within 1 mm at equal work** clause is
  **inevaluable** — a single-fixture work plan has no equal-work denomination on
  another fixture — and is recorded **NOT-RUN**, never as a pass.

---

## The pre-committed criteria of this wave

Everything a number could be compared against, written down while there are no
numbers to compare it against.

### 1. `U'`, and the rule that may reject it

The amended formula, verbatim:

> `U' = sample_evaluations + B·master_batches + E·exact_checkpoint_calls
> + P·published_bites + D·disruption_moves`

* `R` is **absent**. Not zero — absent. `WorkTermsPrime` has no repair field, so
  no arrangement of repair rows can move a `U'` reading.
* The derivation is the signed one: **timing-only**, on **all three fixtures**,
  **conservative rounding** (`ceil`, the one direction the spec licenses).
* The reject rule is the signed sentence, unchanged: **wall-prediction error
  > 10 % on any transfer reading rejects `U'`**. Three runs × six ordered pairs
  × three fixtures. **No charity, no reweighing, no dropping fixtures.** Every
  reading is recorded, including the ones that would be dropped by a kinder
  rule.

### 2. Rider (i): the counter comes before the coefficient

`published_bites` must be proven **bit-identical across two processes** on the
fixed-work cells **before** `P` is fitted. `counter.py` is that proof and it
runs first; if it is red, no coefficient is fitted at all.

### 3. Rider (ii): the collinearity criterion, chosen before it is applied

The `E` and `P` design vectors are reported **side by side**. They are
**collinear** — and one combined term is fitted — iff **both** of:

* the per-fixture ratio `E_f / P_f`, over fixtures where both are non-zero, has
  **max/min ≤ 1.05**; and
* the cosine of the angle between the two whole vectors is **≥ 0.9995**.

Two measures rather than one, because either alone has a shape it cannot see. The
bars are constants in `search::overlap_ics_meter::currency`
(`COLLINEARITY_RATIO_BAR`, `COLLINEARITY_COSINE_BAR`), so the document and the
decision cannot disagree.

### 4. The budget, and the two paths it can come from

* **If `U'` passes**: the calibrated-work plan is denominated in `U'`, spent
  through the `icscal` read path, with no live probe on any gated trajectory and
  a stop only between master batches.
* **If `U'` fails**: the declared fallback, and it is final (rider (iii): this
  is the last currency proposal this funding sees). A **mixed-61-only
  shelf-probed work budget**, labelled *"single-fixture work plan, no transfer
  claim"* in every document that carries a number from it. The budget is set
  from the shelf probe **before any gate cell runs** and is **not retuned after
  seeing `p95`**.

### 5. The battery, fixed here so that it cannot be chosen later

Bare mixed-61, seeds 0..=8, workers = 8, quiet box (`/proc/loadavg` one-minute
figure below 1.00 before every timed battery), `--revalidate=1` on every gate
cell.

| battery | arms | cells | what it answers |
|---|---|---|---|
| 10 s gate | control + treatment | 9 seeds × 2 processes | clauses (1) (2) (3) (4) |
| 10 s p95 | control + treatment | 9 seeds × 5 repetitions | clause (5) |
| 30 s | control + treatment | 9 seeds | the 30 s clauses |
| 60 s | control + treatment | 9 seeds | reported, never gated |
| 3 s | control + treatment | 9 seeds | the curve, never gated |
| AB/BA | old wall arm | interleaved | diagnostic only, never a lane |

*The two arms:* **control** is the frozen literals `200/3/100/5/0.98`;
**treatment** is the work-quanta impatient policy at the frozen KNOB
`1_630_000` / `815_000`. Strike semantics are the only delta between them.

*Attribution, clause (6):* treatment is promoted iff it gains **≥ 2 qualifying
seeds** or a **≥ 1.000 mm paired median** over control. Otherwise the absolute
5/9 is a draw, the impatient policy is **not** promoted, and the control's
policy remains the member.

### 6. What is frozen, and stays frozen

The executor stays unbuilt — wave 1's census closed that gate at 5.082 % against
a 10.000 % bar, and the 5/9 clause does **not** drop to 4/9. Every item the spec
froze is frozen. The only engine-side change this wave carries is in the
**meter**, which is measurement code that cannot reach a trajectory:
`search/overlap_ics/` is byte-for-byte unchanged since `e4da8c5`, and
`heavy.sh` records the `git diff` over that directory rather than asserting it.

---
---

# What the batteries measured

Everything above this line was written before the first gate cell ran.
Everything below it is a number.

## 1. Rider (i): the counter, before the coefficient

The amendment asks for `published_bites` as *an instrumented counter proven
bit-identical across runs before fitting `P`*. **One already existed**, so none
was written: `BiteRecord::published` is the trajectory's own publication record,
emitted per bite as `"published": <bool>` by every build the campaign has run,
at no clock cost, with no engine decision reading it, and inside the
whole-document two-process comparison every determinism claim already rests on.
Adding a second counter for the same fact would have made the first thing anyone
checks "do the two agree".

So `counter.py` proves the one that exists. Three fixtures; two processes of the
`ics-profile` build and one of the plain build; **the per-bite vector, not just
its sum**, because a scalar that matches can hide two bites swapping:

| fixture | bites | `publishedBites` | two processes | whole document | plain build | reconciles with `publicationCount` |
|---|---:|---:|---|---|---|---|
| mixed-61 | 26 | **24** | identical | identical | identical | yes |
| shapes-17 | 5 | **0** | identical | identical | identical | yes |
| triangle-20 | 34 | **34** | identical | identical | identical | yes |

`COUNTER_BIT_IDENTICAL: true`, exit 0. "Whole document" strips the census's own
`TIMING_KEYS` and nothing else — imported from `census/identity.py` rather than
re-listed, so the set of things that count as a clock cannot drift between two
files. `publicationCount` is built by appending to the publication list and
`sum(published)` is a fold over the bite records: two routes to one number, and
they agree on all three cells.

**This ran first.** Had it been red, no coefficient would have been fitted and
the wave would have stopped on rider (i) rather than on the reject rule.

## 2. `U'` — derived, and rejected by its own rule

### Rider (ii), reported before it was applied

| fixture | `E` = `exact_checkpoint_calls` | `P` = `published_bites` | `E/P` |
|---|---:|---:|---:|
| mixed-61 | 50 | 24 | 2.0833 |
| shapes-17 | 0 | 0 | — |
| triangle-20 | 34 | 34 | 1.0000 |

Ratio spread **2.0833** against the 1.05 bar; cosine **0.936264** against the
0.9995 bar. **Rider (ii) did not fire** — the two vectors are separable, so two
prices were fitted rather than one. Both bars are constants in
`search::overlap_ics_meter::currency` and were in the crate before these vectors
were measured. Grok's worry that "triangle-20 reads 34/34" was worth writing
down and is not what the whole matrix does: mixed-61 asks the exact authorities
twice per published bite and triangle-20 once.

### The coefficients, and the one that is not `R`

| term | run 1 | run 2 | run 3 | max/min |
|---|---:|---:|---:|---:|
| `B` master batch | 623 | 639 | 624 | 1.026× |
| `E` exact checkpoint call | 436 | 467 | 474 | 1.087× |
| **`P` published bite** | **51** | **57** | **54** | **1.118×** |
| `D` disruption move | 985 | 1,054 | 1,129 | 1.146× |

`R` moved **6.89×** between repetitions of its own calibration and was never a
price. `P` moves **1.118×** with support on two fixtures — it would pass the
rule the amendment wrote for restoring `R` in a future funding (*"spread ≤1.5x
across three runs AND support on ≥2 fixtures"*), which is reported in
`evidence/rejectgate2.json` as `pSpreadUnderFutureRRule` and **decides nothing
today**. The per-bite term is identified. It is simply small.

Two things the derivation says out loud rather than leaving to be discovered:
**`R` being absent does not delete the repair's wall** — it is inside `exactNs`
and is now charged to `E`, which is why `E` reads 436–474 where `U`'s read
340; and **`P` and `D` are a two-term fit, not two readings**, because no timer
exists around the cut, the pose install, the publication commit, the row rebuild
or the disruption. `calibrationPrime.residualSplit` prints that design matrix and
each fixture's miss.

### The rule, over all three fixtures, with nothing dropped

**Every reading is recorded** — three runs × six ordered pairs × three
currencies — in `evidence/rejectgate2.json` under `everyReadingPerRun`.

| currency | run 1 | run 2 | run 3 | worst pair | pairs over the bar |
|---|---:|---:|---:|---|---:|
| `U0-sample-evaluations` | 229.60 % | 232.72 % | 293.07 % | shapes-17 → triangle-20 | 6 of 6 |
| `U1-weighted-vector` | 253.73 % | 260.41 % | 333.82 % | mixed-61 → triangle-20 | 6 of 6 |
| **`U2-per-bite-vector` (`U'`)** | **291.50 %** | **301.47 %** | **383.68 %** | mixed-61 → triangle-20 | **6 of 6** |

`CURRENCY_PRIME_ACCEPTED: false`, exit 1, three times, over a design matrix
**bit-identical across all three runs** — `publishedBites` included, which is
rider (i) restated over the runs the coefficient was actually fitted from. The
loads at each run's start were **0.63 / 0.93 / 0.96**, and run 1 — the quietest
box — is the run with the *smallest* worst error, so a warm box did not
manufacture this rejection.

The matrix is also **the same one wave 4 rejected `U` on**: 6,977,140 / 514 / 50
/ 8 on mixed-61, 1,418,260 / 840 / 0 / 4 on shapes-17, 45,364 / 5 / 34 / 0 on
triangle-20, measured here by a differently built binary in a different
worktree. So seven independent readings of this clause now exist over one matrix
— wave 2b's, wave 4's three, and these three — and every one rejects. Only the
seconds have ever moved.

**Why `U'` is worse than `U0`, and why that is not a bug.** `U0` is the floor:
every coefficient zero, so every fixture's units are its sample evaluations
alone. On these cells triangle-20 already carries **too many** units for its
wall — 45,364 evaluations in 5.20 ms against mixed-61's 6,977,140 in 2.573 s —
so `mixed-61 → triangle-20` reads **186.82 / 188.61 / 247.06 %** at `U0`, before
a single coefficient exists. `U1` takes it to 253.73 / 260.41 / 333.82 % and
`U'` to 291.50 / 301.47 / 383.68 %: each currency that adds a term makes this
pair worse, monotonically, in every run. Every term `U'` adds is
non-negative, and `P`'s design
vector is proportionally **larger** on triangle-20 (34 of 34 bites) than on
mixed-61 (24 of 6.98 M evaluations). Adding it moves the ratio the wrong way. It
is arithmetic, not luck: **no non-negative per-bite term can make this pair
transfer**, and the amendment's own instruction — no charity, no reweighing, no
dropping fixtures — is what makes that the answer rather than an argument for
dropping triangle-20.

The heavy-pair reading `U`'s stop rested on is still computed and still printed,
labelled a **diagnostic** in the document itself, because the amended rule drops
no fixture and a reader who remembers wave 4's table is entitled to the
comparison. **It does not rescue `U'` either:**

| pair, `U'` | run 1 | run 2 | run 3 |
|---|---:|---:|---:|
| mixed-61 → shapes-17 | 13.65 % | 13.92 % | 15.34 % |
| shapes-17 → mixed-61 | 12.01 % | 12.22 % | 13.30 % |

Six readings, two directions, three runs — every one over the 10.000 % bar, as
wave 4's twelve were. There is no fixture set, no direction and no run in which
the amended currency transfers.

Rider (iii) binds: **this was the last currency proposal this funding sees.**

## 3. The budget: a single-fixture work plan, no transfer claim

Set **before any gate cell ran** and **not retuned after seeing `p95`**.

| | |
|---|---|
| label, in the plan's own `provenance` | *single-fixture work plan, no transfer claim* |
| explore rate | **2,740,976 units/s** — bite 22 (the 179 shelf) alone, 400 master iterations, the probe bite's own `sampleEvaluations` over the probe's own wall |
| compress rate | **1,464,184 units/s** — 8,072,852 units over 5.5135 s, measured on the **48 compress bites** of one 30 s wall calibration |
| safety factor | 0.80, the campaign's, applied to both |
| pinned constructor | **2.310938 s** |
| search budget at 10.000 s | **7.689062 s** |
| allocation | explore **13,488,342** units, compress **1,801,312** units |
| plan sha256 | `d83e22d1dcd4e9f2…` · the engine's own reader calls it a **hit**, and the sha it read back is the sha that was written |

The shelf's rate is **1.2261×** the blended explore rate the same wall
calibration wrote for its own explore phase (2,235,468 u/s over 120 bites). That
ratio is the whole of "shelf-probed": a plan built from the blended number would
have under-promised by 23 % and bought correspondingly less search. The blended
phase travels in `evidence/budget.json` as
`compressCalibration.blendedExplorePhase_NOT_USED`, so the road not taken is a
field rather than a claim.

**The frame, because it decides clause (5).** Every §0 clause is written
request-relative: the wall arm's `--wall=10` starts its clock at the decoded
request, §0.1's "a publication completed after 10.000 s cannot change that
verdict" is request-relative, and the evidence audit has a whole chapter about a
driver that compared a loop-relative clock against it. A calibrated trajectory
has **no clock at all**, so it cannot subtract its own constructor; the
constructor is charged and uncapped by the spec, so the search's share of a
10.000 s request is `10.000 − 2.310938`, and that subtraction is done once, in
`budget.py`, from a pinned probe. Handing the pacer the whole 10.000 s would
have spent ten seconds of search on top of a 2.3 s constructor and failed clause
(5) by construction rather than by measurement.

**One defect found on the way, recorded and worked around rather than patched.**
`shelf_work_plan`'s non-`ics-profile` branch divides
`outcome.trace.work.sample_evaluations` — the engine's **cumulative** work
vector, prefix included — by `search_seconds`, which is the **probe alone**. The
witness is `evidence/shelfplan-defect-witness.json`, one `--cell=spawntax
--icscal=` run on the gate binary:

| | |
|---|---:|
| the writer's own plan | `observedUnits` **7,694,847** over `observedSeconds` **2.404511** → **3,200,171 u/s** |
| the probe bite's own counter | **6,605,800** over the same **2.404511** → **2,747,252 u/s** |
| the cheap prefix, which is the difference | **1,089,047** units, spent in a *different* 0.406 s |
| ratio | **1.1649 — 16 % fast** |

…under a `derivation` string that says the rate "includes the cheap prefix and
is deliberately slower than the shelf's". It *is* slower on a profiling build,
where the branch above it takes the shelf's own barrier-to-barrier wall, so the
census is unaffected — it measured that build and took the other branch. A plan
that over-promises a rate overruns, which is the one direction the conservative
rounding rule exists to avoid. **This wave may not edit engine code, so it did
not**: `budget.py` reads the two counters the document already carries and does
the arithmetic where it can be seen. It is the next round's one-line repair.

## 4. §0's verdict

**`GATE_PASS: false`.** Read on the control, because the control is the member;
both arms are in the table.

| clause | control | treatment | verdict |
|---|---|---|---|
| **(1)** ≥5/9 exact-valid ≤168.484 mm | **0 / 9** | **0 / 9** | **FAIL** |
| **(2)** median ≤168.484 mm | **179.07608** | **179.07957** | **FAIL** |
| **(3)** every publication Exclusive r=2.500 + contract-valid | 0 invalid, all revalidated | 0 invalid, all revalidated | **PASS** |
| **(4)** per-seed two-process bit identity | 9/9 identical | 9/9 identical | **PASS** |
| **(5)** quiet-box p95 ≤10.000 s over 5×9 | **9.5271 s** | **9.4191 s** | **PASS** |
| **(6)** attribution vs the control arm | — | — | **draw, not promoted** |

Contract: `twoRMicron: 5000.0` — Exclusive `r = 2.500` — and `--revalidate=1` on
every gate cell, so every publication's depth and fingerprint were recomputed by
the untouched validator and matched **bitwise**.

### Per seed, per arm, 10 s

| seed | control | treatment | paired gain (control − treatment) |
|---:|---:|---:|---:|
| 0 | 179.07608 | 176.29856 | **+2.7775** |
| 1 | 179.08099 | 179.08099 | 0.0000 |
| 2 | 171.57160 | 179.07957 | **−7.5080** |
| 3 | **169.11881** | 169.22504 | −0.1062 |
| 4 | 179.08123 | 179.08123 | 0.0000 |
| 5 | 179.07170 | 179.07170 | 0.0000 |
| 6 | **169.69284** | 169.69284 | 0.0000 |
| 7 | 179.08210 | 179.08210 | 0.0000 |
| 8 | 179.08210 | 179.08210 | 0.0000 |

Nothing is under 168.484. The two closest are seeds 3 and 6 at 169.119 and
169.693 — **0.63 mm and 1.21 mm short**.

### Clause (4): quality is deterministic in work space

Every one of the **five repetitions** of every (seed, arm) cell published the
*same depth to the last bit*, and the first two processes of each are
byte-identical as whole documents once `wall` is stripped. **90 cells, 18
identity pairs (9 seeds × 2 arms), `ALL_BIT_IDENTICAL: true`.** That is the
property a work budget
exists to buy, and it is the one clause of §0 that a wall budget cannot state at
all.

The wall, meanwhile, is a distribution — which is the spec's own wording. Over
the five repetitions of one cell it moves by **0.03–0.42 s**; across the nine
seeds it moves by **2.1 s**, from 7.44 s to 9.55 s, because the seeds that break
off the shelf spend their units more slowly.

### Clause (5): the claim the amendment fought over

| arm | readings | min | median | **p95** | max | ceiling |
|---|---:|---:|---:|---:|---:|---:|
| control | 45 | 7.483 | 7.906 | **9.5271** | 9.552 | 10.000 |
| treatment | 45 | 7.436 | 7.905 | **9.4191** | 9.440 | 10.000 |
| pooled | 90 | 7.436 | 7.906 | **9.4784** | 9.552 | 10.000 |

Measured on the **driver's own process wall** — request-relative, strictly
larger than anything the document reports — so the p95 cannot be flattered by
choosing a frame. `loadavg` one-minute figure **0.64** at the battery's start.

Grok refused to sign a text in which this clause was "measured as-is and
reported". It binds, it was measured, and **it passes** with 0.45 s of headroom
at the 95th percentile and 0.45 s at the maximum.

### Clause (6): attribution — a draw, and one real regression

Seed gain **0** (both arms 0 qualifying, ≥2 required). Paired median gain
**0.0000 mm** (≥1.000 mm required). **The impatient work-quanta policy is NOT
promoted; the control's frozen `200/3/100/5/0.98` remains the member.**

Six of the nine seeds are bit-identical between the arms — at this budget the
work quantum is never reached, so the two policies never diverge. The three that
differ are the finding:

* **seed 0: treatment 2.78 mm better.** 46 publications against the control's
  22; the control struck once, the treatment not at all.
* **seed 2: treatment 7.51 mm WORSE.** The control published 115 times, struck
  3 times and disrupted twice; the treatment published 22 times, struck none and
  disrupted once.
* **seed 3: treatment 0.11 mm worse**, 80 publications against 85.

The mechanism is the same in both directions and that is the point: on all three
seeds the work quantum is reached **less often** than the 200-batch counter, so
the treatment abandons fewer bites. On seed 0 not abandoning was right and on
seed 2 it was expensive. The two policies disagree about when a bite is finished
and **neither is uniformly correct on this fixture** — which is exactly what a
draw means, and why the clause is written as an evidential bar rather than as a
preference.

Seed 5 — the spec's named watch-seed for treatment regressions, the one needing
~288 continued-search iterations rather than a strike — did **not** move at 10 s:
both arms read 179.07170. The regression landed on **seed 2**, a fast-cascade
seed, which no one had named. One seed each way and a 7.5 mm loss against a
2.8 mm gain is not evidence for promotion, and the clause says so without needing
to be argued with.

**At 30 s the watch-seed moves, and it moves the way the spec feared.** See §5.

## 5. The 30 s clauses

> 30 s: median ≤163.00461; ≥7/9 ≤168.484 (no-regression); paired ≥1.000 mm;
> shapes-17/triangle-20 within 1 mm at equal work; zero invalid publications.

| seed | control | treatment | control − treatment |
|---:|---:|---:|---:|
| 0 | **160.30502** | 164.01207 | −3.7071 |
| 1 | 179.08099 | 179.08099 | 0.0000 |
| 2 | **163.54145** | 165.12329 | −1.5818 |
| 3 | **164.00186** | 164.01443 | −0.0126 |
| 4 | **164.19228** | 169.38404 | −5.1918 |
| 5 | **162.84794** | 179.07170 | **−16.2238** |
| 6 | **164.00689** | 164.00689 | 0.0000 |
| 7 | 179.08210 | 177.90562 | +1.1765 |
| 8 | 179.08277 | 179.08210 | 0.0000 |

| clause | control | treatment | verdict |
|---|---|---|---|
| median ≤ 163.00461 mm | **164.00689** | 169.38404 | **FAIL** |
| ≥7/9 ≤168.484 (no-regression) | **6 / 9** | 4 / 9 | **FAIL** |
| paired ≥ 1.000 mm | — | median **−0.0126 mm** | **FAIL** |
| zero invalid publications | 0 | 0 | **PASS** |
| shapes-17 / triangle-20 within 1 mm at equal work | — | — | **NOT-RUN** |

The last row is the amendment's, verbatim: a single-fixture work plan has no
equal-work denomination on another fixture, so the clause is **inevaluable** and
is recorded not-run. It is not a pass, it is not a fail, and it is not silently
dropped — `NOT-RUN` is a value `evidence/verdict.json` prints.

**The control's 30 s median is 164.00689 against the wall arm's committed
164.005** — the two budgets agree to 2 µm on the statistic §0 gates. The
no-regression clause is where they part: the wall arm put **7** of 9 under
168.484 and the work plan puts **6**, and the seed that moved is **seed 1**
(165.006 on the wall arm, 179.081 here). One seed, and it is the whole
difference between the two clauses' outcomes.

**Seed 5 is the finding.** The spec's three-regime map named it before any of
this was built — *"seed 5 needs ~288 continued-search iterations, not a strike —
the watch-seed for treatment regressions"* — and at 30 s the treatment loses
**16.22 mm** on it, from 162.848 to 179.072, which is the shelf. The
work-denominated quantum struck a bite that needed to keep searching. That is
the pre-named failure of the impatient policy, measured, on the seed it was
predicted on.

**One 30 s wall overran badly and it is not hidden.** Seed 7, treatment: a
**39.83 s** process wall against a 30.000 s budget, 33 % over. The work plan
bounds *work*, not seconds, and a bite whose batches are unusually expensive
spends its allocation slowly. Clause (5) is a claim about the **10 s** battery's
5 × 9 and it passes there; there is no clause on the 30 s wall and this document
does not invent one. It is the clearest single illustration of "quality
deterministic, wall a distribution", and at three times the budget the
distribution has a tail the 10 s battery never showed.

## 6. The curves — 3 s and 60 s, reported, never gated

§0: *"60 s reported, never gated"*, and the 3 s cell has never been a clause
either. Both arms, all nine seeds, the same plan, the same frame subtraction.

### 3 s

| seed | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| control | 179.076 | 180.070 | 179.429 | 179.890 | 179.077 | 180.065 | 179.144 | 179.025 | 179.082 |
| treatment | 179.076 | 180.070 | 179.429 | 179.890 | 179.077 | 180.065 | 179.144 | 179.025 | 179.082 |

Median **179.144** on both arms, **0 of 9** under the bar, and **every seed
identical between the arms** — at 0.674 s of search the work quantum is never
approached and the two policies are the same trajectory. Median process wall
**2.821 s** against a 3.000 s budget, maximum 2.855 s: the plan under-runs here
by design, which is the direction the safety factor points.

The committed wall arm read **179.004–179.422** across the nine seeds at 3 s
against this plan's **179.025–180.070**, so the two budgets agree to within
about 0.65 mm at this end of the curve — the work plan is very slightly behind,
which is the same 0.80 discount showing up where there is least search to lose.

### 60 s

| seed | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| control | 160.595 | 164.012 | **159.954** | 161.727 | 160.975 | 159.973 | 162.341 | 162.494 | 167.295 |
| treatment | 159.870 | 166.250 | 164.575 | 161.747 | 160.972 | 166.186 | 164.000 | **176.992** | 165.904 |
| control − treatment | +0.725 | −2.238 | −4.621 | −0.020 | +0.004 | −6.212 | −1.660 | −14.498 | +1.392 |

| | control | treatment |
|---|---:|---:|
| median | **161.727** | 164.575 |
| under 168.484 | **9 of 9** | 8 of 9 |
| paired median gain | — | **−1.660 mm** |
| max process wall | 60.91 s | **97.37 s** |
| invalid publications | 0 | 0 |

**Given a minute, the control puts every one of the nine seeds under the bar** —
including seeds 7 and 8, the different-basin pair no clause may require, at
162.494 and 167.295. So the trajectory is not stuck; it is slow. That is the
same conclusion the campaign's earlier rounds reached at 30 s and it is stated
here at 60 s with a work-denominated budget behind it.

The treatment is worse again — median **+2.85 mm**, paired median **−1.66 mm**,
one fewer seed under the bar — and it owns both of this wave's wall outliers:
**97.37 s against a 60.000 s budget** on seed 7, after **39.83 s against 30.000**
on the same seed. Neither is gated and neither is hidden; both are the same
mechanism, a work allocation spent slowly by expensive batches on a bite the
impatient policy did not abandon.

## 7. The interleaved AB/BA control — a diagnostic, never a lane

The one question the gate cannot answer about itself: **how much quality does
the work budget cost against the wall budget it replaces?** So each seed runs
the pair twice, in both orders — A the work plan, B the old `--mode=wall`
10.000 s arm — because an asymmetry between AB and BA is the box drifting during
the battery, and only a symmetric difference is a difference between the
budgets.

| seed | AB: work / wall | BA: wall / work | wall − work (AB, BA) |
|---:|---|---|---:|
| 0 | 179.0761 / **172.1776** | **171.8887** / 179.0761 | −6.898, −7.187 |
| 1 | 179.0810 / 179.0810 | 179.0810 / 179.0810 | 0.000, 0.000 |
| 2 | 171.5716 / **167.6827** | **168.6587** / 171.5716 | −3.889, −2.913 |
| 3 | **169.1188** / 169.2227 | 169.2290 / **169.1188** | +0.104, +0.110 |
| 4 | 179.0812 / 179.0812 | 179.0812 / 179.0812 | 0.000, 0.000 |
| 5 | 179.0717 / 179.0717 | 179.0717 / 179.0717 | 0.000, 0.000 |
| 6 | 169.6928 / 169.6536 | 169.0895 / 169.6928 | −0.039, −0.603 |
| 7 | 179.0821 / 179.0821 | 179.0821 / 179.0821 | 0.000, 0.000 |
| 8 | 179.0821 / 179.0821 | 179.0821 / 179.0821 | 0.000, 0.000 |

**The two orders agree on every seed** — the largest AB↔BA disagreement is
0.98 mm on seed 2 and the rest are under 0.6 mm — so nothing here is the box
drifting.

| | work plan | old wall arm |
|---|---:|---:|
| cells under 168.484 mm | **0 of 18** | **1 of 18** |
| median process wall | **7.916 s** | 10.016 s |
| median `wall − work` | \- | **0.000 mm** |
| mean `wall − work` | \- | **−1.184 mm** |

So the honest summary is: on **six of nine seeds the two budgets publish exactly
the same depth**, and the difference is concentrated in three — the wall arm
gains 6.9–7.2 mm on seed 0 and 2.9–3.9 mm on seed 2, and *loses* 0.1 mm on
seed 3. It buys that with **2.1 s more wall per cell**. The one cell in the whole
battery that goes under the bar is a wall-arm cell (seed 2, AB, 167.683).

That is the cost of the 0.80 safety factor stated as a measurement rather than
as an excuse, and it is why §4's failure is reported as a **quality** failure and
not as a budget artefact: even at 1 of 18 the wall arm is nowhere near 5 of 9.

## 9. The files

| file | what |
|---|---|
| `section0.py` | re-extracts §0 from the spec by its heading and requires this README's quoted copy to be byte-equal, plus the amendment's fallback line word-for-word. Imports `../gate/section0.py` rather than re-typing it. Exit is the verdict. |
| `counter.py` | **rider (i)**: the `published_bites` vector across two processes and two builds, before any coefficient. Exit is the verdict. |
| `../meter/currency.py` | the three calibration cells and the meter, unchanged except for two additive `U'` summary fields. |
| `rejectgate2.py` | the amended reject rule over N runs, all three fixtures, nothing dropped; rider (ii) reported; `P` measured against the amendment's own future-`R` rule. Exit is the verdict. |
| `budget.py` | the declared fallback's plan: the shelf probe, the compress rate, the frame subtraction, and the engine reader's own hit check. |
| `quiet.sh` | the quiet box as a gate rather than a habit. Run before every timed battery. |
| `gate.py` | the five batteries. Decides nothing. |
| `verdict.py` | §0's six clauses applied to what `gate.py` measured. Contains no threshold of its own — every bar is quoted. Exit is the verdict. |
| `heavy.sh` | the boundary tier: FAST, four pinned gates × two builds, five suites, determinism in both forms, and the §4 binary trap as a measurement. |
| `evidence/shelfplan-defect-witness.json` | the one engine defect this wave found and did not fix. |

Every reduction names the bytes it reduced: each cell row carries its
`sourcePath` and `sourceSha256`, each battery document carries `cellSources`
for every process it spawned including ones the reduction dropped, and the
binary's sha256 is recorded on **both sides** of every battery
(`binaryUnchangedDuringBattery`).

## 10. Honest caveats

* **The gate failed on quality, and the budget is part of why.** The work plan
  is spent against a rate discounted by 0.80 and buys **72.7 %** of the
  search a 10.000 s wall run does (median 5.595 s against 7.694 s, §7). Two
  seeds miss the bar by 0.63 mm and
  1.21 mm. The safety factor is the campaign's, it was fixed before any gate
  cell ran, and the amendment forbids retuning it after seeing `p95` — so this
  is a caveat on the reading, **not** grounds for a second attempt. A budget
  that spent the full ten seconds would have had a different clause-(5) risk and
  the same clause-(1) problem: the wall arm at a full 10.000 s put **two** seeds
  under the bar in the committed rerun, against a bar of **five**.
* **One machine, x86_64, 16 cores, and seconds are a statement about a box.**
  Every timed battery is preceded by `quiet.sh`, which blocks until
  `/proc/loadavg`'s one-minute figure is below 1.00 and prints what it saw. The
  gated battery — the 10 s 5 × 9 — started at **0.64**. The others are recorded
  in their own documents' `machine.loadBefore`, and the box did not always come
  down as fast as the first battery; where a battery started closer to the
  threshold the reading is in the document rather than rounded away.
* **`p95` is measured on the driver's process wall**, which is request-relative
  and strictly larger than any number the document reports. That is the
  strictest available reading and it was chosen before the numbers arrived; a
  looser frame would have made clause (5) pass by more.
* **The 10 s battery's five repetitions are wall readings and identity
  processes at once.** The first two of each cell are clause (4)'s two
  processes; all five are clause (5)'s wall. That is deliberate — a second
  process of a bit-identity pair is a wall reading whether or not anyone reads
  it — but it does mean clause (5)'s 45 readings per arm are 9 cells × 5 rather
  than 45 independent cells.
* **`U'`'s `P` and `D` are a fit, not two readings.** No timer exists around the
  cut, the pose install, the publication commit, the row rebuild or the
  disruption, so both prices come out of one unaccounted residual by
  non-negative least squares over three fixture rows. The design matrix and each
  fixture's miss are printed in `calibrationPrime.residualSplit`. A round that
  wants a tight `P` adds one timer and re-derives — which is an engine edit and
  therefore a different proposal.
* **`R` being absent does not delete the repair's wall.** It is inside `exactNs`
  and is therefore charged to `E`. That is what dropping a term means.
* **The clause-(6) reading is an interpretation and it is written down.** §0
  lists (6) among the clauses a PASS needs; the promotion sentence it points at
  defines a *draw* as a valid outcome — "the control's policy remains the
  member" — rather than as a failure. So (6) is read as "attribution was
  performed and rendered a verdict". Reading it the other way would make §0
  unpassable whenever the arms tie. On this gate the choice changes nothing:
  clauses (1) and (2) fail either way.
* **The wall arm's own 10 s quality is not stable across sessions.** The
  committed rerun read 179.076 on seed 0 at 10 s; the AB/BA cells here read
  ~172 on the same seed with the same flags. Three waves of engine work and a
  different session sit between them, and a wall budget's trajectory depends on
  how much wall the box gives it. This is an argument *for* work-denominated
  budgets and *against* reading any single wall-arm number as a fixed point —
  including the ones this document compares against.
* **`integration/armgate.py`'s cross-binary arm comparison was not re-run.** It
  needs a second checkout of the round's base commit and this run is isolated to
  one worktree. What replaces it is stronger for this wave's specific change and
  weaker in general: `search/overlap_ics/` — the whole trajectory — is
  **byte-for-byte unchanged** since `e4da8c5`, recorded as a `git diff` in
  `evidence/trajectory-unchanged.txt`; the only engine-side edit is an additive
  `U'` section in the **meter**, which no trajectory can reach. The four pinned
  gates on two builds and the two determinism documents are the measured half.
* **Nothing here is a transfer claim.** The plan says so in its own
  `provenance` string, every document that carries a number from it repeats the
  label, and `U'`'s rejection is the reason the label exists.
* **`evidence/gate10.json` is large — about 5 MB — and deliberately so.** Every
  cell row carries its raw `outcome.bites` array, which is Sol review 18's
  second non-gating risk answered: round 1's reduction dropped it and the
  README's per-bite claims stopped being reconstructible from committed
  evidence. Four fifths of that bulk is the five repetitions of each cell, and
  those are *proven* bit-identical in the same document — so it could be
  trimmed. It was not, because a reduction that quietly drops rows after the
  fact is the shape of defect this campaign keeps re-learning, and the raw
  per-cell documents it came from are named by `sourceSha256` either way.

## 11. What was not run

Named, so nobody has to infer it from a silence.

* **The 30 s shapes-17 / triangle-20 equal-work clause: NOT-RUN.** Inevaluable
  on a single-fixture work plan, and recorded as `NOT-RUN` by the amendment's
  own instruction rather than passed, failed or dropped.
* **The persistent executor's promotion battery.** There is no persistent
  executor: wave 1's census closed that gate at 5.082 % against a 10.000 % bar,
  so ≥1,024-batch identity against a second executor, ≥1.15× shelf p50, ≥1.10×
  geomean, ≤5 % regression and ≤10 % RSS have no second arm to run against. The
  half that does have one is in FAST and green.
* **`integration/armgate.py`'s cross-binary arm comparison** — see §10. The
  source-identity check replaces it for this wave's change and does not replace
  it in general.
* **The scheduling-order perturbation vector** the FAST union names. It belongs
  to the refused executor branch and forcing completion order needs test-only
  concurrency inside `tournament`. Unchanged from wave 4.
* **No second attempt at a currency.** Rider (iii): `U'` was the last one this
  funding sees, and `pSpreadUnderFutureRRule` is what the next proposal will be
  asked for rather than an invitation to make it now.
* **No retune of anything.** Not the safety factor, not the quanta, not the
  probe, not the budget after `p95`.
