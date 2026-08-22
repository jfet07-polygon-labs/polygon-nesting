# Gate 0, re-run against the torque pivot

The third and final mechanical round. It changes **one** line of behaviour —
the pivot a proposal turns about — and then re-runs the whole battery against
it. Verdicts that survive this round stand.

This document is written in two halves, at two different times, on purpose.

**§0 is the pre-committed reading.** It is the parent session's arbitration
copied verbatim, and it was committed to this file **before the battery was
run** — commit *"The reading this round is bound by, written before the
battery"*. Nothing in it may be edited after a number arrives.

**§1 onward is the result.** Written after.

---

## 0. The pre-committed reading (binding)

### 0.1 The regression floor

**S0, S1, triangle-20, soundness (1,000 and 10,000), throughput MUST remain
PASS exactly as the previous round left them.**

* **S0 bit-for-bit**: `phi.to_bits() == 0`, raw depth **150.16451**, dual-valid,
  **0** repair rows, giveback 0.0, `two_r = 5000`, 61 placements.
* **S1 republishes inside the locked 150.16547**, repair ≤ 16 µm, giveback
  ≤ 0.050 mm, within 200,000 proposals.
* **triangle-20 publishes inside the locked 70.742**, same caps, same quota.
* **numeric soundness**: 0 false-feasible outside the 4 µm band, 0 containment
  false-feasible, 0 incremental mismatches, on both populations.
* **throughput** holds all four thresholds: cold Φ ≤ 200 µs, row rebuild
  ≤ 20 µs, ≥ 1 M cell-gap evaluations/s, ≥ 100 K piece proposals projected into
  8 s.

**Any break of these is a REGRESSION caused by this round's fix. Investigate
the fix. Do not touch the cell.**

### 0.2 C175 — not predicted, and this is the clause that matters

Pass = **≥ 1 of 3** seeds produces a strict dual-valid non-constructor child
within **240,000** proposals, `entry_depth <= T` on every seed, every
publication dual-valid.

> **If C175 is 0/3 with the pivot fixed, the jump real and installed on stalled
> trajectories, and the clearances right — the verdict STANDS as the family
> separator failing. State it plainly as paradigm evidence. No further
> mechanical round is licensed unless this round's own census surfaces a NEW
> named defect of the same concreteness as the pivot (a code line **and** a
> measurement), in which case report it unrepaired exactly as the previous
> round did.**

The three antecedents are each checkable in the evidence and each must be
reported beside the verdict:

1. *the pivot fixed* — `state::compose_proposal`, and the three unit vectors of
   `evidence/pivot-red.log` / the module suite;
2. *the jump real and installed on stalled trajectories* —
   `jumpAttempted` / `jumpCommitted` per seed, and the `max_g` each fired at;
3. *the clearances right* — `physicalEdgeClearanceMm` and `depthTopInsetMm` on
   the contract object, the split the previous round installed.

The previous round's two escape clauses are retained and must both be checked:

* if Φ **explodes** the way it did under the old unconditional arm (seeds at
  Φ 925 and 3,359), the local sweep is still not a sweep — an implementation
  defect, **not** a family kill;
* if 0/3 with a **real installed** jump on trajectories that stall above
  0.1 mm, the family's separator fails, and §0.2's block quote is the verdict.

### 0.3 The diagnostics

S2, C168 and random-T are **reported, never fatal**. This round must in
addition note **whether the pivot fix moved them**, and say so from
bit-identical entries where the entry is bit-identical and refuse the
comparison where it is not.

### 0.4 What is not a pass, and what is not in this round

**A pass obtained by widening any band, raising any budget, granting extra jump
allowance, disabling sag, or moving any knob after a result is NOT a pass.**

**FROZEN and not touched by this round**: every cell definition; the `W` pins
150.16547 / 70.742 / 168.484; C175 = perturb-then-compress with `entry <= T`
asserted; 240,000 proposals for C175 and 200,000 for everything else; the
seeds; the 0.100 mm two-scale strip gate; the micro-ball radius
`max(4 · max_g, ladder_top)`; the `rho / R` angular radius; the 16 µm repair
cap; the 4 µm attempt band; `stalls_before_jump = 2`; `jump_samples = 16`;
`jump_commits_unconditionally = true` by default on **both** branches (the
previous round's arbitration stands); `homotopy.rs` stays a stub.

Explicitly **not in this round**: accept-equal, SOR, chain/component moves,
two-endpoint PGS, SE(2) decoupling, extra jump allowance on mixed-61, any
change to C175's shock formula, the homotopy schedule, band widening, target
relaxation, repair enlargement, seed changes.

### 0.5 The one thing that did change

`descent::propose` composes its ladder step through
`state::compose_proposal`: a rotation of `dtheta` **about the piece's
transformed centroid**, then the translation `dt`. The torque is untouched —
turning it about the origin instead would have repudiated the spec's own
`tau = (p − c_i) x (w v n)` and its `R_i`.

Nothing else in `search::overlap_ics` moved. The jump's ball branch, the
perturbation operator `Engine::displace`, and publication repair are unchanged
and are listed with their reasons in §6.

### 0.6 C175's budget, unchanged and still derived

    240,000 = floor( 987,861 proposals / 8 s  ×  2 s )

`987,861` is the original round's `projectedProposalsInEightSeconds`, the
slower of the two throughput measurements on record. This is the previous
round's derivation carried forward unchanged, not a new one; the wall of every
C175 seed is reported beside it so "≤ 2 solver seconds" can be checked
directly. Every other cell keeps 200,000.
