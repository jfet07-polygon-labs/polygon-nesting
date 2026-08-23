# `CutCloseRelocate`, round 1 — the frozen wall

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
