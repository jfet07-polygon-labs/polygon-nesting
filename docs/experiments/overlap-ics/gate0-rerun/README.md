# Gate 0, re-run under the frozen fix list

This document is written in two halves and the halves were written at different
times, on purpose.

**§0 is the pre-committed reading.** It is copied from Sol review 15 §C and
Grok review 10 §C, plus the parent session's two arbitrations, and it was
committed to this file **before the battery was run** — commit
`docs: the pre-committed reading and the 240,000-proposal derivation`. Nothing
in it may be edited after a number arrives. If a cell fails, §0 says which
remaining defect that is; it does not license a knob.

**§1 onward is the result.** Written after.

The fix list is the union of Sol review 15 §B and Grok review 10 §B with the
parent session's arbitrations, and it is **frozen**: no knob was added or moved
after seeing any re-run number. What is explicitly *not* in this round is in §0.5.

---

## 0. The pre-committed reading (binding)

### 0.1 The regression floor

* **S0 must stay bit-for-bit**: `phi.to_bits() == 0`, raw depth **150.16451**,
  dual-valid, **0** repair rows, giveback 0.0, `two_r = 5000`.
* **Numeric soundness**: 0 false-feasible outside the 4 µm band, 0 containment
  false-feasible, 0 incremental mismatches.
* **Throughput** holds all four original thresholds: cold Φ ≤ 200 µs, row
  rebuild ≤ 20 µs, ≥ 1 M cell-gap evaluations/s, ≥ 100 K piece proposals
  projected into 8 s.

Any breakage of these **is a regression introduced by this round**, not a new
Gate 0 question.

### 0.2 triangle-20 — **MUST PASS**

Pass = a dual-valid child inside the locked **70.742**, repair ≤ **16 µm**,
giveback ≤ **0.050 mm**, within **200,000** proposals.

Failure ladder, in order, if it does not:

1. `max_g` still on **TOP** at ~0.11 mm → the clearance fix missed a caller;
   incomplete fix item 1.
2. 0 pairs and the residual **one-sided** on L/R/B → still implementation; look
   at the homotopy floor.
3. 0 pairs and **opposite** sides on the **same piece** → only then is the
   canary move-set evidence.

### 0.3 S1 — **not predicted**

The strip jump **must not fire** below 0.1 mm: verify through
`jumpAttempted` / `jumpCommitted` and the recorded scale, which must read
`kind: "ball"`.

Pass = dual-valid republication inside the locked **150.16547** within
**200,000** proposals, repair ≤ **16 µm**.

If it **still** freezes at ~12 µm with 0 accepts *after* a correctly-scaled
micro-jump, the verdict is **STOP for the single-piece strict-decrease
member** — and a chain-move solver is a new round's decision for the owner, not
a retrofit here. The report must say which of the two happened.

### 0.4 C175 — **not predicted**

Pass = **≥ 1 of 3** seeds produces a strict dual-valid non-constructor child
within **240,000** proposals, `entry_depth <= T` on every seed, every
publication dual-valid.

* If **0/3** with a **real installed jump** on the trajectories that stall
  above 0.1 mm: the family's separator fails. That **is** paradigm evidence and
  this document must say so plainly.
* If Φ **explodes** the way it did under the old `always` arm (seeds at
  Φ 925 and 3,359): the local sweep is still not a sweep — incomplete fix
  item 2, **not** a family kill.

### 0.5 What is not a pass, and what is not in this round

**A pass obtained by widening any band, raising any budget beyond the frozen
numbers, granting extra jump allowance, or disabling sag globally is NOT a
pass.**

Frozen, unchanged, and not to be touched by this round: the cell definitions,
the `W` pins (150.16547 / 70.742 / 168.484), `stalls_before_jump = 2`,
`jump_samples = 16`, the 4 µm attempt band, the 16 µm repair cap, C175's shock
formula `D0 − 0.10 (D0 − L)`, the homotopy schedule, the seeds. The two-scale
threshold 0.100 mm and the micro-ball radius `max(4 · max_g, ladder_top)` are
frozen **and derived**.

Explicitly **not in this round** (both reviews' §B "not in this round"):
accept-equal, SOR, chain/component moves, two-endpoint PGS, SE(2) decoupling,
extra jump allowance on mixed-61, any change to C175's shock formula, the
homotopy schedule, band widening, target relaxation, repair enlargement, seed
changes.

### 0.6 C175's proposal budget: 240,000, derived before the code

The spec states C175's clause in **two solver seconds**. A trajectory in this
engine never reads a clock, so the clause has to become a work quota, and the
quota has to be derived from a measurement that already existed rather than
from this round's machine on this round's day.

    240,000 = floor( 987,861 proposals / 8 s  ×  2 s )

`987,861` is the **original** round's `projectedProposalsInEightSeconds`
(`evidence/cell-throughput.json`), which is the slower of the two throughput
measurements on record — the verification round measured 1,061,224 on the same
box, 7 % faster, and the slower number is the conservative one. The original
Gate 0 ran C175 at 200,000 and used only **1.26–1.44 s** of the two seconds it
was entitled to, so 200,000 was under-spending the clause the cell is written
in. 240,000 spends it.

This is a **restoration of the spec's own clause**, not a budget increase
granted to rescue a cell: it is derived from a number measured before this
round began, it is written down here before the battery ran, and the wall of
every C175 seed is reported beside it so that "≤ 2 solver seconds" can be
checked directly rather than inferred.

Every other cell keeps **200,000**.

---
