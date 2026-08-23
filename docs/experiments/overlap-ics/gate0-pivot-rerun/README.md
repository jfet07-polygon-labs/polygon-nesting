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

---

## 1. The verdict table, with §0 applied

**Five of the six fatal cells pass. C175 does not: 0 of 3 — and this time §0.2's
antecedent is satisfied on all three seeds, so the verdict stands.**

| cell | class | verdict | the number that decides it |
|---|---|:--:|---|
| **S0** — the Sparrow pin | fatal | **PASS** | 61 placements, `rawSourceDepthMm` **150.16451**, `phi.to_bits()` **0**, Exclusive and contract accept at `two_r = 5000`, **0** repair rows, giveback 0.0 — bit-for-bit the previous round. A zero-energy layout never forms a gradient, so the fix is unreachable on it by construction |
| **S1** — ±0.5 mm / ±2°, locked `W = 150.16547` | fatal | **PASS** | Φ 433.4919 → **0.0**, `max_g` **0.0**, dual-valid republication at **150.16374161751165** inside the locked strip, repair **6.5 µm** ≤ 16, giveback **0.0028 mm** ≤ 0.050, first strict child at proposal **854** of 200,000 — and `jumpAttempted: 0`. **It no longer needs a jump.** |
| **C175** — constructor shocked by 0.10 (D₀−L) | fatal | **FAIL** | **0 of 3** strict children. `entryDepthSlackMm` **0.0** on all three (entry at exactly `T`). Φ 542.99/543.70/546.47 → **326.98/30.42/42.53**, `max_g` **8.297/1.569/1.953 mm**, **0 publication attempts on all three**, 2.223/1.925/1.754 solver seconds |
| **triangle-20** — locked `W = 70.742` | fatal | **PASS** | Φ 17.8407 → **0.0**, **0** active rows of any kind, dual-valid child at **70.73783697563869** ≤ 70.742, repair **0.0 µm**, giveback **0.0**, first strict child at proposal **480** of 200,000, `jumpAttempted: 0` |
| **numeric soundness** — 1,000 states | fatal | **PASS** | 0 outside the 4 µm band (worst **0 µm**), 0 containment false-feasible on **60**, 0 incremental mismatches, **501/501** force on the scored population — every field identical to the previous round |
| **throughput** | fatal | **PASS** | cold Φ **36.83 µs** (≤200), row rebuild **1.261 µs** (≤20), **7.42 M** cell gaps/s (≥1 M), **1,017,823** proposals projected into 8 s (≥100 K) |
| S2 — ±2 mm / ±10° | diagnostic | fail | Φ 12,668 → **1,003.36** (was 1,137.59), `max_g` **21.33 mm** (was 16.77) — entry bit-identical, so the comparison is clean |
| C168 — squeezed to 168.484 | diagnostic | fail | Φ 2,117.88 → **846.87** (was 1,139.40), `max_g` **11.671 mm** (was 11.403) — entry bit-identical **this round**, so unlike last round the comparison is clean |
| random-T — uniform throw, 8 jumps | diagnostic | fail | Φ 182,899 → **7,055.90** (was 4,207.06), `max_g` **23.86 mm** (was 20.74); 8 jumps, 8 installed, 3 improving (was 7) |
| 10,000-state corpus | diagnostic | **pass** | 0 outside band, 0 containment false-feasible on **589**, 0 incremental mismatches, **5001/5001** — identical to the previous round |

`GATE0_PASS: false`, `fatalFailures: ["C175"]`
(`evidence/gate0-pivot-rerun.json`).

**Every cell's entry is bit-identical to the previous round's.** S0, S1, S2,
C168, random-T, triangle-20 and all three C175 seeds match on entry Φ to the
last bit, on `perturbedPoseDigest` where there is one, and on
`shockedPoseDigest` where there is one. The pivot is in the *move set*, not in
the entry construction, so every comparison in this document is a clean one —
including C168's, which the previous round had to disown.

### 1.1 What §0.1's regression floor says

**It holds, on every clause.** S0 is bit-for-bit, both corpora are identical to
the previous round in every field, and all four throughput thresholds hold with
5x–10x margin. S1 and triangle-20 not only still publish, they publish
**sooner** (854 and 480, against 6,710 and 780) and **deeper**
(150.16374 against 150.16536; 70.73784 against 70.74150).

Repair goes the other way on S1 and is worth stating rather than rounding off:
S1 now publishes twice, with 2 repair rows, 6.5 µm of displacement and
0.0028 mm of giveback, where last round it published once with **0** repair
rows and 0 giveback. It is well inside both caps (16 µm, 0.050 mm) and it buys
1.6 µm of depth, but it is not an improvement in that column. triangle-20 is
strictly better on all three: 0 repair rows against 1, 0.0 µm against 5.0 µm,
and 3.7 µm deeper.

And both of them do it with **`jumpAttempted: 0`** — and with
`guidedStalls: 0` and `weightUpdates: 0`. Last round S1 needed a ball jump and
triangle-20 fired one it did not need; this round **neither trajectory stalls
even once**, so the guided-weight machinery never fires and the jump is never
licensed. The stall ladder is not being escaped: it is not being entered.
S1 reaches Φ = 0 in 358 accepted moves and triangle-20 in 178.

---

## 2. C175, and why §0.2's block quote is the verdict

§0.2 named three antecedents and said what follows if all three hold. All three
hold, and this round can show each of them rather than assert it.

**(1) The pivot is fixed.** `state::compose_proposal`, three unit vectors,
`evidence/pivot-red.log` (the transcript on the un-fixed tree) and
`evidence/pivot-green.log` (the same two vectors, green).

**(2) The jump is real and installed, on all three stalled trajectories.**

| seed | `guidedStalls` | jump kind | `max_g` at the gate | `jumpAttempted` | `jumpCommitted` | guided Φ across it |
|---|---:|---|---:|:--:|:--:|---|
| 0 | 2,487 | **strip** | 2.16898 mm | 1 | **1** | 45.25 → 9,363.68 |
| 1 | 3,240 | **strip** | 1.97293 mm | 1 | **1** | 43.93 → 934.08 |
| 2 | 1,380 | **strip** | 1.94546 mm | 1 | **1** | 42.91 → 5,791.76 |

All three stall **above** the 0.100 mm two-scale threshold, all three take the
strip branch, all three install. Last round seed 1 had `guidedStalls: 0` and
never licensed a jump at all, which is why the previous document could only
claim the antecedent on two of three seeds. **Seed 1 stalls now.** The escape
that left is closed.

**(3) The clearances are right.** `physicalEdgeClearanceMm` 5.0 and
`depthTopInsetMm` 5.0 on mixed-61 (`sag = 0`, so the split is a no-op there),
5.25 and 5.0 on triangle-20 (`sag = 0.25`, so the split is visible and doing
its job). `entryDepthSlackMm` is **0.0** on all three C175 seeds: the entry sits
at exactly `T`, not above it.

### 2.1 The measurement that makes it a separator and not a jump lottery

The three seeds' terminal numbers are noisy — 326.98 / 30.42 / 42.53 of raw Φ —
because the one permitted jump is a strip teleport that commits
unconditionally, and on all three seeds it is catastrophic (guided Φ up by
**207x, 21x and 135x**, in seed order). A verdict read off those three numbers
would be a verdict about a lottery.

So the same cell was run with **`--jumps=0`** on both this round's binary and on
**commit 1f5cd5b**, the tree with the pivot broken. Same three seeds, same
240,000-proposal quota, same everything else. With no topology move there is
nothing stochastic left, and the two columns differ by the pivot and by nothing
else (`evidence/pivot-ab.json`).

| seed | Φ, broken pivot | Φ, fixed pivot | `max_g`, broken | `max_g`, fixed | Δ `max_g` |
|---|---:|---:|---:|---:|---:|
| 0 | 38.041 | **35.909** | 2.27274 mm | **1.61969 mm** | **−28.7 %** |
| 1 | 50.915 | **36.023** | 2.89646 mm | **1.59662 mm** | **−44.9 %** |
| 2 | 38.941 | **36.155** | 2.42217 mm | **1.60740 mm** | **−33.6 %** |

Three things follow, and they have to be read together.

* **The fix is real and it is uniform.** `max_g` — the *only* quantity the
  publication band is measured against — falls by 29 % to 45 % on every seed.
* **The corrected descent has an attractor and the broken one did not.** The
  fixed arm lands on Φ 35.909 / 36.023 / 36.155 and `max_g` 1.61969 / 1.59662 /
  1.60740 from three different shocked entries: a spread of 0.7 % in Φ and
  1.4 % in `max_g`. The broken arm scatters over 38.0–50.9 and 2.27–2.90. Three
  independent trajectories converging to the same point is what a fixed point
  of a *correct* descent looks like.
* **That fixed point is 400x outside the band.** The best `max_g` any of the six
  trajectories reached is **1.59662 mm**, against the 4 µm attempt band:
  a factor of **399**. `exactCheckpointAttempts` is **0** on all six. Not one
  trajectory, on either pivot, with or without the jump, ever got close enough
  to legality for the publication path to be *called*.

The residual's shape is the same on all three fixed-pivot seeds and it is the
shape of a frustration, not of a missing move:

    active pair rows      25 / 24 / 23
    active boundary rows  21 / 21 / 21   -> bottom 7, top 14, left 0, right 0
    worst bottom          1.61969 / 1.59662 / 1.60740 mm
    worst top             1.20017 / 1.13633 / 1.10069 mm
    pieces squeezed on opposite sides   0 / 0 / 0

Seven pieces are pressed through the sheet's bottom edge and fourteen through
the strip's top **at the same time**, on different pieces, with left and right
completely clear. No single rigid translation satisfies both sets, and no
single piece is squeezed both ways — so there is no per-piece move, of any
size, that reduces the pair. The cell asks for 6.714 mm of compression
(`T = 176.262` against `D₀ = 182.976`) and the descent distributes the last
1.6 mm of it across 21 pieces along the one axis it is being compressed on.

### 2.2 §0.2's two escape clauses, both checked

* *"If Φ explodes the way it did under the old unconditional arm (seeds at
  Φ 925 and 3,359), the local sweep is still not a sweep — an implementation
  defect, not a family kill."* **It does not explode.** Seed 0's 326.98 is the
  jump's damage and nothing else: with the jump removed the same seed lands at
  **35.909**, and all three seeds land within 0.7 % of each other. The local
  sweep is a sweep, and the descent is measurably a better descent than last
  round's.
* *"If 0/3 with a real installed jump on trajectories that stall above 0.1 mm,
  the family's separator fails."* 3 of 3 seeds stall above 0.1 mm, take the
  strip branch, and install. The antecedent is satisfied literally and
  completely.

### 2.3 The verdict, stated plainly as §0.2 requires

**C175 is paradigm evidence. The family's separator fails.**

The corrected SE(2) move set — the one the converged spec actually specifies,
with the torque and the coordinate now taken about the same point — converges
reproducibly, from three independent shocked entries, to a fixed point whose
worst violation is **1.6 mm** against a **4 µm** band, and the one topology
move the spec licenses moves it *away* from legality on all three seeds.

Four independent C175 batteries of 719,922 proposals each — this round's cell,
the jump-free fixed-pivot arm, the jump-free broken-pivot arm, and the previous
round's cell — and **`exactCheckpointAttempts` is 0 in every one of the twelve
trajectories**. In 2,879,688 piece proposals this cell has never once been
close enough to legality for the publication path to be called.

There is nothing left to blame that this campaign has not now measured.

---

## 3. The census this round was required to run, and what it did not find

§0.2 licenses one more mechanical round **only** if this round's own census
surfaces a new named defect of the pivot's concreteness — a code line **and** a
measurement. It does not. What it found is below, in full, including the two
things that are uncomfortable.

### 3.1 The step-scaling signature moved, and did not vanish

The previous round's §2.1 fourth row was the pivot's fingerprint:
`Δ(incident guided)` positive **and linear in the step** all the way to the
0.25 µm floor, which is a first-order *ascent* coefficient on a direction whose
steepest descent coefficient should be `−|∇|`.

Fitting the local exponent `p` in `Δ ~ s^p` over the last exact halving of the
ladder, on the same 32-rejection sample:

| | median `p` | `p < 1.5` (first-order ascent) |
|---|---:|---:|
| seed 0, broken pivot | 1.174 | **21 / 32** |
| seed 0, fixed pivot | **1.770** | **11 / 32** |
| seed 2, broken pivot | 1.284 | **22 / 32** |
| seed 2, fixed pivot | **1.656** | **14 / 32** |

(Seed 1 has no broken-pivot sample: it never stalled last round, so its census
never armed.)

The ascent population roughly halves and the median exponent moves most of the
way from 1 (ascent) toward 2 (a stationary point with curvature). It does not
reach 2, and about a third of sampled rejections still refuse at first order.

**That residue is not a defect and this document will not pretend it is one.**
Φ is a hinge over a **max** — `v_ij` is a max over cell pairs and each boundary
residual is a max over ring vertices — so `incident_gradient` returns a
*subgradient* element taken at one witness, and at a non-smooth point a
subgradient direction can ascend when the maximizer switches. That is a
property of the spec's own force model `tau = (p − c_i) x (w v n)`, not a
disagreement between two pieces of code, and this round's own unit test
`a_ladder_step_descends_the_energy_its_own_gradient_was_taken_from` walks
straight through an instance of it (the bottom-most corner of its square
switches under the step, and the direction is still a descent direction because
the translation dominates). No code line was found where the gradient and the
coordinate disagree any more. There is no second pivot.

### 3.2 A correction to the previous round's §2.1, which is mine to make

The previous round wrote: *"the guided escalation reached weight 226 somewhere
in the layout and **never reached a single row incident on the pieces that were
being refused**"*, on the evidence that `activeIncidentPenaltyMax` is 0 on all
32 sampled rejections.

**That claim is scoped more narrowly than it was stated.** The census arms on
the *first* stalled sweep (`descent.rs`, `self.census.armed = true` in
`on_stalled_sweep`) and fills its 32 slots immediately
(`records.len() < rejection_census_samples`), so the sample is the first
deadlock and not the terminal one:

| | census records span | of | fraction in |
|---|---|---:|---:|
| seed 0, broken pivot | ordinals 22,633 – 22,708 | 239,974 | 9.5 % |
| seed 0, fixed pivot | ordinals 7,139 – 7,217 | 239,974 | 3.0 % |
| seed 1, fixed pivot | ordinals 2,259 – 2,335 | 239,974 | 1.0 % |
| seed 2, fixed pivot | ordinals 16,532 – 16,605 | 239,974 | 6.9 % |

At seed 0's sampling window the whole layout carries `guidedΦ / rawΦ` = 41.383
/ 36.678 = **1.13**; by the end of the trajectory it carries **184.8**. So
`activeIncidentPenaltyMax = 0` is a true statement about a moment when almost
no weight had been assigned anywhere, and it is **not** evidence that escalated
weight fails to reach blocking rows at the deadlock the cell actually dies in.
`activeIncidentPenaltyMax` is 0 on 95 of this round's 96 sampled rejections,
and that number should be read with this caveat attached, in both documents.

This is a code line and a measurement, and it is reported here rather than
repaired, exactly as the pivot was. **It does not license another mechanical
round**, and the reason is not a judgement call: the rejection census is
read-only — enabling it cannot move a trajectory — so no scoping of it can
change C175's 0/3, its `max_g` of 1.6 mm, or its zero publication attempts.
Correcting it would produce a better *document*, not a different verdict.

### 3.3 The unconditional strip commit costs, and it is frozen

On all three C175 seeds the one permitted jump raises guided Φ by 207x, 21x and
135x and is installed anyway, and the trajectory then spends the remaining
96.6 %, 98.6 % and 92.7 % of its quota failing to get back. `descent.rs`'s
`let install = self.config.jump_commits_unconditionally || improved_guided;` is
the line.

It is **not** offered as a new defect, for two reasons. It is frozen by §0.4 —
the previous round's arbitration installed it deliberately, on an A/B, and this
round is bound by that. And it cannot change the verdict: the arm with the jump
removed entirely is *also* 0/3, with **zero** publication attempts and
`max_g` 1.6 mm.

What it does change is the reading of the *diagnostics*, and §5 says so there.

---

## 4. The basin sweep, and the A/B that no longer separates anything

`evidence/basin-jump-default.json` and `evidence/basin-jump-guided.json` — the
S0 pin perturbed by a ladder of magnitudes, everything else identical, both
arms at 200,000 proposals.

| perturbation | entry Φ | derived commit rule | `--jumpcommit=guided` |
|---|---:|---|---|
| 0.005 mm / 0.02° | 0.003 | ✅ 150.16229, Φ 0, 0 µm, **0 jumps** | ✅ identical |
| 0.020 mm / 0.08° | 0.158 | ✅ 150.14447, Φ 0, 0 µm, **0 jumps** | ✅ identical |
| 0.050 mm / 0.20° | 1.567 | ✅ 150.14875, Φ 0, 0 µm, **0 jumps** | ✅ identical |
| 0.100 mm / 0.40° | 9.742 | ✅ 150.16061, Φ 0, 0 µm, **0 jumps** | ✅ identical |
| 0.250 mm / 1.00° | 90.190 | ✅ 150.15690, Φ 0, 0 µm, **0 jumps** | ✅ identical |
| **0.500 mm / 2.00°** (S1) | 433.492 | ✅ **150.16374, Φ 0, 6.5 µm, 0 jumps** | ✅ **identical** |
| 2.000 mm / 10.0° (S2) | 12,668 | ❌ Φ 1,003.36, `max_g` 21.33 | ❌ Φ 235.18, `max_g` 10.48 |

**The basin's extent is unchanged — and its mechanism is not.** It still runs
to 0.5 mm / 2.0° and still stops before 2.0 mm / 10°. But **no jump fires on
any of the six passing rungs, in either arm**, so the two arms are byte-equal on
every one of them, and the previous round's headline A/B result is superseded:

> *"the **only** difference between republishing and freezing at 12.635 µm is
> whether the jump is allowed to commit a candidate that does not improve
> guided Φ."*

That was true of the code that measured it. It is not true any more. With the
pivot corrected, S1 never stalls at all (`guidedStalls: 0`), never licenses a
jump, and republishes on the descent alone. The commit rule is now
**unobservable** anywhere inside the basin, and the 169-attempt /
164,944-proposal cost the `guided` arm used to pay on S1 is gone with it — it
attempts **0**.

The two arms separate only at 2.0 mm / 10°, where the residual is
millimetre-scale, the strip branch is licensed, and the unconditional commit is
**worse**: Φ 1,003.36 against 235.18, `max_g` 21.33 mm against 10.48 mm, on 1
installed jump against 13 attempted and 0 installed. That is the honest price of
§0.4's frozen arbitration, and it is now the *only* place in the sweep where
that price is charged.

---

## 5. The three diagnostics, and what the pivot did to them

All three entries are bit-identical to the previous round's, so all three
comparisons are clean — which is itself new: last round C168's was not.

| diagnostic | Φ out, previous | Φ out, this round | `max_g`, previous | `max_g`, this round |
|---|---:|---:|---:|---:|
| S2 (±2 mm / ±10°) | 1,137.59 | **1,003.36** | 16.768 mm | **21.328 mm** |
| C168 (squeezed to 168.484) | 1,139.40 | **846.87** | 11.403 mm | **11.671 mm** |
| random-T (8 jumps) | 4,207.06 | **7,055.90** | 20.741 mm | **23.855 mm** |

Φ improves on two and worsens on one; `max_g` worsens on all three. None of
them publishes, none of them published before, and none carries a fatal
verdict.

The honest reading is that **these three are dominated by the jump, not by the
descent.** Every one of them fires a strip relocation (S2 at `max_g` 10.475 mm,
C168 at 7.807 mm, random-T eight times from 58.73 mm down to 21.77 mm) and
installs it unconditionally, and §3.3's measurement on C175 is that an
installed strip relocation costs one to two orders of magnitude of guided Φ that
the remaining quota does not recover. random-T's eight installs, only 3 of which
improved guided Φ against last round's 7, is that effect eight times over.

The basin sweep's 2.0 mm row is the controlled version of the same statement:
same entry, same code, jump refused → Φ 235.18 instead of 1,003.36. So the
diagnostics moving the wrong way is **not** evidence against the pivot fix; the
controlled A/B in §2.1 is the evidence about the pivot fix, and it is uniform
and in the other direction.

### 5.1 The frozen-θ probes, kept current

Grok review 10 Finding 3 asked for the S1 one by name.

| probe | Φ out | `max_g` | published |
|---|---:|---:|:--:|
| S1, rotation on, derived commit rule | **0.0** | **0.0** | ✅ 150.16374 |
| S1, rotation on, `--jumpcommit=guided` | **0.0** | **0.0** | ✅ 150.16374 |
| S1, rotation **off**, derived commit rule | 6,180.23 | 61.205 mm | ❌ |
| S1, rotation **off**, `--jumpcommit=guided` | 20.62 | 1.433 mm | ❌ |

Freezing θ still makes S1 catastrophically worse, which is what it should do —
the cell's own perturbation is ±2° and only rotation can undo it. So §2's
verdict is not "rotation is useless": it is "rotation is necessary, its
direction is now right, and it is still not enough for C175". Both halves have
to survive together and they do.

One number in that table moved for a reason worth writing down: the rotation-off
rows are **not** bit-identical to the previous round's (6,180.23 against
5,039.1). `compose_proposal` at `dtheta = 0` computes
`pivot + (t − pivot) + dt` rather than `t + dt`, and although those are equal in
exact arithmetic they can differ by an ulp in floating point. A 1-ulp
perturbation on a 200,000-proposal trajectory that ends in the thousands of Φ
diverges; the `guided` row, which converges, is unchanged to five figures
(20.6243 against 20.63). No cell runs with rotation off, so no cell is affected,
and the round did not special-case `dtheta == 0` to hide it — that would have
been a second behaviour change made after seeing a number.

---

## 6. Determinism: two processes, every cell

`evidence/determinism-two-process.json`. Eleven cells, each run in two separate
processes with identical arguments, compared over the **entire** document minus
the `wall` object. `ALL_BIT_IDENTICAL: true`.

| cell | bit-identical |
|---|:--:|
| S0, S1, S2 | ✅ |
| C175 seed 0, seed 1, seed 2 | ✅ |
| triangle-20, C168, random-T | ✅ |
| 1,000- and 10,000-state corpora | ✅ |
| throughput | not claimed — every number in it is a timing; its four verdict booleans are identical |

The pivot is read once per proposal from the cached transformed geometry, so it
is a pure function of the state; nothing in the fix introduces an ordering, an
address or a clock. The determinism contract is unchanged and is still the
narrowed one: same box, same toolchain, same target, `std::f64::sin_cos` rather
than `libm`, no cross-platform `sin`/`cos` identity claimed.

---

## 7. The round-boundary battery

Run from the **clean committed tree**, on binaries rebuilt from it.
`drivers/heavy.sh`, `FAILURES=0`; `drivers/fast.sh`, `FAILURES=0`.

This is the second attempt and the first one that counts, and the reason is
worth recording rather than hiding. The first attempt was stopped after two
suites because I noticed and corrected an arithmetic slip in a doc comment in
`tests.rs` **while it was running** (§9, caveat 7). A round-boundary battery
that straddles an edit is not a round-boundary battery: the previous round's
suite logs were restored with `git checkout`, the partial evidence was deleted,
and the whole thing was re-run from the tree the last commit leaves behind. One
incidental confirmation falls out of that: **both binaries rebuilt
byte-identically across the two attempts** — `61befdc5…` and `cbf01fe3…` —
which is independent evidence that the corrected comment changed nothing
executable.

### 7.1 The four pinned gates, on two binaries

`evidence/gates.json`, `evidence/binaries.txt`. The `base` binary is
`--features jagua-experimental` (this round's feature **absent**); the `meas`
binary is `--features jagua-experimental,overlap-ics` (**compiled**, and
unarmed — nothing outside the example can reach it).

| gate | pinned | base | meas | documents identical |
|---|---|:--:|:--:|:--:|
| g1 | 206.869 / `8a7737381238fa4d` | ✅ | ✅ | ✅ |
| g2 | 159.09233022733062 / `fa01012af1d559ae` | ✅ | ✅ | ✅ |
| g3 | 159.07876040364795 / `e28fba007f8031d4` | ✅ | ✅ | ✅ |
| g4 | 164.0375677990678 / `49f094d7e59a9008` | ✅ | ✅ | ✅ |

`BASE_ALL_PASS: true`, `MEAS_ALL_PASS: true`,
**`WHOLE_DOCUMENT_IDENTITY: true`** on all four. g2, g3 and g4 also report
`exactValid: true` and `contractValid: true`.

The `base` binary's sha256 is `61befdc544b4135a…` — **byte-identical to the
previous two rounds' base binary**, and that is the check that matters after a
round which changed the engine's move set: the default build did not move one
byte. The `meas` binary is `cbf01fe3c83f68db…` and differs from the previous
round's `2fc9ac2c00511ca8…`, as it must.

### 7.2 The suites

`evidence/suites.json` and `evidence/suite-*.log`. All `--release`, every exit
status read directly on the line after its command, no pipelines.
`SUITES_PASS: true`; no suite tripped the campaign's known allocator flake, so
no rerun clause fired.

| # | features | targets | passed | failed | ignored | exit |
|---|---|---:|---:|---:|---:|---:|
| 1 | `jagua-experimental` | 55 | 1293 | 0 | 2 | **0** |
| 2 | the protocol's full combo | 55 | 1357 | 0 | 2 | **0** |
| 3 | `jagua-experimental`, `--example general_request_benchmark` | 1 | 20 | 0 | 0 | **0** |
| 4 | `jagua-experimental,overlap-ics` | 55 | **1345** | 0 | 2 | **0** |
| 5 | `overlap-ics` alone, `--lib --tests` | 50 | **1155** | 0 | 0 | **0** |

Suites 1, 2 and 3 are unchanged from all three previous rounds. Suites 4 and 5
are each exactly **+3** (1342 → 1345, 1152 → 1155): the three pivot vectors
this round added, and nothing else.

`run-suites.sh` writes its five logs into the **original** round's committed
`evidence/`, so `heavy.sh` copies them here and restores that round's with
`git checkout` — the same procedure the previous two rounds used, so all four
rounds' logs exist and none overwrote another.

### 7.3 The FAST tier

`evidence/fast-tier-stdout.txt`: **EXIT=0**, ten stages, no red.

    default-build compile check                       EXIT=0
    cargo tree --features overlap-ics                 EXIT=0
    dependency hygiene: jagua-rs ABSENT               EXIT=0
    module unit vectors                               EXIT=0
    validation_vectors::sat_penetration_...           EXIT=0
    canonical_grid_vectors                            EXIT=0
    collision_builder_vectors                         EXIT=0
    release example build                             EXIT=0
    1,000-state contact corpus                        EXIT=0
    two-process fixed-work smoke                      EXIT=0

`evidence/smoke-two-process.json`: `SMOKE_PASS: true`,
`INVARIANTS_PASS: true`, S0's two-process digest `716baae468196ade…` —
the previous round's, unmoved. Stage 6's S1 clause is green as it was last
round; what changed is that it is now green **without a jump**.

---

## 8. What this round did not change

Named explicitly, because a one-line round is only credible if the list of
untouched adjacent things is also written down.

* **`energy::incident_gradient`** — not one character. Turning the torque about
  the pose origin would have made the two agree just as well and would have
  repudiated the spec's `tau = (p − c_i) x (w v n)` and its `R_i`.
* **The jump's ball branch** still draws its 16 candidates about the pose
  origin. It is a low-discrepancy *sample*, not a gradient step, so it is not
  the mismatch this round named; what the origin pivot distorts there is the
  shape of the ball, and the ball still contains offsets at every scale below
  `rho`. And it is not material to a single number in this document: across
  **every** JSON in the battery — ten cells, fourteen basin rows, five probes,
  six A/B arms — there are **165 jump events and all 165 are `strip`**. The
  ball branch did not fire once. That is the two-scale gate working from both
  ends: everything that stalls, stalls in millimetres, and everything below
  0.1 mm no longer stalls at all. The strip branch already positions by
  transformed centroid.
* **`Engine::displace`** — the cells' own perturbation operator. Untouched, and
  that is why every entry in §1 is bit-identical to the previous round's.
* **`publish.rs` repair** — pure translation, no rotation, nothing to pivot.
* **`homotopy.rs`** — still the stub.
* Every frozen item of §0.4: cell definitions, the `W` pins, C175's shock
  formula and its 240,000, the 200,000 elsewhere, the seeds, the 0.100 mm strip
  gate, `max(4 · max_g, ladder_top)`, `rho / R`, the 16 µm repair cap, the 4 µm
  band, `stalls_before_jump = 2`, `jump_samples = 16`, the unconditional commit
  on both branches.
* Nothing on the two reviews' "not in this round" list: accept-equal, SOR,
  chain/component moves, two-endpoint PGS, SE(2) decoupling, extra jump
  allowance, band widening, target relaxation, repair enlargement, seed changes.

**No knob was moved after a result.** The six drivers are the previous round's,
byte-identical modulo the round rename, and `evidence/pivot-ab.json` and
`evidence/probes.json` are read-only probes added *before* their numbers were
read, neither of which is a cell and neither of which carries a verdict.

---

## 9. Caveats, in full

1. **C175 seed 0 spent 2.223 solver seconds**, 11 % over the two-second clause
   the 240,000-proposal quota was derived to fit, so
   `allWithinTwoSolverSeconds` is `false` in the verdict document. The quota
   itself did not move and the seed spent exactly 239,974 proposals like the
   other two; this is the box being slower today than the box the quota was
   derived from. It does not change the verdict — the cell is 0/3 on the
   proposal clause alone — but it is a number in the evidence that reads
   `false` and it should not be found by a reader rather than stated here.
2. **The rejection census is scoped to the first deadlock** (§3.2). Every
   `activeIncidentPenaltyMax` statement in this campaign, including the
   previous round's, carries that caveat.
3. **The three diagnostics moved the wrong way on `max_g`** (§5). The
   explanation offered — that they are dominated by an unconditionally
   committed strip relocation — is supported by the basin's 2.0 mm row and by
   §3.3's C175 measurement, but it is an explanation and not a controlled
   experiment on those three cells.
4. **The rotation-off probes are not bit-comparable across rounds** (§5.1).
5. **`p < 1.5` on about a third of sampled rejections** (§3.1). The reading
   offered — subgradient of a max — is a structural argument, and this document
   states it as one. It is the one place where a future round could still find
   something, and §0.2's bar for that is a code line and a measurement, which
   this round did not produce.
6. **`forceActiveAtLeast95AllFamilies` is `false`** on both corpora
   (0.775 / 0.778), exactly as in the previous two rounds. The clause is scored
   on the compressed population per the original arbitration and passes at
   1.000 there; the all-families number is reported and is not a gate. Sol
   review 15 §D's reading of it stands unchanged and unaddressed by this round.
7. **One of this document's own vectors shipped with a wrong sentence in its
   doc comment.** The first vector described its accepted
   step's rate as *"1.225 mm/mm of lift from the translation, less 0.816 of
   drop from the 10 mm arm"*. Those are the wrong two numbers: the translation
   lifts at 0.816 and the arm moves each corner by ±0.408, and the minimum is
   taken at the corner the arm carries **down**, so the rate is
   0.816 − 0.408 = 0.408. The conclusion the comment drew — 0.408 mm/mm, nine
   times slower than the 3.674 the origin pivot drags it the other way — was
   right, and no assertion was ever wrong. It is corrected, `pivot-red.log`'s
   verification recipe is pinned at `5685ca3` so it still checks, and §7 records
   what correcting it cost the battery.

---

## 10. Reproduction

```sh
bash docs/experiments/overlap-ics/gate0-pivot-rerun/drivers/rerun.sh      # every cell + basin + determinism
python3 docs/experiments/overlap-ics/gate0-pivot-rerun/drivers/probes.py  # the read-only probes
python3 docs/experiments/overlap-ics/gate0-pivot-rerun/drivers/ab.py      # the jump-free A/B against 1f5cd5b
bash docs/experiments/overlap-ics/gate0-pivot-rerun/drivers/heavy.sh      # gates + suites, round boundary
ICS_ROOT=<worktree> bash docs/experiments/overlap-ics/drivers/fast.sh
```

`ab.py` needs the un-fixed tree built first — that is the whole point of it, so
it is not optional and it is not a path the script can guess:

```sh
mkdir -p /tmp/base-tree && git archive 1f5cd5b | tar -x -C /tmp/base-tree
cd /tmp/base-tree && cargo build -p polygon-nesting-core --release \
  --features overlap-ics --example overlap_ics_benchmark
```

Do **not** pipe any of them into `tee` or `tail`: you would read the pipe's
status instead of the script's.

The committed documents are the drivers' output with one renaming, following
the previous rounds' convention: `s0.json` is committed as `cell-s0.json` and
so on, so all four rounds' evidence files line up by name.

### 10.1 The three vectors, red and green

```sh
# RED, against the tree with the defect
git checkout 1f5cd5b
git show 5685ca3:crates/polygon-nesting-core/src/search/overlap_ics/tests.rs \
  | head -1076 > crates/polygon-nesting-core/src/search/overlap_ics/tests.rs
cargo test -p polygon-nesting-core --release --features overlap-ics --lib \
  -- --exact --test-threads=1 \
  search::overlap_ics::tests::a_ladder_step_descends_the_energy_its_own_gradient_was_taken_from \
  search::overlap_ics::tests::a_proposal_moves_the_transformed_centroid_by_its_translation_alone
# exit 101; the transcript is evidence/pivot-red.log

# GREEN, against this tree, all three
cargo test -p polygon-nesting-core --release --features overlap-ics --lib \
  search::overlap_ics::tests:: 
# exit 0; evidence/pivot-green.log is the three vectors alone
```
