# The economics round

The spec of record is [`../../../economics-round-spec.md`](../../../economics-round-spec.md) —
the first three-model quorum of the campaign (Sol review 19 R3, Grok review 14
R3, ox-alpha review 1), gated on the evidence-integrity audit in
[`../evidence-audit/`](../evidence-audit/README.md), which has since landed.

It funds exactly three changes on the frozen member, and runs them in three
waves. This directory holds each wave's evidence.

| wave | what | where | status |
|---|---|---|---|
| 1 | the spec/profile census, no quality edit | [`census/`](census/README.md) | **done** |
| 2 | executor agent ∥ meter agent | [`meter/`](meter/evidence/currency.json) | **done** — executor refused by the census |
| 3 | one integration agent owns `mod.rs`/`Pacer`/schema | [`integration/`](integration/evidence/armgate.json) | **done** |
| 4 | evidence agent runs the drivers, edits no engine code | [`gate/`](gate/README.md) | **stopped at the currency reject rule** |

---

## §0 — the pre-committed gates

*Copied verbatim from [`../../../economics-round-spec.md`](../../../economics-round-spec.md)
§0, before this directory carried a single gate number. It is reproduced here
so the clauses can be read without the reader having to trust that the gate
text was not edited after the numbers arrived; `gate/section0.py` re-extracts
those thirteen lines from the spec file and requires this block to match them
byte for byte.*

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

**No number below or in any subdirectory answers any of those six clauses.**
Wave 4 stopped one clause earlier, at funded change 3's own reject rule, and
the reason is [`gate/README.md`](gate/README.md).

---

## What wave 1 settled

**The persistent executor is NOT built.** The spec's pre-committed gate is
"build iff prep+dispatch ≥ 10 % of hard-state wall"; measured on the frozen
eight workers at the 179 shelf over 200 master iterations, two processes, six
seeds, the largest reading is **5.082 %** (`census/evidence/spawntax.json`,
`verdict.observedShareMax`; the smallest is 4.088 %). The critical-path worker
sweep is 94–96 % of a master iteration and there is no room under it for the
tax the executor would remove. Per the spec, **the 5/9 clause does not drop.**

*Corrected by wave 4:* this paragraph printed **5.154 %**, which is not the
committed verdict at all — it is one of the six verdict maxima from the
census's own load-sensitivity repeat (`census/README.md` §"…4.359 %, 5.082 %,
5.151 %, 5.154 %, 5.257 % and 5.310 %"), and not that band's maximum either.
No conclusion moves: every reading in the band is less than half the bar.

The census also carries the audit's instrumentation items (F4's exact-attempt
split, RV2's publication poses, RV3's per-cell shas), the `icscal/v1` schema and
its writer, and the repair to `control.py`'s missing time filter. It changes no
quality semantics and no frozen item, and it proves that: 16 trajectory-identity
vectors across three binaries, the whole evidence audit green on the new tree,
and the committed fixed-work replay depths still reproducing bit for bit.

Read [`census/README.md`](census/README.md) — the verdict is its first section.

## What waves 2 and 3 built

The two strike arms and the calibrated-work pacer, neither of them a frozen
item. The arms sit behind **one field** on `ScheduleConfig` that defaults to
the control, with exactly one `match` on the arm anywhere in the trajectory —
`patience_exhausted`, which asks "200 batches without a 2 % improvement?" in
the control and "1,630,000 sample evaluations of None-batches?" in the
treatment. That the control is the trajectory the member closed is a
**measurement across two binaries**, not a reading of the diff:
[`integration/armgate.py`](integration/armgate.py) runs four fixed-work cells
through the round's base binary and the new one and finds **zero field
differences on all four**.

The pacer cannot read a clock three ways over: its module contains no
`std::time` (a FAST hygiene stage), it is handed a clock it never calls, and a
whole two-phase trajectory is driven under a clock that panics if it is read.
The spec's worst-ranked defect — double-debit, "stable but false" work
accounting — is three identities in the emitted ledger, built by routes that do
not touch each other.

## What wave 4 settled

**Funded change 3's own reject rule fired, and the round stopped without a
gate number.** Three independent runs, both currencies, all six ordered
fixture pairs — and still rejected on the heavy mixed-61 ↔ shapes-17 pair
alone, twelve readings from 10.48 % to 18.28 % against a 10.000 % bar. §0's
budget is a *10 s calibrated-work* plan, so the nine-seed two-arm battery was
never run, attribution is undecided, and the impatient work-quanta policy is
therefore **not promoted**: the frozen `200/3/100/5/0.98` remains the member.

The boundary floor is green — FAST at FAILURES=0, four pinned gates on both
builds identical as whole documents, five suites, two-binary determinism.

Read [`gate/README.md`](gate/README.md).
