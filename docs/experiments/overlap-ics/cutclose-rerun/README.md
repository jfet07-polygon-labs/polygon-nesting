# `CutCloseRelocate`, the rerun — the identical frozen wall, on the repaired predicate

The one rerun the failure license grants. Round 1 FAILED the gate 0 of 9
([`../cutclose-round1/`](../cutclose-round1/README.md)); both implementation
reviews then returned **(A) — line-level defect**, independently, on the same
line ([`docs/sol-review-18-the-strike-predicate.md`](../../../sol-review-18-the-strike-predicate.md)
§P0, [`docs/grok-review-13-the-strike-predicate.md`](../../../grok-review-13-the-strike-predicate.md)
flag 3). Exactly one semantic change was made — `Engine::separate`'s
no-improvement counter now resets only on a ≥2 % raw-Φ improvement, via a shared
`observe_raw` the vector and the engine both call — and this document is the
**identical** gate re-measured on the repaired binary.

Nothing else moved. Not the publication band, not `proxy_depth > T`, not the
`exactAttempts` counter (its overclaim is *recorded* in §9 and left alone), not
the worker count, not the thread-pool shape, not the bite sizes, not the GLS
multipliers, not the sample counts, not the pool-restore weight policy, not a
knob.

The two halves of this document are written at two different times, on purpose,
exactly as round 1's were.

**§0 is the pre-committed reading.** It is round 1's §0 —
[`../cutclose-round1/README.md`](../cutclose-round1/README.md) §0, itself
[`docs/cutclose-relocate-spec.md`](../../../cutclose-relocate-spec.md) §"The
gate" copied verbatim — **copied verbatim again, unchanged, and committed to
this file before any wall second of this rerun was spent**. It is unchanged for
the rerun because the license says the rerun is of *this same gate*: same seeds
0..=8, same 3/10/30 budget-response cells, same interleaved AB/BA control, same
floor. Nothing in it may be edited now that numbers exist. Only a result section
may be appended.

**§1 onward is the result.** Written after.

---

## 0. The pre-committed reading (binding)

### 0.1 The gate, verbatim from `docs/cutclose-relocate-spec.md` §"The gate"

> From the bare mixed-61 request, one release binary (feature overlap-ics),
> 8 workers, seeds 0..=8, 10.000 s wall: **PASS iff ≥3/9 seeds publish a
> strict non-constructor child with exact-valid raw-source depth ≤168.484 mm**,
> every publication of every seed passing Exclusive r=2.500 (allowance 0) and
> the untouched contract validator. Full non-interpolated 3/10/30 curve, all
> nine seeds. Interleaved AB/BA wall-arm control cells, diagnostic only —
> 168.484 is absolute, the control can neither rescue nor kill. Regression
> floor: S0 bit-for-bit, 1k/10k soundness zeros, literal old throughput
> thresholds (new relocate metrics get NEW names — arbitration 4), four pinned
> engine gates on default and feature-compiled-unarmed, default-build
> isolation, jagua-rs/Xoshiro/rand:: absent from the tree. Forbidden rescues
> and the failure license: Grok R2 §6.7 verbatim (one named line-level repair
> with red/green vector, or — children exist in a tight band above the bar
> with first bites publishing — nothing; the member closes; any other family
> is separately funded). S1/triangle-20 become locked-T relocate regressions
> (same pins, relocate-eval quotas).

### 0.2 The gate body, verbatim from Grok review 12 Round 2 §6.7

The arbitration above resolves the two source readings where they differ (Grok
Round 1 §4.1 says one separator worker; the arbitration and §6.7 say eight, and
the arbitration wins). §6.7 is the body it points at:

> **ROUND VALIDITY.** One release binary, feature `overlap-ics`, eight
> Algorithm-10 workers, seeds 0 through 8, one run per seed. S0 remains
> bit-identical at raw depth 150.16451 with Φ bits zero, Exclusive
> `two_r=5000`, untouched contract-valid, zero repair rows and zero giveback.
> Both numeric-soundness populations retain zero false-feasible, zero
> containment false-feasible and zero incremental mismatch. Cold-Φ, row-rebuild
> and cell-gap throughput thresholds remain green. The legacy proposal
> microbenchmark remains recorded under its original meaning; the new member
> additionally sustains at least 100,000 relocate-evals projected into eight
> seconds. All four pinned engine gates pass on default, feature-compiled-unarmed
> and armed builds. `fast.sh` has no stale worktree default.
>
> **PASS.** From the bare mixed-61 request, at the 10.000-second checkpoint, at
> least 3 of 9 distinct seeds have published a non-constructor layout with
> raw-source depth ≤168.484 mm. Every emitted publication at every time passes
> Exclusive `r=2.500` and the untouched publication contract. The complete
> non-interpolated 3/10/30 curve, all nine seeds, is reported. A contemporaneous
> interleaved AB/BA wall-arm control is reported and cannot raise or lower
> 168.484.
>
> **FAIL.** A valid round with fewer than 3 of 9 qualifying seeds fails the
> funded `CutCloseRelocate + 0.1% cut-close` member. Proxy depth, best seed,
> median alone, constructor depth, or a publication completed after 10.000 s
> cannot change that verdict.
>
> **FORBIDDEN RESCUES.** No Sparrow fixture as a seed or warm start; no Sparrow
> or jagua code linked into the engine; no pole proxy, polygon simplification,
> or Xoshiro; no `general_relaxed`, portfolio, m34, crot, contact-block, old
> jump, Union kernel, allowance, 2.502 radius, enlarged repair band, alternate
> feature arm, seed substitution, wall interpolation, post-result bite change,
> \(\beta\) fitted to mixed-61, 8 workers re-read as a later rescue, joint-PGS
> or component-Y retrofit, or rerun selected by outcome.
>
> **FAILURE LICENSE.** A failing valid round licenses one read-only funnel
> autopsy: `bitesStarted → proxyBandReached → exactAttempted →
> dualValidPublished → ≤168.484`. It licenses one rerun only for a line-level
> violation of this frozen specification accompanied by a red/green minimal
> vector. Otherwise the member is closed; joint projection, component-Y, a
> different sampler, and a different homotopy are separately funded proposals.

### 0.3 The two clauses of the failure license that decide what happens next

From Grok review 12 Round 1 §4.5, which §6.7's last paragraph compresses:

> **If the member is the member in §2, the regime is the regime in §3, the
> floor in §4.3 is green, and ≥3/9 ≤168.484 is still false → nothing further on
> this family.** No worker-count round, no bite-size search, no PGS-pair
> retrofit, no chain operator, no homotopy bisection. Write the kill as:
> Sparrow-faithful relocate + 0.1 % split-and-close, on our Φ and our dual-valid
> judge, did not beat 168.484 at 10 s on 3/9.
>
> The **one** exception that is a diagnosis rather than a new family: if ≥3/9
> produce **strict dual-valid children** and the 10 s depths sit in a tight band
> **above** 168.484 (constructor 182.976 minus a handful of 0.1 % bites), and
> the first 0.1 % bite **does** publish, that is **throughput, not basin**. It
> licenses **one** follow-up that only raises separator workers, with this same
> gate. It does not license a new operator.
>
> A first 0.1 % bite that does **not** publish on mixed-61 is a **member** fail,
> not a throughput fail. 0.183 mm is inside S1.

Sol review 17 Round 2 §6 adds one amendment to that exception, and it is
binding here because the arbitration seats eight workers from the start:

> Because workers=8 are already present, it does not license a scaling
> follow-up.

**So the reading this round is bound by, stated once and before any number:**

| what the battery shows | verdict | what it licenses |
|---|---|---|
| ≥3/9 seeds ≤168.484 at 10.000 s, every publication dual-valid, floor green | **PASS** | nothing further is needed |
| any single invalid publication, any seed, any time | **FAIL** | a defect hunt, not a retarget |
| <3/9, floor green, member and regime as specified | **FAIL** | one funnel autopsy; the member **closes** |
| <3/9 **and** a named line-level violation of this spec with a red/green vector | **FAIL** | one named repair, one re-run of this same gate, then stop |
| a regression-floor break | **defect in this round**, not a retarget | investigate the round, do not touch the cell |

The 3 s and 30 s cells **cannot pass or fail** this gate. They are reported in
full, non-interpolated, for all nine seeds. The interleaved AB/BA wall-arm
control is reported and is diagnostic only.

### 0.4 What must be true for the round to be valid at all

Checked and recorded **before** the wall battery is started, not after:

1. **FAST fully green**, including the first-bite canary. Grok review 12 Round 2
   §6.3.4: "FAIL here is a member fail; do not run the 9-seed wall." The wall
   driver reads `CANARY_PASS` out of the canary's own document and refuses to
   start without it.
2. **One release binary**, feature `overlap-ics`, whose SHA-256 is recorded in
   every evidence document it produces.
3. **The bare mixed-61 request** — no pinned parent, no warm start, no Sparrow
   pose fixture anywhere on the live path. The fixture is read by S0/S1/S2 and
   by nothing else.
4. **Eight workers**, `--workers=8`, one process per cell.
5. **Seeds 0..=8**, one run per seed at each of 3 / 10 / 30 s, as *separate
   budget-response cells* — not one 30 s trajectory sampled three times.
6. **The clock starts at the decoded bare request** and the constructor is
   charged against it, uncapped (arbitration 3). Sparrow's own timer excludes
   import and LBF; ours does not, and that asymmetry is documented rather than
   compensated.

### 0.5 The funnel the autopsy is licensed to read

One row per seed, and nothing else is licensed without a new proposal:

```
bitesStarted → proxyBandReached → exactAttempted → dualValidPublished → ≤168.484
```

### 0.6 The one sentence this rerun adds, pre-committed before any wall number

Everything above is round 1's §0, byte for byte. This is the only clause that is
new, and it exists because §0.3's fourth row grants *"one named repair, one
re-run of this same gate, **then stop**"* and someone has to write down in
advance what "then stop" means:

> **PASS iff ≥3/9 seeds publish a strict non-constructor child with exact-valid
> raw-source depth ≤168.484 mm at 10.000 s.** If fewer than 3 of 9 qualify with
> the regression floor green and the green vector holding — that is, with the
> repaired predicate demonstrably changing the trajectory it was named for —
> **the `CutCloseRelocate` member CLOSES on a now-faithful FAIL, and no further
> license exists.** A new named line-level defect of the same grade may be
> REPORTED unrepaired, as round 1's autopsy reported two, but it does not reopen
> this round.

The **green vector** that clause refers to is Grok review 13 §(A)'s, also
pre-committed here before it was run: on the repaired binary the seed-1 30 s
cell must record `strikes ≥ 3` and `disruptions ≥ 1` on bite 22 — the 200-counter
no longer restarting on 1e-15 minima — against round 1's `5319 / 0 / 0`, and
secondarily the stuck bite-22 rows across the nine 10 s cells must stop being
uniformly `disruptions: 0`. That is a check on the *repair*, not on the gate. It
cannot pass or fail 168.484, and a green vector with a failing quorum is exactly
the outcome the clause above closes the member on.

---

*Everything below this line was written after the numbers arrived.*
---

# Part I — the verdict

## 1. The gate

**FAIL.** 2 of 9 seeds published a strict non-constructor child at or below
168.484 mm within 10.000 seconds. The quorum §0.1 requires is 3.

| clause | required | measured |
|---|---|---|
| seeds ≤168.484 mm at 10.000 s | ≥3 of 9 | **2 of 9** (seeds 2 and 3) |
| every publication dual-valid, every seed, every budget | 0 invalid | **0 invalid** of 1,701 |
| all nine seeds produced a valid run | 9 of 9 | 9 of 9 |
| first 0.1 % bite publishes | yes | **yes**, on all nine |

The best 10-second depth of the round is **167.31508 mm** (seed 3), 1.169 mm
*below* the bar; the second is **167.95169 mm** (seed 2). Round 1's best was
169.00246 and its quorum was 0. The repair moved the count from 0/9 to 2/9 and
the bar is 3/9, so the gate says the same word it said in round 1.

`GATE_PASS: false`, `wall.py` exit **1**.

## 2. The full 3/10/30 curve, non-interpolated, all nine seeds

Best **strict non-constructor dual-valid** child published at or before each
budget. Separate processes, separate budget-response cells — not one 30 s
trajectory sampled three times. `bites` is explore bites published.

| seed | 3.000 s | bites | 10.000 s | bites | 30.000 s | bites | ≤168.484 at 10 s |
|---:|---:|---:|---:|---:|---:|---:|:--:|
| 0 | 179.07614 | 21 | 179.07609 | 21 | 161.05499 | 120 | no |
| 1 | 179.42186 | 19 | 179.08099 | 21 | 165.00578 | 76 | no |
| 2 | 179.07962 | 21 | **167.95169** | 63 | 163.56062 | 111 | **yes** |
| 3 | 179.05642 | 21 | **167.31508** | 72 | 164.00461 | 109 | **yes** |
| 4 | 179.06842 | 21 | 179.08123 | 21 | 164.00094 | 109 | no |
| 5 | 179.07175 | 21 | 179.07170 | 21 | 162.40477 | 117 | no |
| 6 | 179.05432 | 21 | 169.17186 | 74 | 164.00930 | 105 | no |
| 7 | 179.00421 | 21 | 179.08210 | 21 | 179.08210 | 21 | no |
| 8 | 179.08215 | 21 | 179.08210 | 21 | 179.06000 | 21 | no |
| **≤168.484** | **0 / 9** | | **2 / 9** | | **7 / 9** | | |

The 3 s and 30 s columns **cannot pass or fail this gate** (§0.1). They are
reported in full because §0 requires it.

Against round 1, cell for cell:

| seed | 10 s round 1 | 10 s rerun | Δ | 30 s round 1 | 30 s rerun | Δ |
|---:|---:|---:|---:|---:|---:|---:|
| 0 | 179.07686 | 179.07609 | −0.001 | 164.00236 | **161.05499** | **−2.947** |
| 1 | 179.08099 | 179.08099 | +0.000 | 179.08099 | **165.00578** | **−14.075** |
| 2 | 179.07957 | **167.95169** | **−11.128** | 168.66303 | 163.56062 | −5.102 |
| 3 | 169.21860 | **167.31508** | −1.904 | 164.00577 | 164.00461 | −0.001 |
| 4 | 179.08123 | 179.08123 | +0.000 | 165.05518 | 164.00094 | −1.054 |
| 5 | 179.07170 | 179.07170 | +0.000 | 163.69242 | 162.40477 | −1.288 |
| 6 | 169.00246 | 169.17186 | +0.169 | 164.00972 | 164.00930 | −0.000 |
| 7 | 179.08211 | 179.08210 | −0.000 | 179.08210 | 179.08210 | +0.000 |
| 8 | 179.08211 | 179.08210 | −0.000 | 179.08210 | 179.06000 | −0.022 |

Nine 30-second cells improved or held; **7 of 9 now go below the bar at 30 s**
against round 1's 5, and 161.05499 (seed 0) is the best depth this member has
ever produced on mixed-61. None of that is a gate clause and none of it is
offered as one.

## 3. The interleaved AB/BA wall-arm control — diagnostic only

Nine pairs at 10.000 s, separate processes, AB on even seeds and BA on odd
ones. **It cannot raise or lower 168.484** (§0.1). Arm B is the campaign's
published wall arm on its own pinned positional tail, `wall=10000,v3=1`, from a
binary whose SHA-256 is byte-identical to round 1's — the repair does not
compile into it.

| seed | order | A `CutCloseRelocate` | B old wall arm | A − B |
|---:|:--:|---:|---:|---:|
| 0 | AB | 179.07609 | 170.45273 | +8.623 |
| 1 | BA | 179.08099 | 165.65578 | +13.425 |
| 2 | AB | 167.91944 | 174.28000 | **−6.361** |
| 3 | BA | 169.21217 | 172.28409 | **−3.072** |
| 4 | AB | 175.00538 | 172.12900 | +2.876 |
| 5 | BA | 179.07170 | 179.63300 | **−0.561** |
| 6 | AB | 169.08134 | 168.46800 | +0.613 |
| 7 | BA | 179.08210 | 169.35992 | +9.722 |
| 8 | AB | 179.08211 | 169.03159 | +10.051 |
| **median** | | **179.07170** | **170.45273** | |

`CutCloseRelocate` beats the old wall arm on **3 of 9** seeds here against round
1's 2. Every arm-B run was dual-gate valid; arm A published nothing invalid.

Two facts a reader is owed, neither of which moves the bar. Arm B reproduces
**168.46800** on seed 6, to five decimals the published 168.484's neighbourhood,
so the box is healthy. And arm B is again a lottery: **13.977 mm** of spread
across its nine seeds and a median **1.969 mm** from its own published number.
Six of its nine cells returned exactly round 1's value; three (seeds 0, 3, 7)
did not, so arm B is not reproducible run to run either. 168.484 remains the
bar, absolutely, and this document does not relitigate it.
---

# Part II — the green vector, which is what the repair was licensed for

## 4. The state machine, in the tree

`evidence/strike-red.log` carries both halves of Sol review 18 §P0's vector as
transcripts, and `evidence/strike-red.patch` is the one-token diff that
reproduces the red.

| | red — round 1's rule | green — the repaired rule |
|---|---|---|
| observations fed | 10,000 | 222 |
| resets (`Substantial`) | 1,000 | 0 |
| paused (`Marginal`) | 0 | 22 |
| counted (`None`) | 9,000 | 200 |
| counter at the end | **0** | **200** |
| strike reached | **never**, at any length | on observation **222** |
| `cargo test` exit | **101** | **0** |

Round 1's counter on this vector is exactly the longest run of consecutive
non-minima in it, which is **nine**, against a limit of 200. That is asserted
in the tree, permanently, without a second copy of the predicate: the vector
drives `observe_raw`, the same function `Engine::separate` drives.

## 5. The green vector on the trajectory — Grok review 13 §(A), pre-committed in §0.6

**Seed 1, 30 s, bite 22 — the cell both reviews named.**

| | master iterations | strikes | disruptions | separation attempts | published | min raw Φ |
|---|---:|---:|---:|---:|:--:|---:|
| **round 1 (red)** | 5,319 | **0** | **0** | 1 | **no** | 3.6957e−05 |
| **rerun (green)** | 3,059 | **6** | **2** | 2 | **yes** | 0.0 |

§0.6 asked for `strikes ≥ 3` and `disruptions ≥ 1`. Measured: **6 and 2**, and
the bite that had swallowed 5,319 master iterations without ever letting
Algorithm 12 run now strikes out twice, is disrupted twice, and **crosses** —
which is why seed 1's 30 s answer moves 14.075 mm, from 179.08099 to 165.00578.

The same comparison across all nine 30-second cells, bite 22 only:

| seed | round 1 iters/strikes/disruptions/attempts/published | rerun iters/strikes/disruptions/attempts/published |
|---:|---|---|
| 0 | 2,061 / 2 / 0 / 0 / yes | 1,424 / 2 / 0 / 0 / yes |
| 1 | **5,319 / 0 / 0 / 1 / no** | **3,059 / 6 / 2 / 2 / yes** |
| 2 | 7,450 / 3 / 1 / 1 / yes | 1,283 / 3 / 1 / 1 / yes |
| 3 | 137 / 0 / 0 / 0 / yes | 137 / 0 / 0 / 0 / yes |
| 4 | 3,622 / 6 / 2 / 2 / yes | 2,032 / 6 / 2 / 2 / yes |
| 5 | 1,700 / 2 / 0 / 0 / yes | 1,142 / 0 / 0 / 0 / yes |
| 6 | 131 / 0 / 0 / 0 / yes | 131 / 0 / 0 / 0 / yes |
| 7 | **3,906 / 0 / 0 / 1 / no** | **5,638 / 17 / 5 / 6 / no** |
| 8 | 3,825 / 4 / 1 / 2 / no | 6,483 / **15** / **5** / 6 / no |

Seeds 7 and 8 are the clearest statement of what the repair did and did not
buy. Round 1 gave bite 22 one or two separation attempts there and 0–1
disruptions; the rerun gives it **six attempts and five disruptions** — the
operator now runs, repeatedly, on exactly the shelf it was written for — and
those two seeds **still do not cross it**. The starvation is fixed. The shelf,
for those seeds, is not a starvation artifact.

Across all 27 cells: **145 strikes and 164 disruptions**, against round 1's
**88 and 122**, on 1,825 bites against 1,391.

## 6. The secondary check, stated exactly

Grok's secondary was that stuck bite-22 rows across the 10 s cells "are no
longer uniformly `disruptions: 0`". Measured, honestly:

| seed | round 1, 10 s, bite 22 | rerun, 10 s, bite 22 |
|---:|---|---|
| 0 | 1,290 / 1 strike / 0 disr / stuck | 1,408 / **2** / 0 / stuck |
| 1 | 797 / 0 / 0 / stuck | 809 / 0 / 0 / stuck |
| 2 | 1,754 / 0 / 0 / **stuck** | 1,283 / **3** / **1** / **published** |
| 3 | 137 / 0 / 0 / published | 137 / 0 / 0 / published |
| 4 | 1,072 / 1 / 0 / stuck | 1,125 / **2** / 0 / stuck |
| 5 | 855 / 0 / 0 / stuck | 854 / 0 / 0 / stuck |
| 6 | 131 / 0 / 0 / published | 131 / 0 / 0 / published |
| 7 | 892 / 0 / 0 / stuck | 893 / **2** / 0 / stuck |
| 8 | 922 / 1 / 0 / stuck | 925 / **2** / 0 / stuck |

**Half met, and the half that is not met has a mechanical reason.** One of the
seven round-1 stuck rows (seed 2) now takes a strike-out and a disruption and
**crosses**, which is the whole of seed 2's 11.128 mm gain and one of this
round's two qualifying seeds. But the six rows that remain stuck at 10 s still
read `disruptions: 0`, because the counter change cannot manufacture time:
`disrupt` runs only when a separation *ends* with budget left, three strikes
take ≥600 non-improving master iterations, and these cells reach the explore
deadline after 800–1,400 iterations total. What did change on four of them is
the strike count itself — 0 or 1 in round 1, 2 in the rerun — which is the same
counter moving in the same direction, one strike short of firing.

At 30 s, where the time exists, the same rows fire five times each. The
mechanism is demonstrated; the 10 s budget is what withholds it.
---

# Part III — the regression floor

## 7. Every clause of §0.1's floor, measured

| clause | required | measured | verdict |
|---|---|---|---|
| **S0 bit-for-bit** | 61 placements, 150.16451, `phi.to_bits() == 0`, `two_r = 5000`, dual-valid, 0 repair rows, giveback 0.0 | all of them | PASS |
| **S0 two-process** | stripped documents identical | identical (`8632386…`) | PASS |
| **S1 locked-`T` relocate regression** | republish inside 150.16547, repair ≤ 16 µm, giveback ≤ 0.050 mm, quota in relocate-evals | republished at **150.16547**, repair **7.968 µm**, giveback **0.000506 mm**, 83,594 relocate-evals of a 200,000 cap, two-process bit-identical | PASS |
| **triangle-20 locked-`T` relocate regression** | publish inside 70.742, same caps | published at **70.74073**, 0 repair rows, giveback 0, 4,080 relocate-evals | PASS |
| **1,000-state soundness** (FAST) | 0 outside the 4 µm band, 0 containment false-feasible, 0 incremental mismatch | 0 / 0 / 0 | PASS |
| **10,000-state soundness** (HEAVY) | same three zeros, force ≥95 % active and ≥80 % total on the `compressed` family | 0 / 0 / 0; force **100.0 %** and **100.0 %** on 5,001 scored steps; `worstBandMicron` 0 | PASS |
| **four pinned engine gates, default build** (`jagua-experimental`) | 206.869/`8a7737381238fa4d`, 159.09233022733062/`fa01012af1d559ae`, 159.07876040364795/`e28fba007f8031d4`, 164.0375677990678/`49f094d7e59a9008` | all four hit | PASS |
| **four pinned engine gates, feature-compiled-unarmed** (`jagua-experimental,overlap-ics`) | same four | all four hit | PASS |
| **whole-document identity between the two builds** | identical with `gatelib.VOLATILE` stripped | **identical, all four** | PASS |
| **default-build isolation** | `--no-default-features --lib` compiles | compiles | PASS |
| **`jagua-rs` absent** | absent from `cargo tree --features overlap-ics` | absent | PASS |
| **`Xoshiro` / `rand::` / `jagua` absent from `search/overlap_ics/`** | absent outside line comments | absent | PASS |
| **`fast.sh` has no stale worktree default** | resolves from the script | resolves from the script | PASS |
| **two-binary determinism** | s0, s1, c175, triangle-20 and the K=8 `cutclose` cell identical across two independently built binaries | identical, all five, **and this round the two binaries really do differ** — see §11 | PASS |
| **five suites** | exit 0 | exit 0, all five | PASS |
| **FAST tier** | 13 stages, exit 0, canary green | 13/13, exit 0, `CANARY_PASS: true` | PASS |

The floor is **green in every clause**. Nothing in this round's FAIL is a
regression, and nothing in the repair moved it.

**The strongest single fact about the blast radius:** the default-build gate
binary (`--features jagua-experimental`) has SHA-256
`61befdc544b4135a929e8e6e38d281e337cfc394fa243de4d6ebb267c8955819`, which is
**byte-for-byte round 1's**. The repair did not move one byte of the shipped
engine.

## 8. Throughput, measured on a quiet box

| metric | threshold | measured | round 1 | clause of `pass` |
|---|---|---|---|---|
| cold Φ | ≤ 200 µs | **36.820 µs** | 31.975 | yes |
| moved-piece row rebuild | ≤ 20 µs | **1.262 µs** | 1.118 | yes |
| convex cell-gap evaluations | ≥ 1 M/s | **7,381,492/s** | 7,438,834 | yes |
| `projectedProposalsInEightSeconds` (retired unit) | — | **66,011** | 61,446 | **no** — arbitration 4, not a clause |
| `projectedRelocateEvalsInEightSeconds` | ≥ 100 K | **5,723,239** (57×) | 5,327,414 | yes |

`rawPhiBeforeProposals` 565.376 → `rawPhiAfterProposals` 65.013, with 2,817
accepted relocates of 7,071 and 245.2 sample evaluations per relocate — the same
loop doing the same work as round 1, to three significant figures.
---

# Part IV — the funnel, and the three things recorded rather than repaired

## 9. `bitesStarted → proxyBandReached → exactAttempted → dualValidPublished`

The one row §0.5 licenses, at the gate budget, summed over the nine seeds:

| stage | 10 s total | share of the previous |
|---|---:|---:|
| `bitesStarted` | 607 | — |
| `proxyBandReached` | 601 | 99.0 % |
| `exactAttempted` | 601 | 100.0 % |
| `dualValidPublished` | 584 | 97.2 % |
| ≤168.484 | **2 seeds of 9** | — |

The funnel does not leak, and it did not leak in round 1 either. What changed is
the numerator: **607** bites started at 10 s against round 1's **350**
(`round1-bites-red.json`, the nine 10 s cells' bite rows counted).

**The `exactAttempted` overclaim, recorded and not repaired.** Both reviews name
it and both refuse to bundle it: `exact_attempts` increments in `separate`
*before* `publish::attempt`, while the publisher rejects over-target poses before
incrementing `exactCheckpoints` ([mod.rs:776](../../../../crates/polygon-nesting-core/src/search/overlap_ics/mod.rs),
[publish.rs:264](../../../../crates/polygon-nesting-core/src/search/overlap_ics/publish.rs)),
and the funnel's own `exactAttempted` counts **bites with ≥1 attempt**, not
attempts. Both are visible in this round's committed evidence now that the raw
rows are in `wall.json`: seed 2's per-bite `exactAttempts` sum to **1,313**
while its funnel row says **174**. So:

* "100 % exact-attempt conversion" is a **bite-count** statement, not an
  attempt-count one, and this document does not make the stronger claim;
* the counters are **not renamed here.** Sol review 18 §1: *"Rename or split the
  counters later; this does not itself license a trajectory rerun."* Renaming
  them inside the licensed repair would have changed a wall document's schema
  between two runs that must be compared.

## 10. What was explicitly not in this repair

Both reviews list them; they are listed again here so that a reader can check
the diff against the list rather than against a sentence.

| named, not done | where it was asked for | why not |
|---|---|---|
| publication band | Grok 13 §(A) "not in this repair" | outside the licence |
| `proxy_depth > T` semantics | Sol 18 §1, Grok 13 flag 1 | **refuted**, not a defect: the strip-top row and proxy depth share one sag-less convention, so `max_g ≤ 0.004` and `proxyDepth > T` coexist exactly in the (0, 4 µm] overshoot window |
| `exactAttempts` counter rename | Sol 18 §1 | recorded above; explicitly deferred by the review that found it |
| worker count | Sol 17 R2 §6, Grok 13 §(A) | eight are seated and a scaling follow-up is voided |
| thread-pool optimisation (8 OS threads per master iteration vs a persistent pool) | Sol 18 general fidelity, risk 1 | *"Measure it later; do not combine its optimization with the strike rerun."* |
| bite sizes, GLS multipliers, sample counts, 200 / 3 / 100 / 5 / 0.98 | both | frozen knobs; none was retuned |
| pool-restore weight policy | Sol 18 §2 | a **declared** frozen difference, not a defect |

The engine diff of this round is `observe_raw` plus its call site. Nothing else
in `crates/` changed except the tests and their doc comments.
---

# Part V — the verdict, applied

## 11. Which row of §0.3's table this round lands on

| what the battery shows | verdict | what it licenses |
|---|---|---|
| ~~≥3/9 seeds ≤168.484 at 10.000 s~~ | — | 2 of 9 |
| ~~any single invalid publication~~ | — | 0 of 1,701 |
| **<3/9, floor green, member and regime as specified** | **FAIL** | **the member closes** |
| ~~<3/9 **and** a named line-level violation with a red/green vector~~ | — | spent; this document is that rerun |
| ~~a regression-floor break~~ | — | the floor is green in every clause |

Row 3, and row 4 is spent. §0.6 pre-committed the consequence before any number
existed, and every antecedent of it is measured:

* fewer than 3 of 9 qualify — **2 of 9**;
* the regression floor is green — **every clause of §7**;
* the green vector holds — **§5**: the repaired predicate demonstrably changes
  the trajectory it was named for, `strikes 0 → 6` and `disruptions 0 → 2` on
  the exact cell both reviews named, and 145 strikes / 164 disruptions across
  the round against 88 / 122.

**Therefore: the `CutCloseRelocate` member CLOSES on a now-faithful FAIL.** No
further license exists on this family. The kill, in the words §0.3 requires:

> **Sparrow-faithful relocate + 0.1 % split-and-close, on our Φ and our
> dual-valid judge, with the frozen 2 % strike predicate implemented as
> specified, did not beat 168.484 at 10 s on 3 of 9 seeds. It beat it on 2 of 9
> at 10 s and on 7 of 9 at 30 s.**

Joint projection, component-Y, a different sampler and a different homotopy
remain separately funded proposals. Nothing in this document argues for one.

## 12. What the repair bought, stated without inflation

It is worth being exact, because two readings of this round are both available
and only one is supported.

**Supported.** The predicate was wrong, both reviewers said so from the source,
and correcting it changed the member's behaviour in the direction they predicted
and by a large amount: the 10 s quorum went 0 → 2, the 30 s sub-bar count went
5 → 7, the best depth this member has ever produced on mixed-61 went from
163.69242 to **161.05499**, and the operator the defect was starving now fires
164 times where it fired 122. Grok's own expectation — *"I do not expect that
rerun to pass 3/9"* — held.

**Not supported.** That one more repair, or one more seed, or a different
box, would carry it to 3/9. Two of the nine 10 s cells are **below** the bar
(−0.532 and −1.169 mm), exactly one is within a millimetre of it on the wrong
side (seed 6, +0.688 mm), and the other six are still welded at 179.07–179.08,
between +10.59 and +10.60 mm out. The distribution
is bimodal, not a tight band, exactly as Sol review 18 §3 said of round 1, and
this round did not change that shape — it moved mass across it. A 10 s cell has
~7.7 s of search after the constructor; the seeds that clear the bar are the
ones that get past `W ≈ 178.99` early, and whether a given seed does is a
heavy-tailed time-to-escape draw. §0 pre-committed one run per seed, and this is
that run.

**One measurement that should temper any reading of "2 of 9" as a near miss.**
The AB/BA control's arm A is the same binary on the same nine seeds at the same
budget in a different process, and it returned **169.21217 on seed 3** where the
wall cell returned 167.31508, and 175.00538 on seed 4 where the wall cell
returned 179.08123. The 10 s outcome of one seed is not reproducible run to run,
because separations end on a clock. The gate is one draw of nine, as §0 fixed it
in advance, and 2 is what this draw returned.
---

# Part VI — honest caveats

1. **One box, one session, one binary.** Every wall number here was taken on the
   same 16-core x86_64 machine as round 1, with the tree clean, the box waited
   down to a load below 1.0 before each timed battery, and the binary's SHA-256
   recorded in every document. Nothing was rerun and nothing was selected by
   outcome. The 27 wall cells ran once each, in seed order, in one pass.
2. **The 10 s cell is not reproducible per seed.** §12 gives the numbers: arm A
   of the control disagrees with the wall cell by 1.9 mm on seed 3 and 4.1 mm on
   seed 4. This is a property of a wall-clocked search and it was true in round 1
   too; it means "2 of 9" is a draw from a distribution, not a constant of the
   member. §0 fixed one run per seed in advance and this document does not get to
   re-draw.
3. **The secondary green-vector check is only half met.** §6. One of the seven
   round-1 stuck 10 s bite-22 rows now disrupts and crosses; the other six still
   read `disruptions: 0`, because three strikes need ≥600 non-improving master
   iterations and those cells reach the explore deadline after 800–1,400 total.
   The strike counts on four of them did move from 0–1 to 2. The primary vector
   (seed 1, 30 s) is unambiguous and the 30 s column as a whole is emphatic; the
   10 s half is a budget limit, and it is reported as such rather than as a pass.
4. **The two-binary determinism cell is stronger than it was designed to be, by
   accident.** `run-suites.sh` suite 4 (`cargo test --release --features
   jagua-experimental,overlap-ics`, unscoped) rebuilds
   `target/release/examples/overlap_ics_benchmark` with a different feature set,
   so when `determinism.py` ran afterwards its binary A was that build
   (`47172fe1…`) and its binary B was the wall binary (`b42c10af…`). The two
   genuinely differ in bytes **and in feature set**, and all five cells were
   still bit-identical. Round 1's caveat 2 — that its two "independent" binaries
   were byte-identical and so the cell varied nothing — does not apply to this
   one. Round 1's underlying observation still reproduces, separately: building
   the *same* invocation into two `CARGO_TARGET_DIR`s gives the same SHA-256
   (`b42c10af…` twice), so the release build is byte-reproducible on this
   toolchain. The wall binary was rebuilt to `b42c10af…` afterwards and every
   other HEAVY document in this round carries that hash.
5. **`repairMaxDisplacementMm` touched 16.000 µm again** — exactly the
   `4 × epsilon_grid` cap — on at least one publication among the 1,701. At the
   cap, not over it, and the cap was not widened.
6. **`exactAttempted` counts bites, not attempts.** §9, with the arithmetic now
   reconstructible from committed evidence (seed 2: 1,313 attempts, 174 bites
   with ≥1). The counters were deliberately not renamed inside a licensed
   single-change repair.
7. **Max deadline overrun +8.07 ms across the 27 cells** (seed 4 at 10 s),
   against round 1's +6.6 ms. Same barrier, same order of magnitude, no clause
   near it.
8. **The 3 s column is close to constructor-only on every seed**, because the
   constructor costs 2.31–2.35 s of the budget and Sparrow's timer excludes
   import and LBF while ours does not (arbitration 3). Unchanged from round 1 and
   documented rather than compensated.
9. **Arm B moved.** Six of the control's nine arm-B cells returned exactly round
   1's value and three did not; its seed-0 cell came back 170.45273 where round 1
   got 168.48360, and its seed-6 cell came back 168.46800 both times. Arm B is a
   lottery with 13.977 mm of spread, which is what Grok review 12 Round 1 §4.2
   refused to build a clause on. It cannot move the bar in either direction.
10. **The red transcript is an assertion failure, not a compile error, and it was
    produced by patching the repaired tree back to round 1's comparison rather
    than by running against `1fd70d0` itself.** `evidence/strike-red.patch` is
    that one-token diff and `git apply` reproduces the transcript. The reason is
    structural: `observe_raw` does not exist on `1fd70d0`, so a vector that
    called it would fail to *compile* there and would prove nothing about the
    rule. The rule exercised in the red run is round 1's, character for
    character in its comparison, reached through the same call site
    `Engine::separate` uses.
---

# Part VII — the round boundary

## 13. HEAVY, in full

| battery | result |
|---|---|
| 10,000-state contact corpus | **PASS** — 0 outside the 4 µm band, 0 containment false-feasible, 0 incremental mismatch, `worstBandMicron` 0, force 100.0 % / 100.0 % on 5,001 scored `compressed` steps, 20.5 s |
| four pinned gates, `--features jagua-experimental` | **ALL_PASS** — 206.869/`8a7737381238fa4d`, 159.09233022733062/`fa01012af1d559ae`, 159.07876040364795/`e28fba007f8031d4`, 164.0375677990678/`49f094d7e59a9008` |
| four pinned gates, `--features jagua-experimental,overlap-ics` (compiled, unarmed) | **ALL_PASS**, same four |
| whole-document identity between the two builds | **true, all four gates** (`gatelib.VOLATILE` stripped) |
| suite 1 `jagua-experimental` | exit **0** |
| suite 2 the full combo | exit **0** |
| suite 3 `--example general_request_benchmark` | exit **0** |
| suite 4 `jagua-experimental,overlap-ics` (stacked) | exit **0** |
| suite 5 `overlap-ics` alone, `--lib --tests` (the Chinese wall as a build) | exit **0** |
| two-binary determinism (s0, s1, c175, triangle-20, K=8 `cutclose`) | **TWO_BINARY_IDENTICAL true**, and see caveat 4 |
| triangle-20 locked-`T` | **PASS**, published 70.74073 inside 70.742 |
| throughput | **PASS** on all four live clauses |

No suite needed the known-flaky `free_material_multi_eviction` rerun.

## 14. Reproduction

```bash
# the red/green state-machine vector, both halves  (~2 minutes)
cargo test -p polygon-nesting-core --release --features overlap-ics --lib \
  search::overlap_ics::tests::the_no_improvement_counter -- --nocapture   # exit 0
git apply docs/experiments/overlap-ics/cutclose-rerun/evidence/strike-red.patch
cargo test -p polygon-nesting-core --release --features overlap-ics --lib \
  search::overlap_ics::tests::the_no_improvement_counter -- --nocapture   # exit 101
git checkout crates/polygon-nesting-core/src/search/overlap_ics/mod.rs

# FAST, including the canary that licenses the wall  (~9 minutes)
bash docs/experiments/overlap-ics/drivers/fast.sh          # 13 stages, exit 0

# the frozen wall: nine seeds x 3/10/30 s + the fixed-work replay  (~10 minutes)
mkdir -p /var/lib/t3/tmp/overlapics/rerun
cp /var/lib/t3/tmp/overlapics/fast/cutclose-fast.json \
   /var/lib/t3/tmp/overlapics/rerun/            # the canary licence
ICS_OUT=/var/lib/t3/tmp/overlapics/rerun \
  python3 docs/experiments/overlap-ics/drivers/wall.py

# the interleaved AB/BA control  (~4 minutes; needs the combo binary)
cargo build --release --example general_request_benchmark --features \
  jagua-experimental,compression-schedule,parallel-compression-schedule,\
continuous-rotation,sparse-rotation,fast-contract-validator
ICS_OUT=/var/lib/t3/tmp/overlapics/rerun \
  python3 docs/experiments/overlap-ics/drivers/control.py \
  target/release/examples/general_request_benchmark 10.0

# HEAVY
ICS_OUT=/var/lib/t3/tmp/overlapics/rerun \
  python3 docs/experiments/overlap-ics/drivers/corpus_gate.py 10000
python3 docs/experiments/overlap-ics/drivers/gates.py base  <base-binary>  <dir>/base
python3 docs/experiments/overlap-ics/drivers/gates.py meas  <meas-binary>  <dir>/meas
python3 docs/experiments/overlap-ics/drivers/gatecompare.py <dir>/base <dir>/meas <out> base meas
ICS_OUT=/var/lib/t3/tmp/overlapics/rerun \
  python3 docs/experiments/overlap-ics/drivers/cells.py triangle throughput
bash docs/experiments/overlap-ics/drivers/run-suites.sh
python3 docs/experiments/overlap-ics/drivers/determinism.py <ics-a> <ics-b>

# the per-bite rows of any cell directory, without re-running anything
python3 docs/experiments/overlap-ics/drivers/bites.py <cells-dir> <out.json> <label>
```

**Do not pipe any of these into `tee` or `tail`.** Every exit status in this
round was read directly; a pipe reports the last stage's.

### Binaries

| what | features | sha256 | vs round 1 |
|---|---|---|---|
| `overlap_ics_benchmark` (every ICS cell, the wall, arm A) | `overlap-ics` | `b42c10afca031ce24fac4cb2a85a752462c6fffb1eee42956e523ee846376f03` | changed — this is the repair |
| `general_request_benchmark` (arm B, the control) | the combo | `b44eb7fd3da62c4b7562c9d386d81ce80e9578e808cf90689d73b47f598d0ea1` | **identical** |
| `general_request_benchmark` (gate, default) | `jagua-experimental` | `61befdc544b4135a929e8e6e38d281e337cfc394fa243de4d6ebb267c8955819` | **identical** |
| `general_request_benchmark` (gate, feature-compiled-unarmed) | `jagua-experimental,overlap-ics` | `faf72fd80807dee19ccb082721b447cb15ec51c2f32e23e3911a992e7d0b07fe` | changed — the module is compiled in |

Every one of the 27 wall documents carries `b42c10af…`.

### Evidence

`docs/experiments/overlap-ics/cutclose-rerun/evidence/`:
`wall.json` (all 27 cells with their **raw per-bite rows**, the verdict, the
fixed-work replay), `control-ab-ba.json`, `strike-red.log`, `strike-red.patch`,
`round1-bites-red.json` (round 1's raw per-bite rows, the red trajectory),
`cutclose-fast.json`, `corpus-1000.json`, `corpus-10000.json`, `smoke.json`,
`gates-default-build.json`, `gates-feature-compiled-unarmed.json`,
`gates-both-builds.json`, `determinism-two-binary.json`, `throughput.json`,
`triangle20.json`, `floor-cells.json`, `suites.txt`, `fast-tier-stdout.txt`.

Every per-bite claim in this document is reconstructible from `wall.json` and
`round1-bites-red.json` alone. That was Sol review 18's second non-gating risk
and it is discharged.
