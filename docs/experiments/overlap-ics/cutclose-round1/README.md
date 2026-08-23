# `CutCloseRelocate`, round 1 — the frozen wall

> **SUPERSEDED, in one row and one claim, by
> [`../cutclose-rerun/`](../cutclose-rerun/README.md).** Sol review 18 §P0 and
> Grok review 13 flag 3 both found a line-level defect in the member this round
> measured: `Engine::separate` reset its no-improvement counter on any new raw-Φ
> minimum where the frozen spec requires 2 %. **§13's "separator strikes" row —
> *"identical / none"* — was therefore wrong**, and the test this round cited as
> covering it (`the_strike_caps_are_the_published_…`) checked only literals.
> Both are corrected in the rerun, which re-measures this identical gate on the
> repaired binary and lands on 2 of 9. The numbers below are left exactly as they
> were measured; nothing here has been edited to agree with the later round, and
> §0 is untouched by construction.

The round the quorum spec funds: Sparrow-faithful relocate plus a 0.1 %
split-and-close homotopy, on our Φ, our sampler and our dual-valid judge,
against a pre-committed 168.484 at 10 seconds.

This document is written in two halves, at two different times, on purpose.

**§0 is the pre-committed reading.** It is
[`docs/cutclose-relocate-spec.md`](../../../cutclose-relocate-spec.md) §"The
gate" copied verbatim, together with the two source texts that section
arbitrates between, and it was committed to this file **before any wall number
existed** — commit *"The reading this round is bound by, before a single wall
second is spent"*. Nothing in it may be edited after a number arrives. Only a
result section may be appended.

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

---

*Everything below this line was written after the numbers arrived.*
---

# Part I — the verdict

## 1. The gate

**FAIL.** 0 of 9 seeds published a strict non-constructor child at or below
168.484 mm within 10.000 seconds. The quorum §0.1 requires is 3.

| clause | required | measured |
|---|---|---|
| seeds ≤168.484 mm at 10.000 s | ≥3 of 9 | **0 of 9** |
| every publication dual-valid, every seed, every budget | 0 invalid | **0 invalid** of 1,269 |
| all nine seeds produced a valid run | 9 of 9 | 9 of 9 |
| first 0.1 % bite publishes | yes | **yes**, at exactly 0.999 × D\* |

The best 10-second depth of the round is **169.00246 mm** (seed 6), 0.518 mm
above the bar. The second is 169.21860 mm (seed 3). Seven of the nine sit on
a plateau at 179.07–179.08.

## 2. The full 3/10/30 curve, non-interpolated, all nine seeds

Best **strict non-constructor dual-valid** child published at or before each
budget. Separate processes, separate budget-response cells — not one 30 s
trajectory sampled three times. `bites` is explore bites published.

| seed | 3.000 s | bites | 10.000 s | bites | 30.000 s | bites | ≤168.484 at 10 s |
|---:|---:|---:|---:|---:|---:|---:|:--:|
| 0 | 179.00812 | 21 | 179.07686 | 21 | 164.00236 | 107 | **no** |
| 1 | 179.52945 | 19 | 179.08099 | 21 | 179.08099 | 21 | **no** |
| 2 | 179.00617 | 20 | 179.07957 | 21 | 168.66303 | 65 | **no** |
| 3 | 179.05645 | 21 | 169.21860 | 72 | 164.00577 | 109 | **no** |
| 4 | 179.05867 | 21 | 179.08123 | 21 | 165.05518 | 101 | **no** |
| 5 | 179.07174 | 21 | 179.07170 | 21 | 163.69242 | 109 | **no** |
| 6 | 179.05431 | 21 | 169.00246 | 74 | 164.00972 | 105 | **no** |
| 7 | 179.00593 | 21 | 179.08211 | 21 | 179.08210 | 21 | **no** |
| 8 | 179.08215 | 21 | 179.08211 | 21 | 179.08210 | 21 | **no** |
| **≤168.484** | | | **0 / 9** | | **5 / 9** | | |

The 3 s and 30 s columns **cannot pass or fail this gate** (§0.1). They are
reported in full because §0 requires it, and because the 30 s column is the
evidence for §5's diagnosis.

## 3. The interleaved AB/BA wall-arm control — diagnostic only

Nine pairs at 10.000 s, separate processes, AB on even seeds and BA on odd
ones. **It cannot raise or lower 168.484** (§0.1). Arm B is the campaign's
published wall arm on its own pinned positional tail, `wall=10000,v3=1`.

| seed | order | A `CutCloseRelocate` | B old wall arm | A − B |
|---:|:--:|---:|---:|---:|
| 0 | AB | 179.07609 | 168.48360 | +10.592 |
| 1 | BA | 179.08099 | 165.65578 | +13.425 |
| 2 | AB | 179.07958 | 174.28000 | +4.800 |
| 3 | BA | 169.18995 | 169.58500 | -0.395 |
| 4 | AB | 179.08123 | 172.12900 | +6.952 |
| 5 | BA | 179.07171 | 179.63300 | -0.561 |
| 6 | AB | 169.65550 | 168.46800 | +1.188 |
| 7 | BA | 179.08210 | 169.22900 | +9.853 |
| 8 | AB | 179.08211 | 169.03159 | +10.051 |
| **median** | | **179.07958** | **169.22900** | |

**The box is healthy and the bar is not stale.** Arm B on seed 0 returns
**168.48360** — the published 168.484 to five decimal places. What the
control also shows is why Grok review 12 Round 1 §4.2 refused to make it a
clause: arm B's nine seeds span **13.977 mm** (165.65578 to 179.63300) and its
median drifts **+0.745 mm** from its own published number. 168.484 was one draw
from a highly variable arm. It is still the bar, absolutely, and this round
does not get to relitigate it — but a reader deserves both facts.

`CutCloseRelocate` beats the old wall arm on 2 of 9 seeds (3 and 5) and loses
on 7. Every arm-B run was dual-gate valid; every arm-A run published nothing
invalid.
---

# Part II — the regression floor

## 4. Every clause of §0.1's floor, measured

| clause | required | measured | verdict |
|---|---|---|---|
| **S0 bit-for-bit** | 61 placements, 150.16451, `phi.to_bits() == 0`, `two_r = 5000`, dual-valid, 0 repair rows, giveback 0.0 | all of them, and the whole document is **byte-identical to a build of `b2aae68`** once the additive `quota` block is stripped | PASS |
| **S0 two-process** | stripped documents identical | identical | PASS |
| **S1 locked-`T` relocate regression** | republish inside 150.16547, repair ≤ 16 µm, giveback ≤ 0.050 mm, quota in relocate-evals | republished at **150.16547**, repair **7.968 µm**, giveback **0.000506 mm**, 83,594 relocate-evals of a 200,000 cap, two-process bit-identical | PASS |
| **triangle-20 locked-`T` relocate regression** | publish inside 70.742, same caps | published inside the lock, byte-identical to the pre-wave build modulo `quota`, 4,080 relocate-evals of the same cap | PASS |
| **1,000-state soundness** (FAST) | 0 outside the 4 µm band, 0 containment false-feasible, 0 incremental mismatch | 0 / 0 / 0 | PASS |
| **10,000-state soundness** (HEAVY) | same three zeros, force ≥95 % active and ≥80 % total on the `compressed` family | 0 / 0 / 0; force **100.0 %** and **100.0 %** on 5,001 scored steps; `worstBandMicron` 0 | PASS |
| **four pinned engine gates, default build** (`jagua-experimental`) | 206.869/`8a7737381238fa4d`, 159.09233022733062/`fa01012af1d559ae`, 159.07876040364795/`e28fba007f8031d4`, 164.0375677990678/`49f094d7e59a9008` | all four hit | PASS |
| **four pinned engine gates, feature-compiled-unarmed** (`jagua-experimental,overlap-ics`) | same four | all four hit | PASS |
| **whole-document identity between the two builds** | identical with `gatelib.VOLATILE` stripped | **identical, all four** | PASS |
| **default-build isolation** | `--no-default-features --lib` compiles | compiles | PASS |
| **`jagua-rs` absent** | absent from `cargo tree --features overlap-ics` | absent | PASS |
| **`Xoshiro` / `rand::` / `jagua` absent from `search/overlap_ics/`** | absent outside line comments | absent | PASS |
| **`fast.sh` has no stale worktree default** | resolves from the script | resolves from the script; `run-suites.sh` had the same stale default and is repaired here | PASS |
| **two-binary determinism** | s0, s1, c175, triangle-20 and the K=8 `cutclose` cell identical across two independent builds | identical, all five | PASS |

The floor is **green in every clause**. Nothing in this round's FAIL is a
regression.

## 5. The relocate metric version, and what happened to the retired pin

Arbitration 4 says the committed thresholds stay literal and the relocate
economics get **new names**. Both halves are honoured, and the second half has a
number that must not be quietly buried:

| pin | unit | threshold | measured | clause of `pass` |
|---|---|---|---|---|
| cold Φ | µs | ≤ 200 | **31.975** | yes, literal |
| moved-piece row rebuild | µs | ≤ 20 | **1.118** | yes, literal |
| convex cell-gap evaluations | /s | ≥ 1 M | **7,438,834** | yes, literal |
| **the retired proposal pin** | `pieceProposals`/8 s | ≥ 100 K | **61,446** — below, and expected to be | **no longer a clause** |
| **the re-denominated pin** | `relocateEvals`/8 s | ≥ 100 K | **5,327,414** (53x) | yes |

One `pieceProposal` used to buy a gradient and a backtracking ladder. It now
buys a whole relocate — 75 pool samples and four coordinate descents, measured
at **246–261 sample evaluations per relocate** across every cell in this round.
The two numbers are not the same currency and never were; the driver prints the
old one under its old name with `retiredProposalPinNote` beside it, and scores
`pass` on the new one.

The member's actual throughput on this box, measured across the nine 10-second
wall cells: **181,291,747 sample evaluations in 69.1 seconds of search =
2.63 million relocate-evals per second**, or 21 million projected into eight
seconds — 210× the re-denominated pin. **Throughput of the *operator* is not
what this round failed on.**

---

# Part III — the funnel autopsy

This is the one read-only autopsy §0.1's failure license grants. It reads the
row §0.5 pins and nothing else.

## 6. `bitesStarted → proxyBandReached → exactAttempted → dualValidPublished`

Summed over nine seeds at each budget:

| budget | bitesStarted | proxyBandReached | exactAttempted | dualValidPublished | ≤168.484 |
|---|---:|---:|---:|---:|---:|
| 3.000 s | 222 | 209 | 209 | 204 | **0 / 9** |
| 10.000 s | 350 | 343 | 343 | 328 | **0 / 9** |
| 30.000 s | 819 | 805 | 805 | 737 | **5 / 9** |

**The funnel does not leak.** 98 % of bites reach the 4 µm band, 100 % of those
attempt an exact publication, and 94–96 % of those publish dual-valid. Nothing
is lost between "the separator got there" and "the judge accepted it". The
member is not being strangled by its own publication contract, which is the
first thing a FAIL of this shape would be blamed on.

What the funnel **does** say is that the loop simply does not take enough bites
inside ten seconds.

## 7. Where the ten seconds go, and where the trajectory stops

Seven of nine seeds publish exactly **21 explore bites** and then stop. The
22nd bite, at `W ≈ 178.99`, is the wall.

Seed 8, at 10 s — the complete bite record:

| bites | master iterations each | outcome |
|---|---|---|
| 1 – 21 | 1 – 14 | all publish, total ≈ 0.3 s |
| **22** (`W = 178.99252`) | **922** | never publishes; 1 strike; 53 exact attempts; `min raw Φ = 6.35e-5` |
| 23 (compress, `W = 179.08211`) | 3 | publishes |
| 24 (compress, `W = 178.99319`) | 219 | never publishes |

The same seed at **30 s** spends **3,825** master iterations on bite 22, takes
4 strikes and one disruption, and still does not publish. Seeds 1 and 7 behave
identically. The 3 s, 10 s and 30 s answers for seed 8 agree to seven
significant figures — 179.08215 / 179.08211 / 179.08210 — because after bite 21
the extra 27 seconds buy nothing at all.

The other six seeds cross bite 22 and then **cascade**: seed 5 reaches 109
explore bites and 163.69242 at 30 s. Crossing the wall is worth ~15 mm; failing
to cross it is worth 0.

That is a **basin barrier at one width**, not a slow grind. The member either
gets through `W ≈ 178.99` or spends its whole budget there.

## 8. Why the 22nd bite refuses

53 exact attempts at bite 22 produce **zero** checkpoint rows, because
`publish::attempt` returns before recording one:

```rust
if proxy_depth > state.target_depth_mm { return None; }
```

The state reaches `max_g ≤ 4 µm` — all rows within the band — while its **raw
depth is still a few micrometres above the target strip**. The band is a
statement about rows; the target is a statement about depth; a layout can
satisfy the first and miss the second by less than the band.

This is **pre-existing publication behaviour**, not this round's, and it is not
a defect to repair here. §6.2 lists `publish.rs` among what survives untouched,
and §0.1's forbidden rescues name "widening the publication band, repair cap, or
giveback cap". It is recorded because it is the mechanical reason the 53
attempts are invisible in `exactCheckpoints`, and because a future round that
wants to look here should know the funnel row `exactAttempted` counts attempts
the publisher declined to score.

---

# Part IV — the defect hunt

## 9. What §0.1's failure license actually asks

> It licenses **one** rerun only for a **line-level violation of this frozen
> specification** accompanied by a red/green minimal vector.

So the question is not "could this be faster". It is "does the shipped member
differ from the frozen specification on a line". Both consultants pre-named
their favourites. Both were checked, on this round's own evidence, and **both
are closed**.

## 10. Pre-named defect #1 — the neutered relocate: **not present**

Grok review 12 Round 2 §6.4: *"the 50 container-wide samples run, then a
leftover incident-strict-decrease (or a leftover PGS ladder, or a 'max step =
`ladder_top`') rejects every sample that leaves the neighbourhood."*

| test | result |
|---|---|
| unit vector `a_relocate_commits_a_container_pose_far_beyond_the_old_ladder_top` | green |
| container samples per relocate, on the shipped binary | **exactly 50.0** |
| focused samples per relocate | **exactly 25.0** |
| `containerCommits` across the nine 10 s cells | **1,762** — and it equals `containerWinners` exactly, so **every** container winner moved its piece |
| relocates that moved the piece at all | 268,117 of 732,895 = **36.6 %** |
| source read for a leftover filter | `relocate.rs` has no `after < before`, no step cap, no maximum displacement, no exact predicate |

**`stayPutWinners` is 98.0 %** of relocates, and that number will look alarming
to anyone who has read the pre-named defect. It is not the defect, and the two
counters that separate them are in the table above: the operator's *seed* is the
current pose 98 % of the time — because on a nearly feasible layout 75 uniform
draws almost never beat a locally refined incumbent — while the operator's
*output* is a different pose 36.6 % of the time, produced by the two-stage
coordinate descent that is part of the move. A neutered relocate would show
`containerCommits == 0`; this one shows 1,762 out of 1,762 container winners
committing.

## 11. Pre-named defect #2 — exact-parent drift: **not present**

Sol review 17 Round 2 §2's mandatory addition 1 asks for a forced-nonzero-repair
two-bite vector. This round has 27 wall cells' worth:

| test | result |
|---|---|
| unit vector `a_repaired_publication_becomes_the_next_bites_exact_parent` | green |
| publications checked across all 27 wall cells | **1,269** |
| of those, publications carrying **nonzero repair** | **329** |
| links where `parentFingerprint` ≠ the previous publication's `placementFingerprint` | **0** |
| links where an explore bite's target ≠ `0.999 ×` the previous **published raw depth** | **0** |
| publications deeper than their own target | **0** |

The chain is unbroken through 329 repaired links. `D` comes from
`Publication.raw_source_depth_mm` and never from `T` or a pre-repair proxy
depth. The old `mod.rs:295` defect is closed and stays closed under repair.

## 12. The rest of the frozen member, clause by clause

| spec clause | measured on this round's evidence |
|---|---|
| 25 focused + 50 container samples | exactly 25.0 / 50.0 per relocate |
| current pose always in the pool | `stayPutWinners` nonzero and dominant — it is in the pool by construction |
| 16 sampled orientations, continuous CD wiggle | unit vectors green; `a_frozen_piece_keeps_its_angle_through_the_whole_member` green |
| 3 unique finalists (0.05·min_dim, 1°) | `the_finalist_pool_holds_three_poses_no_two_of_which_are_the_same_sample` green |
| two-stage axis CD, accept-equal | `the_coordinate_descent_crosses_a_plateau_of_equal_evaluations` green |
| GLS on all rows every master iteration | `every_sweep_runs_exactly_one_weight_pass_and_a_worker_sweep_runs_none` green; `weightUpdates` = master iterations |
| GLS multipliers 1.2 + 0.8·v/v_max, decay 0.95, floor 1, cap 2²⁰ | `the_gls_schedule_is_the_published_one_and_the_only_one` green |
| eight competitive workers, barrier, min weighted Φ, ordinal tie | 9/9 master iterations contested with **four distinct winning ordinals** in the merge vector; two processes bit-identical |
| disruption on the explore fail path, interior witness, follower cap | 25 disruptions / 75 follower moves across the nine 10 s cells; `every_fixture_piece_has_an_interior_witness_inside_its_own_material` green |
| `W ← W(1−0.001)`, centre cut, far side `t_y` only | measured `deltaMm` exact, `splitYMm == W/2` exact, `step == 0.001` on every explore bite |
| shrink advances **only** on a dual-valid publication at the new `W` | 0 widths left behind without a publication, all 27 cells |
| never grow `W`, never restore-to-skip | `W` monotone non-increasing in every cell |
| Φ = 0 refused ⇒ failed separation | `a_refused_publication_never_advances_the_width` green |
| compress from the installed exact parent, uniform cut, TimeBased 0.0005→0.00001 | `the_time_based_step_interpolates_against_a_fake_elapsed` and `the_compress_cut_and_the_pool_rank_are_functions_of_their_keys` green |
| 80/20 of post-constructor wall | measured explore 6.140 s of 7.674 s search on a 10 s cell = **80.0 %** |
| clock read only at worker-sweep barriers | **max deadline overrun +6.6 ms across 27 cells** (the pivot round measured 2.223 s on a 2 s clause) |
| strike caps 200/3 and 100/5, 2 % improving reset | `the_strike_caps_are_the_published_two_hundred_three_and_one_hundred_five` green |

**No line-level violation was found.** §0.1's rerun licence is therefore not
granted, and this document does not ask for one.
---

# Part V — provenance

## 13. The provenance table

Required by `docs/cutclose-relocate-spec.md` line 14 — *"A provenance table
(concept → paper algorithm → source-confirmed default → our difference) is part
of the spec commit"* — and by Sol review 17 Round 2 §3: *"Call it
'paper-derived relocate/GLS homotopy,' not 'a Sparrow port.'"* It was not
committed by the spec wave or the core wave, so it is committed here.

**Every "source-confirmed default" below was read at Sparrow rev `14f4868f` and
re-checked against that source while writing this table.** No source text is
copied anywhere in this tree; the citations exist so a reader can check the
semantics.

| concept | paper algorithm | source-confirmed default (`14f4868f`) | our implementation | our difference, deliberate |
|---|---|---|---|---|
| **the field** | Alg. 3–4, pole-based overlap proxy | `poly_simpl_tolerance: Some(0.001)`, `f32`, quadtree CDE with pole surrogates (`config.rs:100`, `cde_config`) | source-ring signed-gap Φ in `f64`, `energy.rs` | **no simplification, no poles, no `jagua-rs`, no `f32`.** Ours is an exact source-ring measure; theirs is a proxy. The gap is charged against us, not for us: a proxy is cheaper. |
| **routine move** | Alg. 5–6, `search_placement` | `n_focussed_samples: 25`, `n_container_samples: 50`, `n_coord_descents: 3` (`config.rs:67-69`) | `RelocateConfig` 25 / 50 / 3, `relocate.rs` | none in the counts. The **coordinate** differs: their uniform sampler bounds a *transformation translation*; we bound a *centroid*, because our strip box already carries the campaign's clearance split. |
| **sample order** | `eval/sample_eval.rs`, `Clear < Collision{loss}` | payload-free `Clear` variant | `SampleEval` / `eval_cmp`, `relocate.rs:204` | none. Two clear samples compare **equal**, as theirs do. |
| **CD acceptance** | `sample/coord_descent.rs::tell` | `if !worse { pos = candidate }` | `cd_accepts`, `relocate.rs:222` | none. Accept-equal, which is the half the previous round's `after < before` had deleted. |
| **CD schedule** | `sample/coord_descent.rs` | `CD_STEP_SUCCESS 1.1`, `CD_STEP_FAIL 0.5`, `PRE_REFINE_CD_TL_RATIOS (0.25, 0.02)`, `PRE_REFINE_CD_R_STEPS (5°, 1°)`, `SND_REFINE_CD_TL_RATIOS (0.01, 0.001)`, `SND_REFINE_CD_R_STEPS (0.5°, 0.05°)` (`consts.rs:11-26`) | `CoordDescentStage` coarse/fine, identical numbers | none. Rotation steps are stored in degrees rather than radians. |
| **CD axes** | `CDAxis::random` | uniform over `0..6` with wiggle, `0..4` without | `draw_axis`, `relocate.rs:547` | none in the distribution; the draw is `counter_hash`, not `Xoshiro`. |
| **rotation seeds** | `sample/uniform_sampler.rs` | `ROT_N_SAMPLES: usize = 16` (`uniform_sampler.rs:13`) | 16 equally spaced absolute orientations | none. |
| **finalist uniqueness** | `sample/best_samples.rs` | similar ⇒ accept only if better than **all** similar, then evict them | `BestSamples::report`, `relocate.rs:336` | angles are normalised into `[0, 360)` first. Their `r % 2π` inherits a wrap-around blind spot from the sign of the remainder; ours calls 359.5° and 0.5° close, as they are. |
| **early exit on the pool bound** | `best_samples.upper_bound()` handed to the evaluator | present | **absent** | our evaluation is one incremental row rebuild and is already the cheapest form of itself. Costs work, buys an ordering that does not depend on how far a partial evaluation got. |
| **`Invalid` verdict** | evaluator may return `Invalid` for an out-of-container pose | present | **absent** | our four boundary rows are part of Φ, so a pose hanging out of the strip is a scored collision. Only a non-finite pose is refused. |
| **sweep** | Alg. 7, `optimizer/worker.rs::move_items` | colliding set (`ct.get_loss(pk) > 0.0`), shuffled, Gauss–Seidel, re-checked at each turn | `Descent::gauss_seidel`, `descent.rs:399` | permutation is a Fisher–Yates over `counter_hash(seed, bite, iteration, worker)`, not `Xoshiro`. |
| **GLS** | Alg. 8 | `GLS_WEIGHT_MIN_INC_RATIO 1.2`, `GLS_WEIGHT_MAX_INC_RATIO 2.0`, `GLS_WEIGHT_DECAY 0.95` (`consts.rs:4-6`) | `energy::gls_update`, same three constants in `f64` | cap `2²⁰` and floor `1.0` are ours (theirs are implicit in `f32`). All rows every master iteration; **the stall-only integer increment of the previous round is deleted**. |
| **tournament** | Alg. 10 | `n_workers: 3` (`config.rs:66`, `config.rs:83`) — the published 150.165 mixed-61 log used `--workers 8` | **8**, `Engine::tournament` | **ours is 8, the source's compiled default is 3.** Not a difference from the *run* the bar is a test of; it is a difference from the struct literal, and it is named here rather than left for someone to find. Arbitrated: Sol review 17 Round 2 refuses to sign a one-worker version. |
| **tie break** | Alg. 10 merge | minimum weighted loss | `(weighted, worker ordinal)` | Grok M6 asked for `(weighted, fingerprint, ordinal)`. A digest between the two would only reorder exact ties by a hash and costs a fingerprint per worker per iteration. Stated at the call site. |
| **separator strikes** | Alg. 9 | `iter_no_imprv_limit: 200 / 100`, `strike_limit: 3 / 5` (`config.rs:63-64`, `config.rs:80-81`) | `SeparateLimits::EXPLORE` / `::COMPRESS`, identical | ~~none. The 2 % improving-strike reset (`min_loss < 0.98 * initial_strike_loss`) is `STRIKE_IMPROVEMENT_RATIO`.~~ **WRONG, and corrected in the rerun.** The *outer* 0.98 on the strike count shipped; the *inner* one — `separator.rs:106-108`, `loss < min_loss * 0.98` gating `n_iter_no_improvement` — did not. Round 1 reset the 200-counter on any new minimum. See [`../cutclose-rerun/README.md`](../cutclose-rerun/README.md) §4–§5. |
| **explore shrink** | Alg. 11–12 | `shrink_step: 0.001`, `split_position: None` ⇒ centre (`config.rs:58`) | `EXPLORE_SHRINK_STEP`, `centre_cut_mm` | **success is stricter than theirs.** Their bite advances on a proxy-feasible separation; ours advances **only on a dual-valid publication at the new `W`**. Deliberate, and it costs us. |
| **split-and-close** | `separator.rs::change_strip_width` | far side translates by `δ` along the strip axis | `homotopy::split_and_close` | their strip *width* is our long-axis *depth*; their split is an `x`, ours a `y`; `t_x`, `θ` and the mirror are copied through bit for bit. |
| **explore failure pool** | Alg. 12 | `solution_pool_distribution_stddev: 0.25` (`config.rs:61`), `max_conseq_failed_attempts: None` | `normal_biased_rank`, `Normal(0, 0.25)` by Box–Muller on two `counter_hash` uniforms | no `rand_distr`, no `Xoshiro`. Pool capacity **64** is ours, a memory guard keeping the best entries; no run inside a 30 s wall reaches it. **The restore puts back the pooled entry's own weights**, not the tracker's current ones — Sol review 17 Round 2 §5's reading of the explore-fail path. The *rollback inside* a separation keeps the current weights, which is theirs. |
| **disruption** | Alg. 12 fail path | `large_item_ch_area_cutoff_percentile: 0.75` (`config.rs:73`), 1 % area/diameter distinctness | `disrupt::disrupt`, same cutoff and test | followers are found by a **guaranteed interior witness** (centroid of the first positive-area ear-clipped cell) rather than their POI. Arbitration 1: an area centroid can lie outside nonconvex material. Follower cap `n`. |
| **compress shrink** | Alg. 13 | `shrink_range: (0.0005, 0.00001)`, `ShrinkDecayStrategy::TimeBased` (`config.rs:76-78`) | `COMPRESS_SHRINK_RANGE`, `time_based_step` | the function is pure; the clock is the caller's, read once between bites. A fixed-work replay hands it a bite ordinal over a bite quota — the same monotone `[0,1]` with the wall removed. |
| **time split** | §4 of the paper | `DEFAULT_EXPLORE_TIME_RATIO 0.8`, `DEFAULT_COMPRESS_TIME_RATIO 0.2` (`consts.rs:31-32`) | `EXPLORE_TIME_RATIO`, measured at 80.0 % | **their `--global-time` starts after import and LBF; ours starts at the decoded bare request.** Documented, not compensated (arbitration 3). On mixed-61 that is 2.31–2.36 s of constructor charged against the budget — 23.3 % of a 10 s cell and **77.6 % of a 3 s cell**. |
| **RNG** | `Xoshiro256PlusPlus` | seeded per run | `counter_hash` / `rotated_halton` keyed by `(seed, bite, iteration, worker, piece, ordinal)` | **a trajectory is a function of the key**, so two processes with eight OS threads each agree bit for bit. Verified across two processes in this round's `merge` stage. |
| **publication** | none — Sparrow's feasible solution *is* the answer | — | `publish.rs`: 4 µm band, repair ≤ 4n rows and ≤ 16 µm/piece, Exclusive `r = 2.500` allowance 0, untouched contract validator | **entirely ours.** Sparrow has no exact re-validation step; every depth we report has passed two independent authorities. |

## 14. What this member is, in one sentence

A **paper-derived relocate/GLS homotopy** — Algorithms 4–13 of arXiv:2509.13329,
implemented on our state types, our deterministic counter sampler, our
source-ring signed-gap Φ, our exact round kernel and our untouched contract
validator — and **not** a Sparrow port. No `jagua-rs`, no `Xoshiro`, no `rand::`,
no pole proxy, no polygon simplification, no copied source text.
---

# Part VI — the verdict, applied

## 15. Which row of §0.3's table this round lands on

§0.3 pre-committed five rows. This round matches exactly one of them.

* Not row 1 — the quorum is 0, not ≥3.
* Not row 2 — there is no invalid publication anywhere: **0 of 1,269**.
* Not row 4 — §10 through §12 hunted both pre-named defects and every clause of
  the frozen member and found **no line-level violation**. No rerun licence is
  claimed.
* Not row 5 — the regression floor is green in every clause (§4).

**Row 3.** *"<3/9, floor green, member and regime as specified → **FAIL**; one
funnel autopsy; the member **closes**."*

## 16. The diagnosis, and why it does not license a follow-up

§0.3 also pre-committed the one exception, and this round satisfies its
antecedent exactly:

> if ≥3/9 produce **strict dual-valid children** and the 10 s depths sit in a
> tight band **above** 168.484 … and the first 0.1 % bite **does** publish, that
> is **throughput, not basin**.

All three hold. 9 of 9 seeds publish strict dual-valid children. The nine
10-second depths run 169.00246 to 179.08211 — the constructor's 182.976 minus
21 to 74 bites of 0.1 %, which is the band the clause describes. The first
0.1 % bite publishes on every seed, at exactly `0.999 × D*`.

And the 30-second column is decisive about which of the two it is: **5 of 9
seeds go below the bar at 30 s**, reaching 163.69242 to 165.05518. The basin
exists, the member reaches it, and what it lacks is wall.

But §0.3 also pre-committed the amendment that decides what that buys:

> **Because workers=8 are already present, it does not license a scaling
> follow-up.** — Sol review 17 Round 2 §6

Eight workers are seated. The exception's remedy — *"one follow-up that only
raises separator workers"* — has already been spent before the round began, by
the arbitration that refused to test a one-worker version. The diagnosis is
therefore **recorded, not cashed**.

Two further things are worth saying plainly so that nobody has to rediscover
them:

1. **The constructor is not the story.** It costs 2.31–2.36 s of the 10.000 s,
   and arbitration 3 charges it rather than capping it. Returning all of it
   would give the loop 10.0 s of search instead of 7.67 s. The two seeds that
   *did* cross the barrier inside 7.67 s still only reached 169.00 and 169.22;
   the seeds that reached the bar needed **~27 s of search and ~105 explore
   bites**. A 30 % throughput gift does not close a 3× gap.
2. **Operator throughput is not the story either.** 2.63 million relocate-evals
   per second, 210× the re-denominated pin. What costs the round is that seven
   of nine trajectories spend their entire budget on a single width they cannot
   separate — a **basin barrier at `W ≈ 178.99`**, not a slow operator.

## 17. The kill, written as §0.3 requires

> **Sparrow-faithful relocate + 0.1 % split-and-close, on our Φ and our
> dual-valid judge, did not beat 168.484 at 10 s on 3 of 9 seeds.**
>
> It beat it on 5 of 9 at 30 s. The member is sound, the floor is green, no
> line-level violation of the frozen specification was found, and no rescue is
> claimed. Under the pre-committed reading — and specifically under Sol review
> 17 Round 2 §6, which removes the scaling follow-up because eight workers were
> already seated — **the `CutCloseRelocate` member closes.**
>
> Joint projection, component-Y, a different sampler and a different homotopy
> are separately funded proposals. Nothing in this document argues for one.

---

# Part VII — honest caveats

1. **One box, one session, one binary.** Every number here was taken on a
   16-core x86_64 machine at load 0.6–2.1, with the tree clean and the binary's
   SHA-256 recorded in every document. Nothing was rerun and nothing was
   selected by outcome. The 27 wall cells ran once each, in seed order, in one
   pass.
2. **The two "independent" determinism binaries are byte-identical.** Building
   `overlap_ics_benchmark` into two different `CARGO_TARGET_DIR`s produced the
   same SHA-256 (`6f102a04…`). That is a *stronger* result than the cell was
   designed to show — the release build is reproducible on this toolchain — but
   it means this round's two-binary cell did not actually vary the binary. The
   two-**process** comparisons (smoke, K=8 bites, 8-worker merge, wall replay)
   are the ones carrying the determinism claim.
3. **`stayPutWinners` is 98.0 %.** §10 explains why that is not the pre-named
   defect and gives the two counters that separate them, but it is the single
   number in this evidence most likely to be misread, and a future round looking
   at the sampler should start there rather than at the commit filter.
4. **`repairMaxDisplacementMm` touched 16.000 µm** — exactly the
   `4 × epsilon_grid` cap — on at least one publication among the 1,269. It is
   at the cap, not over it, and the cap was not widened. Worth knowing that the
   repair machinery is working at its limit on some bites.
5. **`exactAttempted` in the funnel counts attempts the publisher declined to
   score.** §8 documents the mechanism. The funnel row is still the one §0.5
   pins, and it is reported as pinned, but a reader comparing `exactAttempts`
   against `exactCheckpoints.length` will find them different and should read §8
   before concluding anything.
6. **The 30 s cells include a compress phase whose share of a longer wall is
   larger in absolute terms.** Seed 2 took 62 compress bites at 30 s. The
   3/10/30 cells are separate budget-response runs, as §0.4 clause 5 requires,
   so their phase splits differ by construction and the columns are not a
   single trajectory sampled three times.
7. **The control's arm B is a lottery.** 13.977 mm of spread across nine seeds.
   Its agreement with 168.484 on seed 0 is evidence the box is healthy; its
   median is 0.745 mm away from that number. Neither fact moves the bar, and
   this document does not use either to argue about it.
8. **`disrupt` fires 25 times across the nine 10-second cells and 0 times in
   some of them.** The explore fail path is only reached when a separation
   *ends*, and at 10 s most bite-22 separations end on the deadline. The
   mechanism is exercised (75 follower moves) but it is not on the hot path at
   this budget.
9. **A compress cut can land above all material** (`movedPieces: 0`). Observed.
   Faithful to their uniform split, which is allowed to move everything or
   nothing.
10. **Sparrow's timer excludes import and LBF; ours does not.** Documented, not
    compensated (arbitration 3). On a 3-second cell the constructor is 77.6 % of
    the budget, which is why the 3 s column is close to constructor-only for
    every seed — exactly what Grok review 12 Round 2 §6.6 said to expect.
---

# Part VIII — the round boundary

## 18. HEAVY, in full

| battery | result |
|---|---|
| 10,000-state contact corpus | **PASS** — 0 outside the 4 µm band, 0 containment false-feasible, 0 incremental mismatch, `worstBandMicron` 0, force 100.0 % / 100.0 % on 5,001 scored `compressed` steps, 20.6 s |
| four pinned gates, `--features jagua-experimental` | **ALL_PASS** — 206.869/`8a7737381238fa4d`, 159.09233022733062/`fa01012af1d559ae`, 159.07876040364795/`e28fba007f8031d4`, 164.0375677990678/`49f094d7e59a9008` |
| four pinned gates, `--features jagua-experimental,overlap-ics` (compiled, unarmed) | **ALL_PASS**, same four |
| whole-document identity between the two builds | **true, all four gates** (`gatelib.VOLATILE` stripped) |
| suite 1 `jagua-experimental` | exit **0** |
| suite 2 the full combo `jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator` | exit **0** |
| suite 3 `--example general_request_benchmark` | exit **0** |
| suite 4 `jagua-experimental,overlap-ics` (stacked) | exit **0** |
| suite 5 `overlap-ics` alone, `--lib --tests` (the Chinese wall as a build) | exit **0** |
| two-binary determinism (s0, s1, c175, triangle-20, K=8 `cutclose`) | **TWO_BINARY_IDENTICAL true** |

No suite needed the known-flaky `free_material_multi_eviction` rerun.

## 19. Throughput, measured on a quiet box

| metric | threshold | measured | clause of `pass` |
|---|---|---|---|
| cold Φ | ≤ 200 µs | **31.975 µs** | yes |
| moved-piece row rebuild | ≤ 20 µs | **1.118 µs** | yes |
| convex cell-gap evaluations | ≥ 1 M/s | **7,438,834/s** | yes |
| `projectedProposalsInEightSeconds` (retired unit) | — | **61,446** | **no** — see §5 |
| `projectedRelocateEvalsInEightSeconds` | ≥ 100 K | **5,327,414** | yes |

`rawPhiBeforeProposals` 565.376 → `rawPhiAfterProposals` 65.013, with 2,817
accepted relocates of 7,071 — the loop was doing the work the currency is
denominated in, not skipping.

The retired pin's 61,446 is exactly the number arbitration 4 exists for. One
`pieceProposal` now buys **245.2 sample evaluations**; 61,446 × 245.2 = 15.1
million relocate-evals in eight seconds, which is 151× the pin the old unit was
set at. Nothing got slower.

## 20. Reproduction

```bash
# FAST, including the canary that licenses the wall  (~9 minutes)
bash docs/experiments/overlap-ics/drivers/fast.sh          # 13 stages, exit 0

# the frozen wall: nine seeds x 3/10/30 s + the fixed-work replay  (~8 minutes)
mkdir -p /var/lib/t3/tmp/overlapics/round1
cp /var/lib/t3/tmp/overlapics/fast/cutclose-fast.json \
   /var/lib/t3/tmp/overlapics/round1/            # the canary licence
ICS_OUT=/var/lib/t3/tmp/overlapics/round1 \
  python3 docs/experiments/overlap-ics/drivers/wall.py

# the interleaved AB/BA control  (~4 minutes; needs the combo binary)
cargo build --release --example general_request_benchmark --features \
  jagua-experimental,compression-schedule,parallel-compression-schedule,\
continuous-rotation,sparse-rotation,fast-contract-validator
ICS_OUT=/var/lib/t3/tmp/overlapics/round1 \
  python3 docs/experiments/overlap-ics/drivers/control.py \
  target/release/examples/general_request_benchmark 10.0

# HEAVY
ICS_OUT=/var/lib/t3/tmp/overlapics/round1 \
  python3 docs/experiments/overlap-ics/drivers/corpus_gate.py 10000
python3 docs/experiments/overlap-ics/drivers/gates.py base  <base-binary>  <dir>/base
python3 docs/experiments/overlap-ics/drivers/gates.py meas  <meas-binary>  <dir>/meas
python3 docs/experiments/overlap-ics/drivers/gatecompare.py <dir>/base <dir>/meas <out> base meas
bash docs/experiments/overlap-ics/drivers/run-suites.sh
python3 docs/experiments/overlap-ics/drivers/determinism.py <ics-a> <ics-b>
ICS_OUT=... python3 docs/experiments/overlap-ics/drivers/cells.py throughput
```

**Do not pipe any of these into `tee` or `tail`.** Every exit status in this
round was read directly; a pipe reports the last stage's.

### Binaries

| what | features | sha256 |
|---|---|---|
| `overlap_ics_benchmark` (every ICS cell, the wall, arm A) | `overlap-ics` | `6f102a043820a2c45cd410700f48f1eb362d3eb2656964b0866f9a61cc343e68` |
| `general_request_benchmark` (arm B, the control) | the combo | `b44eb7fd3da62c4b7562c9d386d81ce80e9578e808cf90689d73b47f598d0ea1` |
| `general_request_benchmark` (gate, default) | `jagua-experimental` | `61befdc544b4135a929e8e6e38d281e337cfc394fa243de4d6ebb267c8955819` |
| `general_request_benchmark` (gate, feature-compiled-unarmed) | `jagua-experimental,overlap-ics` | `c87cfac4a364490ab3e63cb41d73107336768269a2f56d6a2a7ceeaacb82c371` |

Every wall document carries its own `executableSha256`; every one of the 27
matches the first row.

### Evidence

`docs/experiments/overlap-ics/cutclose-round1/evidence/`:
`wall.json` (all 27 cells, the verdict, the fixed-work replay),
`control-ab-ba.json`, `cutclose-fast.json`, `corpus-1000.json`,
`corpus-10000.json`, `smoke.json`, `gates-default-build.json`,
`gates-feature-compiled-unarmed.json`, `gates-both-builds.json`,
`determinism-two-binary.json`, `throughput.json`, `suites.txt`,
`fast-tier-stdout.txt`.

## 21. Determinism, as measured this round

| claim | how it was checked | result |
|---|---|---|
| two processes, fixed work, K=8 explore + 2 compress bites, 8 workers | stripped documents compared byte for byte | identical |
| two processes, eight-worker merge, per-iteration | winner ordinals, master state fingerprints (poses **and** weights), winner guided totals, exact parent chain — iteration by iteration | identical, 9 of 9 iterations, **all 9 contested**, 4 distinct winning ordinals |
| every wall publication ordinal replayed as fixed work, two processes | 9 seeds, `bites` = the last wall publication's bite ordinal | **all bit-identical** |
| two independently built binaries | s0, s1, c175, triangle-20, K=8 `cutclose` | identical |
| S0 two-process | stripped documents | identical |
| S1 two-process | stripped documents | identical |

The eight-worker tournament spawns eight OS threads per master iteration. It is
new concurrency in this tree, and the merge-determinism vector is what makes the
claim that completion order is unobservable a measurement rather than an
argument: two processes, eight threads each, agreeing on every winning ordinal
and every master fingerprint.

One incidental finding worth recording: **building `overlap_ics_benchmark` into
two different `CARGO_TARGET_DIR`s produced the same SHA-256.** The release build
is byte-reproducible on this toolchain. That makes the two-binary cell weaker
than it reads — it did not actually vary the binary this round — and the
two-*process* comparisons above are the ones carrying the claim.
