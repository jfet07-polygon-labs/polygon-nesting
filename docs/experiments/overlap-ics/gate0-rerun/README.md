# Gate 0, re-run under the frozen fix list

> **Superseded on three specific claims, and left standing as the record of its
> own round.** §2's defect — the torque pivot — was fixed in the next round and
> Gate 0 re-run whole against it: `../gate0-pivot-rerun/README.md`. Three
> things below are no longer true of the engine, and one of them is a
> correction to a claim this document over-stated:
>
> 1. **§3's refusal to call C175 paradigm evidence has been discharged.** §2.4
>    declined the conclusion because "a failure measured on a move set that is
>    not the specified one is not evidence about the specified one". It is the
>    specified one now, and C175 is still 0/3 — with all three antecedents
>    satisfied on all three seeds, where this round could only claim two.
> 2. **§4's headline A/B no longer separates anything.** "The *only* difference
>    between republishing and freezing at 12.635 µm is whether the jump is
>    allowed to commit" was true of the code that measured it. With the pivot
>    corrected, S1 never reaches a second guided stall, never licenses a jump,
>    and republishes on the descent alone; both arms are byte-equal on all six
>    passing rungs of the basin.
> 3. **§2.1's second row is scoped more narrowly than it was stated.** The
>    rejection census arms on the *first* stalled sweep and fills immediately,
>    so `activeIncidentPenaltyMax = 0` describes proposal ordinals
>    22,633–22,708 of 239,974 — 9.5 % into the trajectory — and not the
>    deadlock the cell dies in. See `../gate0-pivot-rerun/README.md` §3.2.
>
> §2.3's promise — *"writing [the unit vector] is the first thing the next
> round should do, before the fix, so that the fix has something to turn
> green"* — was kept: `../gate0-pivot-rerun/evidence/pivot-red.log`.

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

## 1. The verdict table, with §0 applied

**Five of the six fatal cells pass. C175 does not: 0 of 3.**

| cell | class | verdict | the number that decides it |
|---|---|:--:|---|
| **S0** — the Sparrow pin | fatal | **PASS** | 61 placements, `rawSourceDepthMm` **150.16451**, `phi.to_bits()` **0**, Exclusive accepts at `two_r = 5000`, contract accepts, **0** repair rows, giveback 0.0 — bit-for-bit the previous round |
| **S1** — ±0.5 mm / ±2°, locked `W = 150.16547` | fatal | **PASS** *(was FAIL)* | Φ 433.4919 → **0.0**, `max_g` **0.0**, dual-valid republication at **150.16536149919963** inside the locked strip, **0** repair rows, **0** giveback, first strict child at proposal **6,710** of 200,000 |
| **C175** — constructor shocked by 0.10 (D₀−L) | fatal | **FAIL** | **0 of 3** strict children. Entry at exactly `T` on all three seeds. Φ 542.99/543.70/546.47 → **31.29/50.91/33.11**, `max_g` **2.221/2.896/2.206 mm**, 0 publication attempts, 1.45/1.59/1.89 solver seconds |
| **triangle-20** — locked `W = 70.742` | fatal | **PASS** *(was FAIL)* | Φ 17.8407 → **0.0**, **0** active rows of any kind, dual-valid child at **70.74150389598567** ≤ 70.742, repair **5.0 µm**, giveback **0.0**, first strict child at proposal **780** of 200,000 |
| **numeric soundness** — 1,000 states | fatal | **PASS** | 0 outside the 4 µm band (worst **0 µm**), 0 containment false-feasible on **60**, 0 incremental mismatches, **501/501** force on the scored population |
| **throughput** | fatal | **PASS** | cold Φ **37.07 µs** (≤200), row rebuild **1.250 µs** (≤20), **7.43 M** cell gaps/s (≥1 M), **981,975** proposals projected into 8 s (≥100 K) |
| S2 — ±2 mm / ±10° | diagnostic | fail | Φ 12,668 → **1,137.59**, `max_g` **16.77 mm** — *worse* than the previous round's 362.36 / 9.13, from a bit-identical entry |
| C168 — squeezed to 168.484 | diagnostic | fail | Φ 2,117.88 → **1,139.40**, `max_g` **11.40 mm** — worse, but from a *different* entry (2,117.88 against 1,932.92), so not a clean comparison |
| random-T — uniform throw, 8 jumps | diagnostic | fail | Φ 182,899 → **4,207.06**, `max_g` **20.74 mm**; 8 jumps, **8 installed**, 7 improving — *better* than the previous round's 8,688.3 / 28.29, from a bit-identical entry |
| 10,000-state corpus | diagnostic | **pass** | 0 outside band, 0 containment false-feasible on **589**, 0 incremental mismatches, **5001/5001** |

`GATE0_PASS: false`, `fatalFailures: ["C175"]` (`evidence/gate0-rerun.json`).

### 1.1 What §0 says about each of them

**S0, soundness, throughput — the regression floor holds.** S0 is bit-for-bit.
The corpus clauses are identical to the previous round in every field, on both
populations — which is expected and is worth stating: mixed-61 has `sag = 0`,
so the clearance split is a no-op there to the last bit. All four throughput
thresholds hold with 5x–10x margin.

**triangle-20 — the MUST PASS passes, and no ladder step is reached.** §0.2's
failure ladder is not entered at all: there is no residual left to attribute.
The canary ends at Φ = 0 with **zero** active rows — 0 pairs, 0 boundaries, on
every side — and publishes a dual-valid child 0.5 µm inside its locked strip
with one 5 µm repair row and zero giveback, at proposal 780 of its 200,000.
The clearance split alone did it; the jump that fired was a ball jump at
`max_g = 0.25 µm` and was not what freed the cell.

Three of the four attempts on the way were **refused by the target-immutability
guard** — *"repair would have enlarged the locked strip; the target is
immutable"* — which is the invariant working, in the one round where it had
something to refuse.

**S1 — not predicted, and it republishes.** §0.3's first requirement is
verified in the document rather than argued: `jumpAttempted: 1`,
`jumpCommitted: 1`, `jumpEvents[0].kind: "ball"`. The **strip jump never
fired**; the scale gate chose the ball branch at `max_g = 0.0336760 mm`, which
is below 0.100 mm, and drew its 16 candidates in a ball of radius
`max(4 · max_g, ladder_top) = 1.25 mm` with angular radius `rho / R`.

The jump was a deliberate step **backwards** — guided Φ 0.005559 → 0.070313,
`improvedGuided: false` — and the descent then reached Φ = 0 and republished
with **zero** repair. Under the previous round's commit rule that jump would
have been refused, and §0.3's second branch would have been the verdict.

**C175 — not predicted, and it is 0/3.** §0.4's two escape clauses both have to
be checked, and they give opposite answers:

* *"If Φ explodes as under the old `always`: the local sweep is still not a
  sweep."* **It does not explode.** The old unconditional arm put two seeds at
  Φ 925 and Φ 3,359; this run ends at **31.29 / 50.91 / 33.11**, which is at or
  better than the old *guided* arm's 40.15 / 38.59 / 37.99 on two of three
  seeds, from a **higher** entry (543 against 461, because the corrected shock
  order compresses an already-perturbed parent). The local sweep is a sweep.
* *"If 0/3 with a real INSTALLED jump on trajectories stalling above 0.1 mm:
  the family's separator fails."* Seeds 0 and 2 stalled at `max_g` 2.27 and
  2.42 mm, took the **strip** branch, and **installed** the relocation
  (`jumpCommitted: 1` on both). Seed 1 never stalled at all: `guidedStalls: 0`
  over **3,934** sweeps, `weightUpdates: 0`, `maxGuidedPenalty: 0`. Every one of
  its sweeps strictly improved raw Φ, so the stall ladder never fired, no jump
  was ever licensed, and it was still descending when the quota ran out at
  1.59 s of its 2.0 s. Seed 1 is a **budget** failure; seeds 0 and 2 are stalls.

So the literal antecedent of §0.4 is satisfied on two of three seeds. **§2
explains why this document nevertheless does not call C175 paradigm
evidence**, and the reason is not a knob and not a re-run: it is a fourth
implementation defect, in the move set itself, that this round's own new
instrumentation surfaced and that this round did **not** fix.

---

## 2. The finding this round did not go looking for

The rejection census both reviews demanded was built to answer Sol review 15
§A.3's question about the guided weights. It answers that one — and it points
at something else on the way.

### 2.1 What the census says

On C175 seed 0, at the stall, over the 32 sampled rejections:

| quantity | value |
|---|---|
| direction class of **every** proposal, accepted or rejected | `combined` — 95,578 of 95,578 |
| `activeIncidentPenaltyMax` on every sampled rejection | **0**, while the layout's `maxGuidedPenalty` is **226** |
| rotational share of the SE(2)-normalized direction | **0.776 to 1.000** |
| Δ(incident guided) at the smallest rung, 0.25 µm | positive on all 32, and **linear in the step** from 1.25 mm down |
| proposals that returned before forming a gradient | **144,396** of 240,000 (60 %) |
| acceptance rate among proposals that did form one | **3.8 %** (3,629 accepted, 91,949 rejected) |

The second row is Sol's point, measured: the guided escalation reached weight
226 somewhere in the layout and **never reached a single row incident on the
pieces that were being refused**. `maxGuidedPenalty` was never evidence that
the blocking row had been escalated, and now it does not have to be.

The fourth row needs care, because half of it is a tautology: a *rejected*
proposal is by definition one where Δ ≥ 0 on every rung, so "Δ > 0 at the
smallest rung" cannot by itself be a discovery. What is not tautological is
that Δ is **proportional to the step** all the way down — 9.07, 3.76, 1.686,
0.795, 0.386, 0.190 … 0.000857 as the rung halves. A first-order ascent
coefficient. A correct steepest-descent direction has first-order coefficient
`−|∇|`, so rejections at the 0.25 µm rung should be nearly impossible, and here
they are 96 % of the population.

### 2.2 The mechanism, read off the code and measured on the fixtures

`incident_gradient` takes the torque about the piece's transformed **centroid**
(`arm = witness − centroids[piece]`), which is Sol review 14 §1's
`tau = (p − c) x (w v n)`. The pose parameterization rotates about the pose
**origin**: `apply_pose` is `p = R(theta) · point + (tx, ty)`, so
`dp/dtheta = z x (p − t)`, and the pivot is `(tx, ty)`.

Those are the same point only when the source ring's centroid sits at the
source origin. On this campaign's fixtures it never does:

| fixture | \|centroid − pose origin\| | `R` (circumradius from centroid) | ratio |
|---|---:|---:|---:|
| mixed-61 (61 pieces) | 21.21 – 92.91 mm | 21.21 – 92.91 mm | **1.00 – 1.35** |
| triangle-20 (20 pieces) | 53.15 mm | 40.31 mm | **1.32** |

So every rotational step carries an unmodelled rigid translation of
`|centroid − origin| · dtheta`, which on **every piece of both fixtures** is at
least as large as — and up to 1.35 times — the rotational displacement
`R · dtheta` that the gradient does model. The direction the ladder walks is not
the direction the gradient computed, and the unmodelled part is the same size
as the modelled part or bigger.

This is a defect of exactly the class the two reviews were hunting — a
force model and a coordinate that disagree — and neither of them named it. It
is a candidate explanation for the SE(2)-coupling "inefficiency" both reviews
observed and declined to promote: triangle-20's frozen-θ probe accepting
66,863 moves against 175 in the previous round is what an incorrect rotational
direction looks like from the outside.

### 2.3 What was done about it: nothing, on purpose

The fix list is frozen and this was found **after** the re-run numbers were in
hand. Fixing it now and re-running C175 would produce a number that is not a
pre-committed measurement of anything — which is the precise failure mode §0
exists to prevent, and the one the autopsy round was called to correct. It is
therefore **reported and not repaired**, and it belongs in the next round's
frozen list, not this one's.

**And it is a reading plus two measurements, not a proof.** The two
measurements are the offset table above and the census's step-scaling; the
reading is the four lines of `incident_gradient` and `apply_pose` quoted. What
would settle it is a unit vector — "a step of `s` along the SE(2) direction
lowers the incident guided energy for small `s`" — and that vector cannot be
committed in this round, because on this code it would be **red**. Writing it
is the first thing the next round should do, before the fix, so that the fix
has something to turn green.

### 2.4 Why it changes C175's reading and not S1's or triangle-20's

S1 and triangle-20 **passed**, so a defect that makes the move set weaker than
specified cannot have manufactured their passes.

C175 failed, and §0.4 would read that failure as "the family's separator
fails". This document declines to draw that conclusion, for the same reason
Sol review 15 and Grok review 10 declined to accept the previous round's:
**a failure measured on a move set that is not the specified one is not
evidence about the specified one.** The honest verdict is in §3.

### 2.5 Two frozen-θ probes, one of which Grok asked for by name

Grok review 10 Finding 3: *"There is **no** S1 frozen-θ probe; do not promote
coupling to the cause."* There is one now, and it does not promote coupling to
the cause:

| probe | Φ out | `max_g` | published |
|---|---:|---:|:--:|
| S1, rotation on, derived commit rule | **0.0** | **0.0** | ✅ 150.16536 |
| S1, rotation on, `--jumpcommit=guided` | 0.000448 | 0.012635 | ❌ |
| S1, rotation **off**, derived commit rule | 5,039.1 | 61.21 mm | ❌ |
| S1, rotation **off**, `--jumpcommit=guided` | 20.63 | 1.438 mm | ❌ |

Freezing θ makes S1 **dramatically worse**, which is what it should do: the
cell's own perturbation is ±2° of rotation and only rotation can undo it. So
§2.2 is not "rotation is useless" — it is "the rotational *direction* is wrong,
while rotation itself is necessary". Both statements have to survive together
and they do.

The C175 frozen-θ probe is **not** a clean comparison and is recorded with that
caveat: `--rotation=off` also freezes the constructor, so `D₀` and therefore
`T = D₀ − 0.10(D₀ − L)` move (198.629 instead of 176.262). At that easier
target the frozen-θ run drives Φ to **2.68e-6** and `max_g` to **0.725 µm** with
30,371 accepted moves against 3,629 — but it is a different cell and is not
offered as evidence about this one.

---

## 3. The verdict

**Gate 0 does not pass. The STOP stands, on C175 alone, and this document does
not upgrade it to a family kill.**

* Two of the three fatal failures were **implementation defects**, exactly as
  both refuters argued. triangle-20 and S1 now publish dual-valid children
  inside their locked strips, with 5 µm and 0 µm of repair, at proposals 780
  and 6,710 of their 200,000 quotas.
* C175 is **0/3 and unresolved**. Its literal pre-committed reading says
  "paradigm evidence"; §2 says the move set that produced that 0/3 has a
  gradient whose rotational component is taken about the wrong pivot on 100 %
  of the pieces in the fixture. Calling the family dead on that measurement
  would repeat this campaign's own most recent mistake.
* The next round's frozen list should therefore contain **one** item — the
  torque pivot — and re-run C175 against it. If C175 is still 0/3 with a
  correct SE(2) gradient and an installed jump, that is the separator, and
  there is nothing left to blame.

Nothing in this round widened a band, raised a budget beyond §0.6's derivation,
granted extra jump allowance, or disabled sag.

### 3.1 What the three diagnostics cost

S2's entry is **bit-identical** across the two rounds (Φ 12,668.141430765654,
same `perturbedPoseDigest`), so its comparison is clean and it is **worse**:
362.36 → 1,137.59 of raw Φ, 9.13 → 16.77 mm of `max_g`. Its one jump was a
**strip** relocation at `max_g = 9.13 mm`, installed, guided Φ 578.6 → 2,455.1,
and the trajectory did not recover inside its quota. That is the cost of the
unconditional commit at a scale where the strip branch is licensed, and it is
the same trade the previous round measured on its `always` arm — narrowed now
to the cells that actually stall in millimetres.

C168 is also worse but its entry moved too (2,117.88 against 1,932.92, because
the corrected shock order compresses a perturbed parent), so its comparison is
not clean and is not offered as one.

random-T is **better** from a bit-identical entry: 8,688.3 → 4,207.06, and
8 of 8 jumps installed against the previous round's "8 jumps, 4 improving".

None of the three carries a fatal verdict; all three failed before and all
three fail now.

---

## 4. The basin sweep, and the A/B that isolates the cause

`evidence/basin-jump-default.json` and `evidence/basin-jump-guided.json` — the
S0 pin perturbed by a ladder of magnitudes, everything else identical, both
arms run at 200,000 proposals.

| perturbation | entry Φ | derived commit rule | `--jumpcommit=guided` |
|---|---:|---|---|
| 0.005 mm / 0.02° | 0.003 | ✅ 150.16229, Φ 0, 0 µm repair | ✅ identical |
| 0.020 mm / 0.08° | 0.158 | ✅ 150.15664, Φ 0, 5.0 µm | ✅ identical |
| 0.050 mm / 0.20° | 1.567 | ✅ 150.16305, Φ 0, 5.5 µm | ✅ identical |
| 0.100 mm / 0.40° | 9.742 | ✅ 150.16223, Φ 0, 0 µm | ✅ identical |
| 0.250 mm / 1.00° | 90.190 | ✅ 150.16003, Φ 0, 6.0 µm | ✅ identical |
| **0.500 mm / 2.00°** (S1) | 433.492 | ✅ **150.16536, Φ 0, 0 µm** | ❌ Φ 0.000448, `max_g` **0.012635** |
| 2.000 mm / 10.0° (S2) | 12,668 | ❌ Φ 1,137.59 | ❌ Φ 900.41 |

**The basin moved.** It ran from 0.25 mm to 0.5 mm before; it now includes
0.5 mm / 2.0° and stops between there and 2.0 mm / 10°.

**And the guided arm reproduces the previous round's failure to the last bit.**
At 0.5 mm / 2°: raw Φ `0.0004482304876309415`, `max_g`
`0.012634958179553735`, depth `150.1729903315535` — every digit of
`../evidence/cell-s1.json`. The A/B therefore isolates the cause exactly: with
the same clearance split, the same real `n`-piece sweep and the same two-scale
gate, the *only* difference between republishing and freezing at 12.635 µm is
whether the jump is allowed to commit a candidate that does not improve guided
Φ. It attempted **169** jumps under `guided` and installed **none**.

That is Grok review 10 Finding 2's claim, measured: *"the mechanism was
evaluated once and discarded."*

One honest cost of the new allowance rule, visible in the same document: a
`guided` run no longer spends its one-shot on a refusal, so it re-fires every
second stall — 169 attempts × 16 candidates × a 61-piece sweep is **164,944**
of that trajectory's 200,000 proposals. The default arm spends 976.

### 4.1 The per-side census answers the previous round's open question

The verification round could not decide whether triangle-20's stall was
"one global translation away", because the instrument did not decompose the
rows by side. The canary no longer has a stall to decompose — but S1's
reproduced fixed point does, and the answer for it is **no**:

    bottom: 1 row at 12.635 µm      top: 1 row at 7.520 µm
    left: 0     right: 0            pieces squeezed on opposite sides: 0

A bottom row and a top row are violated at once, on **different** pieces. No
single rigid translation satisfies both. The two rows are not on the same
piece, so this is not an opposite-side squeeze either — it is the cooperative
compaction Grok review 10 Finding 3 described, and the census now says so in
numbers.

---

## 5. Determinism: two processes, every cell

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

The determinism claim itself is narrowed this round, in the code and here: the
pose transform is `std::f64::sin_cos`, **not** `libm`. Identity with the
publication transform is the stronger requirement and is the one taken, so the
contract is a same-box, same-toolchain, same-target contract. No cross-platform
`sin`/`cos` identity is claimed.

---

## 6. The FAST tier is green

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

`SMOKE_PASS: true`, `INVARIANTS_PASS: true`. The red stage in the previous two
rounds was S1's mechanism clause; S1 republishes, so it is green. That is the
two-tier discipline working in the other direction: it went green when, and
only when, the cell it was watching came back.

---

## 7. The round-boundary battery

Run from the clean committed tree, on binaries rebuilt from it.
`drivers/heavy.sh`, `FAILURES=0`.

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
verification round's base binary**, which is the check that matters here: this
round changed `search::overlap_ics` substantially and the default build did not
move one byte. The `meas` binary is `2fc9ac2c00511ca8…` and differs from the
previous rounds', as it must.

### 7.2 The suites

`evidence/suites.json` and `evidence/suite-*.log`. All `--release`, every exit
status read directly on the line after its command, no pipelines. `SUITES_PASS:
true`; no suite tripped the campaign's known allocator flake, so no rerun clause
fired.

| # | features | targets | passed | failed | ignored | exit |
|---|---|---:|---:|---:|---:|---:|
| 1 | `jagua-experimental` | 55 | 1293 | 0 | 2 | **0** |
| 2 | the protocol's full combo | 55 | 1357 | 0 | 2 | **0** |
| 3 | `jagua-experimental`, `--example general_request_benchmark` | 1 | 20 | 0 | 0 | **0** |
| 4 | `jagua-experimental,overlap-ics` | 55 | **1342** | 0 | 2 | **0** |
| 5 | `overlap-ics` alone, `--lib --tests` | 50 | **1152** | 0 | 0 | **0** |

Suites 1, 2 and 3 are unchanged from both previous rounds. Suites 4 and 5 are
each **+2** (1340 → 1342, 1150 → 1152): the two sag-specific vectors this round
added, and nothing else.

`run-suites.sh` writes its five logs into the previous round's committed
`evidence/`, so `heavy.sh` copies them here and restores the previous round's
with `git checkout` — the same procedure the verification round used, so all
three rounds' logs exist and none overwrote another.

---

## 8. Reproduction

```sh
bash docs/experiments/overlap-ics/gate0-rerun/drivers/rerun.sh   # every cell + basin + determinism
bash docs/experiments/overlap-ics/gate0-rerun/drivers/heavy.sh   # gates + suites, round boundary
ICS_ROOT=<worktree> bash docs/experiments/overlap-ics/drivers/fast.sh
```

Do **not** pipe any of them into `tee` or `tail`: you would read the pipe's
status instead of the script's.

The committed documents are the drivers' output with one renaming, following
the previous round's convention: `s0.json` is committed as `cell-s0.json` and
so on, so the two rounds' evidence files line up by name.
