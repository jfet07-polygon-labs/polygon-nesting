# Wave 4, amended — the gate text, before any number

This file was written **before the first gate cell ran**, and its first section
is a byte-for-byte copy of the spec's §0. `section0.py` re-extracts those
thirteen lines from `docs/economics-round-spec.md` by their heading and requires
the block quote below to match them exactly; the exit status is the verdict.

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
`search/overlap_ics/` is byte-for-byte unchanged, and `armgate.py` re-measures
that against the round's base binary rather than asserting it from the diff.
