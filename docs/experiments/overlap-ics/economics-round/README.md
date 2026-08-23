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
| 2 | executor agent ∥ meter agent | — | see the census's verdict |
| 3 | one integration agent owns `mod.rs`/`Pacer`/schema | — | not started |

## What wave 1 settled

**The persistent executor is NOT built.** The spec's pre-committed gate is
"build iff prep+dispatch ≥ 10 % of hard-state wall"; measured on the frozen
eight workers at the 179 shelf over 200 master iterations, two processes, six
seeds, the largest reading is **5.154 %**. The critical-path worker sweep is
94–96 % of a master iteration and there is no room under it for the tax the
executor would remove. Per the spec, **the 5/9 clause does not drop.**

The census also carries the audit's instrumentation items (F4's exact-attempt
split, RV2's publication poses, RV3's per-cell shas), the `icscal/v1` schema and
its writer, and the repair to `control.py`'s missing time filter. It changes no
quality semantics and no frozen item, and it proves that: 16 trajectory-identity
vectors across three binaries, the whole evidence audit green on the new tree,
and the committed fixed-work replay depths still reproducing bit for bit.

Read [`census/README.md`](census/README.md) — the verdict is its first section.
