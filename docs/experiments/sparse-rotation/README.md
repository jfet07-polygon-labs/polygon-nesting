# Sparse rotation: the operator costs 1.06x instead of 2.12x, proposes a fifth as often, is accepted five times as often — and the depth is a null

> The task: take the expressive degree of freedom without the blanket tax. Two
> reviews had converged on the same two verdicts — blanket design A is dead on
> the wall, and a sparse form pays the same per-neighbour price unless the build
> cost is fixed first — so this round fixes the build cost first and then makes
> the arming sparse.
>
> Base commit `09738fb`. x86_64, 16 cores, box shared with other measurement
> agents. Every wall claim is paired and interleaved; every depth claim is
> reported per **seed** as well as per cell, because three rounds of one seed
> are not three samples.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_4111958b-3b3-2` |
| branch | `worktree-wf_4111958b-3b3-2`, on campaign branch `engine/topology-archive-search` |
| base commit | `09738fb` (rotation tax merged; base lands 168.484 at 10 s) |
| governing documents | `docs/sol-review-7-rotation-validator.md` §3.2, `docs/grok-review-2-rotation-compound.md` §4.3, `docs/experiments/rotation-tax/README.md` §5 |
| requests | mixed-61 exact-clearance, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3`; shapes-17; triangle-20 |
| contract | from-request allowance `0.002`; record lineage `0.0005` |
| feature | `sparse-rotation`, off by default, stacked on `continuous-rotation` |
| spec keys | `roteq` (equivariant construction), `sparserot` (design B), `se2w` (design C), `rotbit` (request-adaptive disarm) |
| gate binary (`jagua-experimental`) | sha256 `b6a4ceac21c326d17c0e17115e8788a76258e843ace0615df1b6e98df070920b` |
| measurement binary (full combo + `sparse-rotation`) | sha256 `20728f5ca7acbae7be0d21cc4d0692ff59c1a3efdbb778d9737ba8228918aad5` |
| design-C binary (+ `se2-rigidity-certificate`) | sha256 `2ed2e516bb6e06ec40b9289402137b991638491bdea00b3345e2f849135bff88` |

## The result in one table

Every row is mixed-61 unless it names another request.

| | design A (`crot=1`) | **this round** | |
|---|---:|---:|---|
| µs per rung surrogate build | 4.19 | **1.27** | **3.30x cheaper** (§1) |
| rung surrogates built, per 10 s run | 128,957 | **27,622** | **21.4%** (§2) |
| build wall, per 10 s run | 540 ms | **35 ms** | **15.4x less** |
| **armed mode-34 slice, against the unarmed one** | **2.12x** | **1.064x** | §4.2 |
| rung acceptance, per proposal | 4.47% | **23.36%** | **5.22x** (§2.3) |
| mode-34 slices published, 10 s, 9 runs | 12 of 15 | **31 of 31** | |
| shapes-17: surrogate builds for +0.000 mm | 355,404 | **0** | §4.3 |
| triangle-20: rotation iterations for +0.000 mm | 1,336,518 | **0** | §4.3 |
| mixed-61 at 10 s, paired against each arm's own base | +6.735 mm worse † | **−0.290 mm, 4 of 6 seeds** | §4.1 |
| mixed-61 at 30 s, paired against each arm's own base | +5.736 mm worse † | **+1.483 mm worse, 1 of 6** | §4.1 |
| **depth at equal work, 36 paired cells, one binary, one session** | **+0.005 mm** | **−0.077 mm, 24 of 36** | §6.3 |

† **The design-A column is cross-round on every row above the last, and the two
depth rows marked here are the ones where that matters.** They are read out of
`docs/experiments/rotation-tax/evidence/curves-summary.json`, measured in a
different session against a base arm whose own median differed from this
session's by 2–5 mm (§7.2). The counts and rates are comparable across sessions;
the wall-budget millimetres are not, which is why the **last** row exists: §6 runs
design A and this round's arm on *one binary in one session* at equal work, and
that is the only line in this table where the two columns are the same
measurement.

The mechanism claims are large and they reproduce. **The wall-budget depth claim
does not carry.** §4.1 is why: at ten seconds the seed-level median is −0.290 mm
and the within-seed spread reaches 4.0 mm, so the sign is not resolvable at six
seeds; at thirty seconds the operator loses on five of six. This is an honest
null at the budget the user priority names, and a loss above it.

---

## 0. What the two reviews asked for, and what each item cost

Sol review 7 §3.2 ranked "rotazione sparsa witness-driven/m33, con audit iniziale
e disarmo request-adaptive" as the round's second spend. Grok review 2 §4.3 put
the same item last and attached the condition that makes it hard: *"B/C pagano
la stessa tassa al primo accept se la keying resta overflow-di-griglia"* — a
sparse operator still pays the per-rung build price, so sparsity alone buys
nothing unless the build is fixed first.

Both are addressed, and they are addressed in that order:

* **§1 — the equivariant lever.** `docs/experiments/rotation-tax/` §5 named it,
  priced it at "roughly four times cheaper per rung", and deliberately did not
  take it because it changes the surrogate's geometry. Measured here at
  **3.30x** on the wall and given the matched-arm quality battery that licence
  required.
* **§2 — design B.** Rotation is proposed only for the pieces a stalled
  compression-schedule step names through its own violating-pair queue.
* **§3 — design C.** The SE(2) certificate as a proposal source. Priced. It
  fires, it costs 1.25 ms a call, and it has never once been accepted.
* **§5 — the request-adaptive disarm.** Built, and **it never fires**, because
  design B's own trigger already does its job (§4.3).

---

## 1. The equivariant lever

### 1.1 The construction

A miter join is built from the two incident edge normals and both rotate with
the ring, so in exact arithmetic `offset(T(p)) = T(offset(p))` for any `T` in
`O(2)`. The operator therefore offsets each piece **once**, at zero degrees and
unmirrored, and derives every rung's surrogate by transforming the already
offset ring — `build_oriented_surrogate_equivariant`, against the per-rung
`build_oriented_surrogate`. Both share `finish_oriented_surrogate`, so the only
stage that differs is the one that is replaced.

It is **not** bit-identical, and the reason is the grid rather than the algebra.
The per-rung order snaps the *rotated source* onto Clipper's integer grid and
miters there; this order miters on the unrotated grid and snaps the rotated
result. `the_equivariant_surrogate_is_the_offset_one_up_to_the_grid` brackets
the difference from both sides: bounds agree to within four grid units (the
observed maximum is **one**, 1 µm) and areas to 1e-3 relative, and the collision
rings are asserted **not equal** — because if they were, the battery below would
be measuring nothing.

The transform can fail where the per-rung order would not: an offset ring
carries more vertices, they sit closer together, and rotating them onto the grid
can collide two of them, cross an edge, or exceed `GENERAL_MAX_RING_VERTICES`.
Every such piece falls back to the per-rung offset **permanently** — a ring that
does not survive rotation at one angle keeps not surviving it, and paying a
failed transform before every offset would make the lever a pure loss on that
piece. `rotationEquivariantFallbacks` counts it.

### 1.2 What it costs, measured

Over the whole ten-second battery on mixed-61 — 9 runs, 31 armed mode-34 slices,
`evidence/curves-summary.json`:

| | value |
|---|---:|
| rung surrogates built | 248,602 |
| ... built by the equivariant construction | **248,602** |
| **coverage** | **100.0%** |
| pieces routed back to the per-rung offset | **0** |
| wall inside the build | 315.95 ms |
| **µs per build** | **1.271** |

Against `docs/experiments/rotation-tax/` §1.2's own measurement-binary rate of
**4.19 µs**, that is **3.30x**. The README's estimate was "roughly four times
cheaper, not exactly 5.4x", derived by removing 4.71 µs of Clipper and adding a
transform on a ring with more vertices than the source. The estimate was right
and slightly optimistic.

At thirty seconds the same numbers reproduce: 1,153,723 builds, all equivariant,
0 fallbacks, **1.211 µs** per build.

**Zero fallbacks on 1.4 M builds across three requests** is worth stating
plainly, because it is the risk the fallback exists for and it did not
materialise once on this corpus. It is not a proof that it cannot: the fallback
is kept, counted, and reported in every table.

### 1.3 The quality battery the licence required

See §6. The lever changes the surrogate's geometry, so it gets a matched-arm
comparison at equal work on the twelve pinned 171–179 mm parents rather than an
answer-preservation claim.

---

## 2. Design B: rotation when the clamp binds

### 2.1 The trigger

Inside `run_compression_schedule`'s repair loop, a **stall** is a sweep that
left the frontier infeasible *and* did not lower the common loss it was handed.
At that point — and only then — the pieces named in `PairTracker::collision_pairs`
are armed for the remaining sweeps of that step. A sweep that does lower the
loss disarms again, and every step begins and ends disarmed, so an episode
cannot outlive the stall that opened it.

This introduces **no new selection logic**: `collision_pairs` is the same queue
the schedule's repair tier (d) already orders the next sweep's active set from,
and `piece_is_active` already reads it. The mirror companion honours
`allow_rotation && allow_mirror` — the defect
`docs/experiments/continuous-rotation/` §3.3 fixed — and design B additionally
drops a piece that can do neither *before* counting the episode, so the reported
episode width is the width that could actually propose something.

The fan-out repair runs the identical detector on each worker's private
frontier, reading nothing outside `(lane_state, lane_score, lane.weights)`, so
the reduce stays a total order over the same eight computations. §7.3 is the
two-process determinism measurement that holds it.

`sparse_rotation` is scoped to **mode 34 alone**, and a lane running it has
`continuous_rotation` withdrawn on mode 22. Mode 22 has no schedule, no clamp
and no step to stall — it was 85% of design A's 1.13 M builds
(`docs/experiments/rotation-tax/` §0) and there is no trigger there to be sparse
about.

### 2.2 How sparse it actually is

mixed-61, ten seconds, 9 runs:

| | value |
|---|---:|
| episodes opened | 10,370 |
| pieces armed, summed over episodes | 27,325 |
| **mean episode width** | **2.635** of 61 pieces — **4.3%** |
| repair sweeps that ran with an episode open | 8,264 |
| rung surrogates built | 248,602 |
| ... design A's, same request, same budget, same 9 runs | 1,160,616 |
| **share of design A's builds** | **21.4%** |

Read the third and last rows together. The operator is offered to 4.3% of the
pieces and pays 21.4% of design A's builds — sparsity in the *pieces* is not
sparsity in the *builds*, because the pieces a stall names are exactly the ones
a descent then works hardest on. That is the mechanism doing what it was
designed to do, and it is also why §1 had to come first: at design A's 4.19 µs
a fifth of the builds would still have been 108 ms a run.

### 2.3 The rungs are accepted five times as often

Both columns are the same nine ten-second mixed-61 runs, design A's read out of
its own committed evidence (`docs/experiments/rotation-tax/evidence/curves-summary.json`)
rather than restated:

| | design A | design B |
|---|---:|---:|
| rung + mirror proposals | 1,806,090 | 315,436 |
| improvements | 80,787 | 73,678 |
| **acceptance per proposal** | **4.47%** | **23.36%** |
| acceptance, `improved / (proposals / 2)` | 8.95% | 46.72% |
| rotation's share of proxy loss removed | 56.4% | 31.8% |
| accepted moves that changed the pose | 68.9% | 9.8% |

Both denominators are printed because Grok review 2 §2 is right that
`improved / (proposals / 2)` is `2p` and not `P(at least one improvement)`. On
either convention the ratio between the arms is the same **5.22x**.

Note the second row: design B produces **91% as many improvements as design A
did, from 17% as many proposals.**

The two rows that *fall* are not a regression, they are the design: rotation
removes a smaller share of the proxy loss and changes a smaller share of
accepted moves because it is proposed a quarter as often. What matters is that
each proposal is five times likelier to be worth making, which is the whole
claim "witness-driven" was making.

The cache-hit rate is **44.9%** at ten seconds, against the corrected
`docs/experiments/rotation-tax/` §4.4 figure of **54.0%** for design A on the
same request — and *not* against the retracted 89.4%, which counted
re-confirmation hits nothing needed. Sparse arming makes the window slightly
worse, which is expected: fewer, more scattered proposals reuse less.

---

## 3. Design C: the witness, priced

`se2_witness_proposal` runs the rewritten certificate's **one** program a search
can use — depth-only × SE(2) — instead of the diagnostic's four, and returns the
moved placements. It fires when design B's stall outlives a whole step, at most
`max_calls` times per slice, and never twice on an unchanged floor.

It runs on `confirmed_state`, not on the frontier, and that is a requirement:
the certificate's line search ends at `scale = 0` — the parent itself — and
asserts that rung validates, so an infeasible frontier makes it error rather
than answer. What comes back is already accepted by `validate_publication` at
the scale the line search settled on, and is then re-measured through
`coupled_independent_source_depth` on the untouched source rings, because that
is the number this mode publishes on.

### 3.1 It is affordable — three orders of magnitude cheaper than the diagnostic

`drivers/witnessprice.py`, twelve pinned parents, one serial mode-34 slice each
at the design-slice work cap, design B on the equivariant construction in every
arm and the only difference the witness budget. `evidence/witnessprice.json`:

| budget `trust:iters:maxcalls` | calls | ms/call | median share of the slice | max share |
|---|---:|---:|---:|---:|
| `0.025:64:2` | 22 | **1.42** | **0.18%** | 0.60% |
| `0.1:64:2` | 22 | 1.65 | 0.20% | 1.09% |
| `0.25:64:4` | 42 | 2.35 | 0.53% | 1.83% |

`docs/experiments/se2-rigidity/`'s "every certificate call is ≤ 1 s" was a
**four-program** call at **20,000** iterations. `se2_witness_proposal` solves one
program at 64, which is about 1,250x less work, and the measurement agrees:
1.4 ms against a slice whose `repairMs + confirmationMs` is 424–4,292 ms.

Design C is not too expensive. That was the round's prior and it is **retracted**.

### 3.2 It is accepted, exactly validated — and dominated

| budget | calls | **accepted** | mm bought against the running incumbent | **cells where the final depth moved** |
|---|---:|---:|---:|---:|
| `0.025:64:2` | 22 | 6 | 0.134 | **0 of 12** |
| `0.1:64:2` | 22 | 7 | 0.473 | **0 of 12** |
| `0.25:64:4` | 42 | **16** | **2.714** | **0 of 12** |

The witness fires, the exact validator signs it, and it lowers the published
depth *at the moment it is applied* — 0.976 mm on seed 8 alone, over four
accepted calls. And the slice's final published depth is **bit-identical to the
witness-off arm on all twelve parents**, at every budget.

Everything the certificate finds, the compression schedule's own step-down finds
anyway. That is not a surprise once the two are put side by side: the schedule
walks ~1,600 canonical grid steps of 1 µm per slice, which is 1.6 mm of depth,
while the witness offers one jump bounded by its own trust radius.

`drivers/witnesscurve.py` measures that bound directly, on the same twelve
parents through the **independent** `POLYGON_NESTING_SE2_CERTIFICATE` diagnostic
path, at trust 0.025 (`evidence/witnesscurve-reduced.json`):

| iterations | median validated `δ` | positive cells | median `scale` | SE(2) beats translation | median per-program wall |
|---|---:|---:|---:|---:|---:|
| 64 | **0.02507 mm** | 12 of 12 | 1.00 | 8 of 12 | 2.4 ms |
| 500 | 0.02511 mm | 12 of 12 | 1.00 | 8 of 12 | 4.3 ms |
| 2,000 | 0.02511 mm | 12 of 12 | 1.00 | 8 of 12 | 11.0 ms |
| 20,000 | 0.02511 mm | 12 of 12 | 1.00 | 8 of 12 | 74.5 ms |

**Two retractions are owed here.** The first: this round's working hypothesis
was that 64 iterations would be too few to produce a usable direction. It is
wrong — the witness at 64 iterations is within 4e-5 mm of the witness at 20,000,
`scale` is 1.0 (the model's full step survives exact validation), and the
certificate converges long before a slice's budget is threatened. The second:
"0 accepted on every cell", written from the single seed-0 smoke probe in
`evidence/se2probe.json`, was false as a general statement and is corrected by
the twelve-parent table above.

The real finding is sharper than either. The translation-only column returns
**exactly 0.025000 mm** — the trust radius, to six digits — so at a radius small
enough for the linearization to hold, the witness's answer *is the box*. SE(2)
adds 0.0001–0.0120 mm over translation on 8 of 12 parents, which reproduces
`docs/experiments/se2-rigidity/` §4.2's "1.5–1.8x, and only at small radii" in
absolute terms. A one-shot 0.025 mm jump cannot compete with 1,600 steps of
1 µm walking, and widening the box past the crossover buys model error rather
than depth.

**Design C is therefore a null for a reason that is not its own cost.** It is
cheap, it is sound, its witnesses are exactly validated, and the operator it was
meant to help already reaches everywhere it can point. It ships wired, off by
default, with the price and the domination both recorded, because the parents
where it might not be dominated — a lane that cannot step, or a front the clamp
cannot walk into — are not in this corpus.

---

## 4. The anytime battery

`drivers/run-battery.sh` → `drivers/battery.py`. Both arms carry the 168.484
configuration — `fast-contract-validator` compiled in with no spec key,
`m34lanes=1,m34pconfirm=1`, `m34wall` and `m34bit` at their v3 defaults — and
the only keys that differ are the operator's:
`crot=1,sparserot=1,roteq=1` against `crot=0`. Three requests × three seeds ×
three rounds × 3/10/30 s × two arms, plus three further seeds on mixed-61 at 10
and 30 s. `evidence/curves-summary.json`, `evidence/pool-*.json`.

The statistic is the per-round paired difference in published depth, armed minus
base, so **a negative number is the operator winning**.

### 4.1 The verdict

| request | 3 s | 10 s | 30 s |
|---|---|---|---|
| **mixed-61** (6 seeds) | +0.000 mm, 0 of 9 | **−0.290 mm, 4 of 6 seeds** | **+1.483 mm worse, 1 of 6 seeds** |
| shapes-17 | +0.000 mm, 0 of 9 | +0.000 mm, 0 of 9 | +0.000 mm, 0 of 9 |
| triangle-20 | +0.000 mm, 0 of 9 | +0.000 mm, 1 of 9 | +0.000 mm, 0 of 9 |

**The ten-second cell does not carry, and the reason is in the spread rather
than in the median.** Per seed, `evidence/pool-10s.json`:

| seed | base | armed | paired median | within-seed spread, base / armed |
|---|---:|---:|---:|---|
| 0 | 170.453 | 169.835 | **−0.618** | 0.658 / 3.651 |
| 1 | 165.656 | 169.007 | +3.351 | 0.000 / 1.474 |
| 2 | 174.280 | 170.376 | **−3.904** | 0.000 / 0.000 |
| 3 | 175.085 | 175.085 | +0.000 | 4.670 / 2.705 |
| 4 | 172.129 | 171.650 | **−0.479** | 0.479 / 4.039 |
| 5 | 179.633 | 179.533 | **−0.100** | 0.000 / 0.013 |

Four of six seeds favour the operator, one is exactly equal and one is against
it; the median of the seed medians is −0.290 mm and the mean is −0.292 mm. But
the range is [−3.904, +3.351] and the *within-seed* spread reaches 4.039 mm on
one arm of one seed. An effect of 0.29 mm inside a noise band of 4 mm is not an
effect. The honest statement is: **at ten seconds the sparse operator is
indistinguishable from the base arm, in a direction that is mildly favourable.**

The round's stated success criterion was "mixed-61 10 s better paired". On the
count it is met — 4 seeds of 6, 10 cells of 18 — and on the evidence it is not,
because the same table shows the instrument cannot resolve the sign. It is
recorded as a **null**, not a win.

At thirty seconds it is resolvable and it is against the operator: +1.483 mm
median, base better on **5 of 6 seeds**, `evidence/pool-30s.json`. The one seed
that favours the operator does so by 0.028 mm.

At three seconds neither arm runs a mode-34 slice at all on any request, so the
3 s row measures nothing about rotation. That was true of the previous round too
and is stated again because a table of three budgets invites reading it as
three measurements.

### 4.2 The wall, which is where the round's result actually is

Armed mode-34 slice wall against the unarmed one, aggregated over every slice
each arm ran:

| request, budget | armed s/slice | base s/slice | ratio |
|---|---:|---:|---:|
| **mixed-61, 10 s** | 0.8471 | 0.7959 | **1.0643x** |
| mixed-61, 30 s | 1.1318 | 1.1333 | 0.9986x |
| shapes-17, 10 s | 1.0633 | 1.0844 | 0.9805x |
| shapes-17, 30 s | 1.0656 | 1.0789 | 0.9876x |
| triangle-20, 10 s | 1.7567 | 1.8178 | 0.9664x |
| triangle-20, 30 s | 1.8644 | 1.8300 | 1.0188x |

`docs/experiments/rotation-tax/` §4.2 measured design A's armed slice at
**2.12x** the unarmed one on mixed-61 at ten seconds *after* its own tax fixes,
and that ratio is why design A got 15 slices where the base arm got 40. This
arm gets **31 of the base's 37** at ten seconds and **57 of 60** at thirty. The
throughput loss that dominated the previous round is gone.

Which is what makes §4.1 the finding it is. The operator is now nearly free, it
is accepted five times as often, and the depth still does not move. The tax was
never the whole story, and this round is what establishes that: with the tax
removed, the remaining question is quality per action, exactly as Sol review 7's
closing sentence predicted — *"anche recuperando tutta la tassa attuale, il gap
residuo di 19.4 mm non è spiegabile da nanosecondi"*.

### 4.3 shapes-17 and triangle-20: the waste is gone, and the disarm bit is not why

Both requests run **nine** mode-34 slices in both arms at both budgets, publish
on **0 of 9** in both arms, and the armed arm performs:

| | design A | design B |
|---|---:|---:|
| shapes-17, surrogate builds at 10 s | 355,404 | **0** |
| triangle-20, rotation iterations at 10 s | 1,336,518 | **0** |
| shapes-17, armed s/slice | 1.370 | **1.063** (base 1.084) |
| episodes opened | — | **0** |

Design A bought 355,404 surrogates and threw them away; design B does not buy
them, and the slice returns to parity with the unarmed one.

**The request-adaptive disarm of §5 is not what did this.** The bit requires
`episodes > 0 && accepted == 0` to fire, and there were **zero episodes** — so
the bit never had evidence to act on and never fired. What suppressed the
operator is design B's own trigger: these two requests' schedule steps do not
stall, so no violating-pair queue ever names a piece, so no rung is ever
proposed. "No rotation proposals while translation progresses" is not a heuristic
here, it is the control flow, and on two of three campaign requests it is
sufficient on its own.

This is the round's cleanest result and it is a mechanism result rather than a
depth one: **the two requests where design A's entire spend was waste now cost
nothing, without a request-level policy, a prior, or a bit.**

---

## 5. The request-adaptive disarm

`SparseRotationBit` on the coordinator, the same shape as
`schedule_sterile_bit`: one sterile slice fires it, one audition after
`BARREN_ACTION_PATIENCE` further barren calls hands the operator back, and — the
one difference from the schedule bit — a *productive* audition reverses the
verdict rather than exhausting it, because this bit withholds a degree of
freedom inside a class that keeps running rather than withholding the class.
Its evidence is a **sterile slice**: one that opened at least one episode, so the
mechanism had its trigger and its budget, and accepted no rotation move.

`the_sparse_rotation_bit_is_one_sterile_slice_and_a_reversible_verdict` drives
every transition directly, including the one that matters: a slice with **no**
episodes is not evidence either way, because the mechanism never got its
trigger.

**It did not fire once in this round's 162 runs**, and §4.3 is why. It is kept
because the case it protects against — a request where stalls are common and
rotation is useless — is one this corpus does not contain and the campaign has
no reason to think impossible. It is armed by default inside the operator and
`rotbit=0` turns it off; every table above ran with it on, so every table above
is also the measurement that it changed nothing.

---

## 6. Quality at equal work: the matched-arm battery

This is the battery §1's licence required and the cleanest instrument this round
has. `drivers/armgate.py`: one pinned parent, one serial mode-34 slice, the
anatomy's design-slice work cap (3,341,379 units), **five arms on one binary**,
paired and interleaved with the arm order rotated every round. Twelve parents ×
three rounds = **36 paired cells per comparison**. `evidence/armgate.json`.

Work rather than wall is the denomination on purpose: at a fixed work cap every
arm answers the same number of proxy questions, so the operator has to pay for
its rungs in quality rather than in seconds.

### 6.1 Per arm

| arm | depth median | wall median | within-arm relative spread | rung builds | µs/build | episodes | mean width |
|---|---:|---:|---:|---:|---:|---:|---:|
| `base` | 175.7273 | 3.404 s | 1.360 | 0 | — | — | — |
| `crot` (design A) | 175.0331 | 5.377 s | 0.653 | 2,076,027 | 4.310 | — | — |
| `crotEq` (A + lever) | **174.9197** | 3.991 s | 0.887 | 2,401,056 | **0.993** | — | — |
| `sparse` (design B) | 175.7108 | 3.599 s | 1.278 | 561,189 | 5.546 | 22,671 | 2.67 |
| `sparseEq` (B + lever) | 175.7108 | 3.867 s | 1.104 | 523,947 | **1.221** | 20,691 | 2.75 |

**Read the spread before anything else.** The within-arm relative spread is
0.65–1.36 — the twelve parents differ from each other by more than any arm
differs from any other arm — so *nothing* in the wall column of this table is
resolvable across arms. Only the paired numbers below are.

### 6.2 The equivariant lever's quality gate

The gate the task set: *"If the quality gate fails, keep per-rung offsets and
say so."* It does not fail. It passes in the favourable direction, on both
hosts:

| paired comparison | median Δ depth | better / worse / equal | paired wall ratio |
|---|---:|---|---:|
| **`crotEq` − `crot`** | **−0.0401 mm** | **27 / 9 / 0** | **1.276x faster** |
| **`sparseEq` − `sparse`** | **−0.0275 mm** | **18 / 12 / 6** | 1.024x faster |

Negative is better. The equivariant construction is not merely tolerable, it is
*better than the per-rung offset at equal work* — on design A by 0.040 mm and 27
of 36 cells, on design B by 0.028 mm and 18 of 36. Its build rate here is
**4.34x** and **4.54x** cheaper respectively; the anytime battery of §1.2 puts
the same quantity at **3.30x**. Taken together the three measurements straddle
`docs/experiments/rotation-tax/` §5's "roughly four times cheaper per rung",
and the 38% spread between them is not decomposed by this round (§8.1).

Why it is *better* rather than neutral is not established by this round and is
not claimed. The construction is a different operator geometry; two candidate
explanations — a miter computed on the unrotated grid is a more faithful
expansion, or the arms simply walk different trajectories and this is the luck
of 36 cells — are not separated here. What is established is that the gate does
not fail, which is what the licence to ship it required.

### 6.3 The four arms against the base

| paired comparison | median Δ depth | better / worse / equal | paired wall ratio (arm ÷ base) |
|---|---:|---|---:|
| `crot` − `base` | +0.0046 mm | 18 / 18 / 0 | **1.601x** |
| `crotEq` − `base` | −0.0184 mm | 18 / 18 / 0 | 1.118x |
| `sparse` − `base` | −0.0273 mm | 21 / 12 / 3 | **1.023x** |
| **`sparseEq` − `base`** | **−0.0766 mm** | **24 / 9 / 3** | 1.087x |

Two things to take from this table, and a caution.

**Design A reproduces its own previous measurement to the third decimal.**
`docs/experiments/continuous-rotation/` measured design A at **+0.005 mm** at
equal work; this round measures **+0.0046 mm**, on a different binary five
commits later, with an 18/18 split that is exactly the coin flip that number
implies. The instrument is the same instrument.

**Design B on the equivariant construction is the best arm**, at −0.0766 mm and
24 of 36 cells — the only arm that beats the base on a clear majority of cells.

The caution: 0.0766 mm at equal work is **not** 0.0766 mm at a wall budget, and
§4.1 is what happens when the same arm is asked for depth per second instead of
depth per candidate query. Medians of ratios do not compose, either: the wall
column above is a median over paired ratios and the four rows are not required
to be mutually consistent (`sparse` reads 1.023x and `sparseEq` 1.087x against
the base, while `sparseEq` reads 1.024x *faster* than `sparse` head to head).
Each row is the paired statistic for its own pair and none of them is a
composition of the others.

---

## 7. Gates, suites, determinism

### 7.1 The four pinned gates, on four binaries, exits captured directly

`drivers/gates.py`, `evidence/gates-*.json`.

| binary | sha256 (first 16) | g1 206.869 | g2 159.09233022733062 | g3 159.07876040364795 | g4 164.0375677990678 |
|---|---|---|---|---|---|
| `base-gate` (09738fb, `jagua-experimental`) | `602519f6f068abea` | hit | hit | hit | hit |
| `gate` (this tree, `jagua-experimental`) | `b6a4ceac21c326d1` | hit | hit | hit | hit |
| `meas` (full combo + `sparse-rotation`) | `20728f5ca7acbae7` | hit | hit | hit | hit |
| `meas-se2` (+ `se2-rigidity-certificate`) | `2ed2e516bb6e06ec` | hit | hit | hit | hit |

All four raw depths and fingerprints reproduce, and — the stronger check — the
**whole-document digest is identical across all four binaries on all four
gates**:

```
g1 f0a290dbea69425ced0153df46207d7a9e727ef087805d800917b03dfb5d195c
g2 fa563e762c9c0ae1b2b3a8e976c37d6e8eb0ae5523061af50b89a7b70b5a3d6f
g3 6a3bd92bbbf0b62dfa3850bfb3da0cc6bd205a929cf535d8dd6f4dfa06a71cce
g4 7d744ba9e1bf0104322fa5a6c34d1d4699bde31cd2bce7b727561f7e51aadb06
```

The base-commit binary is in the table for a reason: `finish_oriented_surrogate`
is **not** feature-gated. The refactor that lets the two constructions share
their last three stages sits on the hot path of every surrogate build in every
build of this engine, including the catalogue's own. The digest identity above
is what says it changed nothing.

The gates' wall times are not a wall claim and are not quoted.

### 7.2 Flag-off document reproduction against the base commit

`drivers/reproduce.py`, whole documents at a 40 M **work** budget through the
coordinator — work rather than wall so both sides are deterministic and
load-independent — three requests × three seeds, both sides on the plain default
spec so neither names a key.

**9 of 9 identical**, `allEqual: true`, `evidence/reproduce-flagoff.json`. With
`crot`, `roteq` and `sparserot` unset, the measurement binary **is** the base
binary.

This settles a question §4.1 would otherwise leave open. The base arm of this
round's ten-second battery publishes a median of **173.205 mm** over six seed
medians (and 170.453 on seed 0, the seed the previous round's median came from),
where `docs/experiments/rotation-tax/` §4.2's base arm published **168.484 mm** —
a 2–5 mm cross-round difference on the arm that is supposed to be
*unchanged*. The
reproduction above rules out this round's code as the cause: at a work budget
the two binaries produce the same document, field for field. What is left is the
box, the day, and the seed set. The size of that is worth carrying forward: **a
wall-budget median on this fixture moves 2 mm between sessions on the same seed,
and 5 mm when the seed set widens from three to six** — both larger than any
effect this round measured, and the reason §4.1 reports per seed and refuses the
headline.

### 7.3 Determinism across two processes, armed

The hard gate for anything armed. `drivers/determinism.py`,
`crot=1,sparserot=1,roteq=1,m34lanes=1,m34pconfirm=1`, 40 M work budget, three
requests × three seeds, two processes per cell, whole documents.

It matters this round because design B's arming is *per-lane mutable state read
inside a candidate loop*, and because the fan-out repair runs eight private
copies of the stall detector whose winner is chosen by a reduce. Both are
deterministic functions of their own inputs — nothing reads a clock or a thread
id — but that is an argument, and this is the measurement.

**9 of 9 equal**, `allEqual: true`, `evidence/determinism-sparse.json`. The
armed mixed-61 cells publish 168.3 / 170.303 / 170.37571442162204 in both
processes, to the last bit.

### 7.4 Suites

`drivers/run-suites.sh`, **exit status read from `$?` on the line after the
command** rather than through a pipe. Three suites, not the protocol's two,
because the protocol's combo does not name `sparse-rotation` and would therefore
compile none of this round's code.

| suite | targets | result | exit |
|---|---:|---|---:|
| `--features jagua-experimental` | 62 | **1,262 passed, 0 failed** | **0** |
| `--features <protocol combo>` | 62 | **1,300 passed, 0 failed** | **0** |
| `--features <combo>,sparse-rotation,se2-rigidity-certificate` | 62 | **1,320 passed, 0 failed** | **0** |

The first two totals are exactly the rotation-tax round's (1,262 and 1,300),
which is the check that this round added nothing to and removed nothing from the
shipping feature set. The 20-test difference in suite 3 is this round's four
plus the certificate feature's sixteen.

**No rerun was needed.** The campaign's known flake,
`search::layout_scorer::tests::free_material_multi_eviction_shrinks_retained_container_capacity`,
passed on its first attempt in all three suites; it is present in all three logs
and is reported here rather than left unmentioned, because the protocol names it
and a round that did not hit it should say so.

### 7.5 The round's regression tests

Four, all in the suite-3 log above.

| test | what it pins |
|---|---|
| `the_equivariant_surrogate_is_the_offset_one_up_to_the_grid` | §1.1 from both sides: bounds agree to within 4 grid units (observed max 1) and areas to 1e-3 across three angles × both mirror states, **and** the collision rings are not equal — so the quality battery has a subject |
| `sparse_arming_offers_rungs_to_the_violating_pieces_and_to_nobody_else` | §2.1: the lane starts closed, one violating pair arms exactly its pieces, an empty queue is not an episode and is not counted, a wider stall reports the wider width, and the disarm is total |
| `a_lane_without_design_b_is_armed_everywhere_and_stays_that_way` | that the control arm of every table above really is design A: `disarm_rotation` cannot disarm a lane that is not running design B, and such a lane counts no episodes |
| `the_sparse_rotation_bit_is_one_sterile_slice_and_a_reversible_verdict` | §5: one sterile slice fires the bit, a slice with **no** episodes is not evidence either way, the audition is spent once, and a productive audition reverses the verdict rather than exhausting it |

The rotation operator's ten existing tests are unchanged and still pass,
including the two that pin `docs/experiments/continuous-rotation/`'s defects.

---

## 8. Against Sparrow, and what this round retracts

Same box, same request, same 5.0 mm pair clearance
(`docs/experiments/sparrow-mixed61/` x86_64 addendum: 157.971 mm @ 3 s,
150.165 mm @ 10 s, exact-valid at 61/61):

The 3 s row is a median over 9 cells (3 seeds); the 10 and 30 s rows are the
median of the **six** seed medians of §4.1, which is the statistic that round
argues on:

| budget | Sparrow | this round's base | behind | this round's armed | behind |
|---|---:|---:|---:|---:|---:|
| 3 s | 157.971 | 179.587 | 21.6 mm | 179.587 | 21.6 mm |
| **10 s** | **150.165** | **173.205** | **23.0 mm** | **171.013** | **20.8 mm** |
| 30 s | — | 165.647 | — | 166.190 | — |

Every number here is quoted against **this session's own base arm**, not against
`docs/experiments/rotation-tax/`'s 168.484, and §7.2 is why: the two binaries
reproduce each other document for document at a work budget, so the difference
between the two sessions' base medians is the box rather than the engine.
Borrowing the other session's base to claim "18 mm behind" would be exactly the
cross-round comparison Grok review 2 §3c refused for the crot battery, and it is
refused here too.

Note also that the ten-second medians here (173.205 / 171.013) are **medians of
six seed medians**, where §4.1's table is a *paired* statistic. The two answer
different questions and only the paired one is evidence about the operator: the
unpaired medians differ by 2.19 mm in the operator's favour, which is larger
than the paired −0.290 mm, and the gap between those two numbers is the seed
composition rather than anything the operator did.

### 8.1 Retractions and corrections this round owes

* **"Design C's witness needs more iterations than a slice can afford."** The
  round's working hypothesis, stated in an early draft of §3 from a single smoke
  probe. **Wrong.** §3.2: the witness at 64 iterations is within 4e-5 mm of the
  witness at 20,000, `scale` is 1.0 on 11 of 12 parents, and one call is 1.4 ms.
* **"0 accepted on every cell."** Written from `evidence/se2probe.json`, one
  seed. **False as a general statement.** Across twelve parents the witness is
  accepted 6, 7 and 16 times at the three budgets and buys up to 2.714 mm
  against the running incumbent. What is true — and is the actual finding — is
  that the final published depth moves on **0 of 12** cells, because the
  schedule reaches the same layout without it.
* **The equivariant lever's estimate.** `docs/experiments/rotation-tax/` §5
  predicted "roughly four times cheaper per rung, not exactly 5.4x". Measured:
  **3.30x** in the anytime battery, **4.34x** and **4.54x** in the equal-work
  battery. The estimate straddles the three. Why they differ by 38% is **not
  established here** — a replay at a fixed work cap and a from-request run at a
  wall budget do not offer the operator the same pieces, which is a hypothesis
  and not a measurement, and this round did not decompose the rate by piece.
* **Not retracted, and worth saying:** `docs/experiments/rotation-tax/` §4.4's
  corrected cache-hit rate of **54.0%** for design A on mixed-61 is the number
  this round compares against, and design B measures **44.9%**. The retracted
  89.4% is not used anywhere in this document.

### 8.2 What this round establishes, and what it leaves

The tax is gone and the gap is not. That is the result, and it is a more useful
one than a millimetre would have been, because it removes the explanation the
last three rounds have been able to fall back on. Design A lost by 6.735 mm and
the cost was a sufficient explanation; this arm costs **1.06x** a base slice,
proposes a quarter as often, is accepted **5.2x** as often, produces **91% as
many improvements from 17% as many proposals** — and lands on a null. Sol review
7's closing sentence is now measured rather than predicted: *"anche recuperando
tutta la tassa attuale, il gap residuo di 19.4 mm non è spiegabile da
nanosecondi"*.

Three things this round would hand to the next one, in the order it would spend
them:

1. **The 30 s loss is the informative cell, not the 10 s null.** At thirty
   seconds the base arm gets 60 slices and the armed arm 57 at the *same* wall
   per slice, and still loses 1.483 mm on 5 of 6 seeds. Equal cost, equal slice
   count, worse depth: the rungs are moving the search somewhere it does not
   want to go at depth, and `rotationLossShare` rising from 31.8% at ten seconds
   to 40.1% at thirty says where to look.
2. **The equivariant construction is a shipping candidate on its own.** It is
   better at equal work on 27 of 36 cells against design A and 18 of 36 against
   design B, it is 4.3–4.5x cheaper, and it had **zero** fallbacks on 1.4 M
   builds. It is currently reachable only with the rotation operator armed;
   nothing about the construction requires that.
3. **The measurement floor is the binding constraint on this fixture.** §7.2
   puts a ~2 mm session-to-session movement on a wall-budget median that every
   round of this campaign has been quoting to three decimals. Until an
   instrument resolves better than that, no rotation design will be able to
   prove a millimetre at a ten-second wall, and the equal-work battery of §6 —
   which resolves 0.077 mm on 36 paired cells — is the instrument to design
   against.

---

## 9. Files

* `drivers/armgate.py` — §6's equal-work matched-arm gate, N arms on one binary,
  paired and interleaved. The equality check `ablate.py` makes is deliberately
  absent: the equivariant construction changes the surrogate's geometry, so a
  fingerprint mismatch here is the premise rather than a finding.
* `drivers/witnessprice.py` — §3.1's in-search pricing of design C.
* `drivers/witnesscurve.py` — §3.2's out-of-band certificate curve, run through
  the existing `POLYGON_NESTING_SE2_CERTIFICATE` diagnostic so that it shares no
  code with the in-search invocation.
* `drivers/pool.py` — §4.1's seed-level reducer. Collapses each seed's rounds to
  that seed's median first, because nine cells at a wall budget are three
  results repeated.
* `drivers/run-battery.sh`, `drivers/battery.py`, `drivers/summarize.py` —
  §4's battery. `battery.py` now carries `rotationBuildsRefused` and
  `rotationSurrogateCells`, which Sol review 7 §2 identified as silently dropped,
  and `summarize.py` distinguishes a **missing** telemetry field from a zero
  (`missingFields`) rather than converting the first into the second.
* `drivers/run-suites.sh` — §7's three suites, exits read from `$?`.
* `drivers/gates.py`, `gatelib.py`, `reproduce.py`, `determinism.py`,
  `docdiff.py`, `runlib.py`, `workgate.py`, `taxprobe.py`, `smoke.py`,
  `ablate.py`, `binab.py` — from `docs/experiments/rotation-tax/drivers/`,
  `ROOT`/`BIN`/`OUT` repointed at this worktree and otherwise byte-faithful.
* `evidence/*.json`, and the suite logs.

Reproduce:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                                # gate binary
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,compression-schedule,\
parallel-compression-schedule,continuous-rotation,\
fast-contract-validator,sparse-rotation                          # measurement
cargo build --release --example general_request_benchmark \
    --features ...,sparse-rotation,se2-rigidity-certificate      # design C

D=docs/experiments/sparse-rotation/drivers
P=docs/experiments/parallel-compression-schedule/evidence/parents.json

V4_BIN=<meas> V4_OUT=<out> bash $D/run-battery.sh
SUMMARIZE_ARM=sparse python3 $D/summarize.py <out>/curves-summary.json \
    <out>/curve-*/battery.json
python3 $D/pool.py <out>/pool-10s.json 10 <out>/curve-mixed61*/battery.json
python3 $D/armgate.py <out>/armgate <meas> $P 3 base,crot,crotEq,sparse,sparseEq
python3 $D/witnessprice.py <out>/witness <meas-se2> $P 0.025:64:2,0.025:2000:2
python3 $D/witnesscurve.py <out>/curve <meas-se2> $P 0.025 64,500,2000,20000
python3 $D/gates.py <label> <binary> <outdir>
bash $D/run-suites.sh
```
