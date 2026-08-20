# The SE(2) rigidity certificate, rewritten: the right program, and a witness the exact validator signs

Sol review 6 §3 **rejected** the composed branch `sol5/se2-rigidity-certificate`.
Its documentary corrections were good and are cherry-picked here; its
certificate solved the wrong program, and this round replaces it rather than
patching it.

The one-line result. On the two **record** parents the largest depth reduction
that a bounded SE(2) motion can be *shown* to achieve — shown by applying a
vector and having `validate_publication` accept the result — is **0.039 mm**
(155.264) and **0.030 mm** (155.422). The 0.422 mm the record line is chasing is
an order of magnitude away. The linearized model's upper bound at a 1 mm trust
radius is 0.616 mm and 0.920 mm, so the model does **not** exclude 0.422 mm
either; what has changed is that the lower end of the bracket is now a real,
applicable, independently re-checked layout instead of a number nobody could
use. This is still Sol's third case — *"SE(2) positive but small, report the
bound"* — but for the first time the small number means something.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-3` |
| branch | `sol6/se2-rigidity-rewrite` |
| base commit | `578f2e0` (Sol review 6 recorded) |
| governing document | `docs/sol-review-6-premerge-v5.md` §3 |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | true 5.0/5.0 exact clearance; record lineage allowance `0.0005`, from-request `0.002` |
| feature | `se2-rigidity-certificate`, off by default; further gated on `POLYGON_NESTING_SE2_CERTIFICATE` |
| gate binary (`jagua-experimental`, feature off) | sha256 `89fdddf4fb0c7505c79e43fdbbfb8a390a8e34edb0b2501fa44051261419e593` |
| armed binary (`jagua-experimental,se2-rigidity-certificate`) | sha256 `8537136cf415c3412ae467d6af7853fcb759816a65dd485a10fb13a76bedc04e` |
| box | x86_64, shared with two other measurement agents. Every certificate call is ≤ 1 s and no wall-clock claim is made anywhere in this round. |

## 1. What the old certificate got wrong

Each item is one of Sol review 6 §3's findings, what it actually caused, and
what replaced it.

### 1.1 It optimized uniform slack, not depth

The old program was

```text
max_x  min_i (a_i . x - rhs_i)
```

— push **every** row open by the same amount. Sol: *"reducing the depth by
0.422 mm does not require simultaneously opening every pair contact, the left
edge, the bottom and the short edge by 0.422 mm."* That objective is bounded
above by the tightest contact anywhere in the packing, so it answers a question
about the least slack in the layout and reports it as if it were a statement
about the depth.

The program here introduces `δ` and puts it only on the rows that measure the
depth:

```text
max δ
  pair and non-depth boundary rows:   a_i . x >= rhs_i
  depth rows:                         a_i . x >= rhs_i + δ
  x in Box
```

The size of the difference is visible in the numbers. On 155.422 at a 1 mm
trust radius the old program bracketed `[0.314, 0.462]`; the corrected one
brackets `[0.854, 0.920]`. The old bound was not conservative, it was
**measuring something else**.

### 1.2 It imposed one bound on two different gates, then hand-calibrated it

The old branch reported that two of its four parents *"needed their depth bound
calibrated upward by 0.15–0.28 mm before the program's containment rows agreed
the state was even feasible."* Sol called that a red flag on the formulation,
and it was: the material outline and the miter-jointed collision envelope were
both being measured against one `sheet_long_axis_mm`.

They are now two different numbers read off two different pieces of geometry —
the material family against the publication measure itself
(`raw_source_long_axis_depth_mm`), the envelope family against the engine's own
`tight_strip_depth` quantity. The certificate reports the gap as
`stripExcessMm`, and it is exactly the quantity that used to be calibrated away:

| parent | published depth | strip bound | **strip excess** |
|---|---|---|---|
| 155.264 | 155.26442950832842 | 155.541 | **0.276570** |
| 155.422 | 155.42229074464285 | 155.425 | 0.002709 |
| 156.418 | 156.418 | 156.420 | 0.002000 |
| 171.238 | 171.23783708207895 | 171.389 | **0.151163** |

Two of four, 0.15–0.28 mm — the old branch's own recalibration range, recovered
as a measurement. Nothing is calibrated now, and `parentWorstResidualMm` proves
it: the parent's own worst residual is **non-negative in every family on every
parent**, so the parent satisfies every row of its own model without help.

| parent | MaterialDepth | EnvelopeStrip | MaterialBoundary | EnvelopeBoundary | MaterialPair | EnvelopePair |
|---|---|---|---|---|---|---|
| 155.264 | 0.005187 | 0.009427 | 0.006187 | 0.005187 | 0.004000 | 0.0 |
| 155.422 | 0.009421 | 0.010187 | 0.006187 | 0.005187 | 0.003000 | 0.0 |
| 156.418 | 0.007991 | 0.007991 | 0.006187 | 0.004187 | 0.008000 | 0.0 |
| 171.238 | 0.007991 | 0.007990 | 0.009991 | 0.005187 | 0.002422 | 0.0 |

`EnvelopePair` is exactly `0.0` on all four: the collision envelopes are already
touching. That is not a defect, it is the state of these fronts — and it is the
single most important caveat in §5.

### 1.3 Boundary rows had `theta = 0`

A rotation could open a pair contact without paying for the extreme vertex it
drives into the sheet edge, which **overestimates** rotational room. Every
boundary row now carries `a_theta = n . J(p - c)` for its own vertex, and there
is a row for every vertex that can *become* extreme inside the box, pruned by an
exact domination test (`spread >= Theta * |n . J(p - p*)|`) that collapses to the
single current extreme when `Theta = 0`.

### 1.4 The witness was discarded exactly at touch

`measure_approach` returned `None` for its witness pair when the outlines met —
so every *active* contact, the rows actually holding the front, got a zero
rotational coefficient, which reads as "rotation cannot open this" when the
truth was "nobody measured". It now returns the contact point.

### 1.5 Envelope rows existed only for real overlaps, and the guard band was too short

A legal pair reachable inside the trust box could be driven into collision with
no row to stop it. Every pair inside the band now gets an envelope row, and the
band is `2 * trust + Theta_i * reach_i + Theta_j * reach_j` rather than the old
Euclidean `2 * trust`.

This one has a regression test with a story. A draft of the rewrite recovered
the translation term as `theta_cap * reach`, which is the trust radius for a
rotatable piece and **zero** for a pinned one — so a pair of request-pinned
pieces got a band of just the clearance contract and no row at all,
reintroducing the very defect being fixed.
`a_pinned_pair_inside_the_translation_band_still_gets_a_row` fails on that draft
(`{"EnvelopeBoundary": 12, "EnvelopeStrip": 4, "MaterialBoundary": 12,
"MaterialDepth": 4}` — no pair family at all) and passes on the fix.

### 1.6 `lower > upper` was hidden behind `.max(0.0)`

Every quantity a bound is built from now accumulates through `RoundedSum`, which
carries `(n + 2) * eps * sum|term|` alongside the sum, so `low()`/`high()` are a
real interval around the exact real value. The bracket is **asserted**: an
inverted bracket returns an error naming the program, and never a clamp. Every
raw document carries the string

> `real-arithmetic bounds evaluated in f64 with an outward rounding allowance;
> not exact rational certificates`

so a reader of the JSON does not have to find this README to learn what the
numbers are.

### 1.7 `Approach.witness` cost the production path

It was unconditional on the old branch. It is now behind
`#[cfg(feature = "se2-rigidity-certificate")]`, including inside
`measure_approach`, so a default build's `Approach` neither grows nor does the
extra work. `global_legalize` runs that function on every pair of every round.

### 1.8 The result kept no vector

Fixed, and it turned out to be the most interesting correction of the eight.
See §3.

## 2. The four programs

One row set serves `{depth-only, strip-coupled} x {translation-only, SE(2)}`;
rows the translation column does not need are *implied* at `Theta = 0`, not
wrong, so the four columns are genuinely the same program under different boxes.
`stripCoupled` additionally requires the envelope strip to shrink by `δ`, which
makes the cost of the miter reach visible instead of absorbed.

61 pieces, all rotatable, 797–840 rows, reach 24.75–96.39 mm, so a 1 mm trust
radius is a `theta` cap of 2.31° on the smallest piece and 0.59° on the largest.

## 3. The witness, and why its length is not the model's to choose

The model's rows are relaxed outward by the exact second-order chord term
`reach * theta^2 / 2`, which is what makes the dual bound a valid upper bound on
the *rotated* geometry. The price is that the model's optimum sits up to that
slack **outside** the true constraint, and a solver always drives its binding
rows to equality — so the full-length vector lands a few microns past a sheet
edge and `validate_publication`, whose containment test is a strict inequality,
rejects it. Measured on 155.422 at 1 mm: the model claimed 0.854 mm and the
validator answered *"piece … crosses the sheet clearance boundary"*.

So the model supplies the **direction** and the exact validator decides the
**length**: a geometric ladder plus eight bisections along the ray, maximizing
the validated depth reduction, with `alpha = 0` — the parent — always available
as a floor. `deltaMm` is therefore always present and always exactly validated,
and `fullVectorExactValid` is reported beside it, because *that* is the
diagnostic: a large `primalLowerMm` with `fullVectorExactValid = false` is a
model writing cheques its geometry will not cash.

Two candidate rays are searched, because the model's own feasibility notion is
the relaxed one and is not obviously the better guide: `modelFeasible` (the best
point satisfying every non-`δ` row) and `modelObjective` (the best-objective
point, feasible or not). The winner is whichever the exact validator scores
higher.

## 4. Results

Four parents x six trust radii. `[lower, upper]` is the bracket on the
**linearized, outward-relaxed** program; `witness` is the exactly-validated
depth reduction; `scale` is the fraction of the model's vector that survived.

| parent | trust | translation `[l, u]` | translation witness | SE(2) `[l, u]` | SE(2) witness | scale | full vector valid |
|---|---|---|---|---|---|---|---|
| 155.264 | 0.006 | [0.00000, 0.00010] | 0.006000 | [0.00659, 0.00673] | **0.009361** | 1.0000 | yes |
| 155.264 | 0.025 | [0.00005, 0.00067] | 0.013721 | [0.02733, 0.02830] | 0.006031 | 0.2207 | no |
| 155.264 | 0.1 | [0.00052, 0.00339] | 0.037988 | [0.10942, 0.11393] | 0.007042 | 0.0442 | no |
| 155.264 | 0.25 | [0.00153, 0.00942] | 0.038330 | [0.19618, 0.21040] | 0.008827 | 0.0291 | no |
| 155.264 | 0.5 | [0.00167, 0.02453] | **0.039131** | [0.31986, 0.35119] | 0.006666 | 0.0120 | no |
| 155.264 | 1.0 | [0.01243, 0.07014] | 0.030469 | [0.54787, 0.61553] | 0.005279 | 0.0050 | no |
| 155.422 | 0.006 | [0.00600, 0.00600] | 0.006000 | [0.00676, 0.00677] | 0.003342 | 0.4941 | no |
| 155.422 | 0.025 | [0.02501, 0.02501] | 0.025000 | [0.02577, 0.02589] | 0.003603 | 0.1398 | no |
| 155.422 | 0.1 | [0.00009, 0.09122] | 0.030176 | [0.10086, 0.10153] | 0.002488 | 0.0247 | no |
| 155.422 | 0.25 | [0.00059, 0.10502] | **0.030420** | [0.25135, 0.25399] | 0.003056 | 0.0122 | no |
| 155.422 | 0.5 | [0.00236, 0.12351] | 0.017188 | [0.50312, 0.51343] | 0.003169 | 0.0063 | no |
| 155.422 | 1.0 | [0.00942, 0.15402] | 0.027031 | [0.85397, 0.91987] | 0.015168 | 0.0152 | no |
| 156.418 | 0.006 | [0.00600, 0.00600] | 0.006000 | [0.00600, 0.00601] | 0.006000 | 1.0000 | yes |
| 156.418 | 0.025 | [0.02500, 0.02500] | 0.025000 | [0.02500, 0.02504] | 0.025000 | 1.0000 | yes |
| 156.418 | 0.1 | [0.10008, 0.10008] | 0.100000 | [0.10008, 0.10029] | 0.100000 | 1.0000 | yes |
| 156.418 | 0.25 | [0.25050, 0.25050] | 0.250000 | [0.25050, 0.25129] | 0.051221 | 0.2049 | no |
| 156.418 | 0.5 | [0.50200, 0.50200] | **0.499876** | [0.50200, 0.50493] | 0.054980 | 0.1100 | no |
| 156.418 | 1.0 | [0.70175, 0.76272] | 0.116606 | [1.00799, 1.02200] | 0.038437 | 0.0384 | no |
| 171.238 | 0.006 | [0.00311, 0.00312] | 0.006000 | [0.00600, 0.00601] | 0.010664 | 1.0000 | yes |
| 171.238 | 0.025 | [0.01296, 0.01305] | 0.025000 | [0.02500, 0.02515] | 0.044437 | 1.0000 | yes |
| 171.238 | 0.1 | [0.04984, 0.05099] | 0.100000 | [0.10017, 0.10098] | **0.177778** | 1.0000 | yes |
| 171.238 | 0.25 | [0.11611, 0.11973] | 0.115611 | [0.24472, 0.24925] | 0.150347 | 0.6152 | no |
| 171.238 | 0.5 | [0.22811, 0.23604] | **0.211093** | [0.48274, 0.49534] | 0.132469 | 0.2754 | no |
| 171.238 | 1.0 | [0.45827, 0.47596] | 0.095421 | [0.90647, 0.93458] | 0.022891 | 0.0252 | no |

### 4.1 The verdict, per parent, in Sol's three cases

Nobody is `blocked`: every `dualUpperMm` is positive on every parent at every
radius, so no parent's front is rigid. Every headline verdict is
`positive-below-reference` against the 0.422 mm reference — including 156.418,
whose translation-only column reaches 0.4999 mm, because the headline is the
depth-only **SE(2)** column and that column reaches 0.055 mm at the same radius.

| parent | best validated `δ` (any motion, any radius) | at | model upper at 1 mm | verdict |
|---|---|---|---|---|
| 155.264 (record) | 0.039131 | translation, 0.5 mm | 0.615525 | `positive-below-reference` |
| 155.422 | 0.030420 | translation, 0.25 mm | 0.919870 | `positive-below-reference` |
| 156.418 | 0.499876 | translation, 0.5 mm | 1.022000 | `positive-below-reference` |
| 171.238 | 0.211093 | translation, 0.5 mm | 0.934582 | `positive-below-reference` |

### 4.2 Does rotation actually open more room?

This was Sol review 5's rank-0 question and the old branch answered it from the
model alone ("rotation is consistently 1.2–3x bigger than translation"). On the
model, that reproduces here and then some: SE(2)'s upper bound runs from 1.00x
to **70.48x** the translation-only bound. On the **exactly-validated** number it
is far more interesting.

SE(2) strictly beats translation in **5 of 24** cells, ties in 3, and loses in
16:

| | cells | where |
|---|---|---|
| SE(2) wins | 5 | 155.264 @ 6 µm; 171.238 @ 6 µm, 25 µm, 0.1 mm, 0.25 mm |
| tie | 3 | 156.418 @ 6 µm, 25 µm, 0.1 mm |
| SE(2) loses | 16 | the whole 155.422 column; 155.264 @ ≥ 25 µm; 156.418 and 171.238 @ ≥ 0.25 mm and ≥ 0.5 mm |

Two separate effects, and it is worth not conflating them.

* **Where rotation wins, it wins by a real margin and only at small radii**:
  1.56x on 155.264 at 6 µm (0.009361 vs 0.006000) and 1.78x on 171.238 at 6 µm,
  25 µm and 0.1 mm (0.177778 vs 0.100000). Every one of these has
  `scale = 1.0` — the model's full step survived the exact validator intact.
* **Where the radius is large, rotation loses because the model over-reaches.**
  `scale` collapses to a few percent, and the line search hands most of the
  claimed room back. On 155.264 the crossover is between 6 µm and 25 µm; on
  156.418 and 171.238 it is between 0.1 mm and 0.25 mm.
* **155.422 is not that story.** It loses at *every* radius, 6 µm included, where
  `scale` is already only 0.494. Rotation is simply not the lever on that front,
  and no trust radius makes it one. Note that 155.422 is also the parent with
  almost no strip excess (0.0027 mm) — its envelopes and its material agree about
  where the depth is, so there is nothing for a rotation to exploit.

The reading: rotation does open genuinely more room on *some* fronts, by roughly
1.5–1.8x, and only inside a trust region small enough for the linearization to
hold. Widen the box and the model's rotational optimum runs away from the
feasible set faster than the extra freedom pays for. A trust radius is not a free
parameter for this diagnostic — past the crossover it is buying model error —
and rotation is not a universal lever, because on one of these four parents it
never paid at any radius tested.

### 4.3 The old brackets, and the 0.422 mm question

Sol: *"at 1 mm trust the parents 155.264 and 155.422 have SE(2) brackets of
about [0.3347, 0.5024] and [0.3140, 0.4617]. The lower does not reach 0.422, but
the upper does not exclude it."*

The corrected program does not exclude it either — the upper bounds are larger,
0.6155 and 0.9199, because the old program was solving a strictly harder problem
and its upper bound was small for the wrong reason. What the rewrite adds is the
other end: a **constructive** 0.039 mm and 0.030 mm that can be applied, and
were. The gap between 0.039 and 0.616 is the honest statement of what is
unknown, and it is a gap between a real layout and a model bound rather than
between two model numbers.

## 5. What these numbers are NOT

**They are not record claims.** `validate_publication` checks material
containment and material pair clearance. It does **not** look at the collision
envelope, and `contractValid` — the engine's own contract gate — was never run
on any witness in this round. Three specific reasons that matters here:

1. `EnvelopePair` slack is exactly `0.0` on all four parents. The envelopes are
   already touching, so a move that is fine for the material gate can trivially
   fail the envelope gate.
2. The winning ray is `modelObjective` on most cells, and that ray is under no
   obligation to satisfy the model's own envelope rows.
3. The SE(2) witnesses rotate pieces by 0.005–0.29°, which are not angles the
   engine's orientation grid necessarily carries.

So the correct reading of "0.039 mm on the 155.264 record" is *the material gate
and the publication measure accept a layout that much shallower*, not *the record
is 155.225*. Turning any of these into a record claim needs a pinned fixture and
the replay battery, and that is the single highest-value follow-up this round
leaves behind.

**They are one ray each.** The line search explores the model's direction and
nothing else, in a 183-dimensional space. `deltaMm` is a lower bound on what
SE(2) motion inside the box can do; it is not a characterization of it.

**`primalLowerMm` is not a bound on anything physical.** It is the best objective
reached by a point feasible for the *relaxed* rows, and relaxed feasibility does
not imply real feasibility. It is one end of a bracket on the model. The only
number here that lower-bounds achievable depth reduction is `witness.deltaMm`,
and the only one that upper-bounds it is `dualUpperMm`, and those two do not
meet.

## 6. Independent verification

Every number above was re-derived **out of engine**. `verify_witness.py`
re-implements the placement transform from `transform_source_ring`, the strict
containment test from `validate_sheet`, and a brute-force segment-to-segment pair
distance, in Python, with no engine code in the loop. It calibrates itself first:
the parent's depth must reproduce the pinned value to the ULP, or nothing after
it is trusted.

Over all **96** cells (4 parents x 6 radii x 4 programs):

```text
ALL_AGREE=True  ALL_CONTAINED=True  ALL_PAIR_OK=True  rows=96
```

— the independent depth matches the engine's to < 1e-9 in every cell, every
witness is inside the sheet, and every witness holds the 5.0 mm pair contract.

The check also settled the contract itself rather than assuming it: the parent's
worst pair distance measures 5.004 mm, and the certificate's
`parentWorstResidualMm.MaterialPair`, computed by unrelated code inside the
engine, is 0.004. Two independent routes to the same 5.0 mm.

## 7. Regression gates

Four pinned gates, on both binaries.

`gate_round.py` runs **three** passes, not two: flag-off, flag-off *again*, and
flag-on-unarmed. The repeat is the noise floor, without which a document
comparison between two binaries cannot tell "the flag changed something" from
"this document was never stable" — see §7.1.

| gate | mode | pinned raw depth | pinned fingerprint | off | off (repeat) | on (unarmed) | shared document digest |
|---|---|---|---|---|---|---|---|
| g1 | 20 | 206.869 | `8a7737381238fa4d` | hit | hit | hit | `7a4b0288009a6ca2` |
| g2 | 22 | 159.09233022733062 | `fa01012af1d559ae` | hit | hit | hit | `18631dad1272c7ba` |
| g3 | 22 | 159.07876040364795 | `e28fba007f8031d4` | hit | hit | hit | `d9a3666c96b3c414` |
| g4 | 22 | 164.0375677990678 | `49f094d7e59a9008` | hit | hit | hit | `9c10414dda9b3a0b` |

```text
ALL_PASS: true    ALL_DIGESTS_MATCH: true
CLAIM_PATHS_OUTSIDE_FLOOR: ["/executableSha256"]
```

All three runs share one digest per gate — the flag-off binary against itself
and against the flag-on binary. Per gate the noise floor is 6 leaf paths (all
wall-clock), the off-vs-on claim is 7, and the single path outside the floor is
`/executableSha256`, which is the binary's own hash and is the one field that
*must* differ. The default path is bit-reproducing.

The flag-off binary is byte-identical before and after the certificate work
(`89fdddf4…`), which is the mechanical confirmation that the whole module is
behind its `#[cfg]`.

### 7.1 The whole-document instrument was broken, and is fixed

This round was told to copy its gate drivers from
`docs/experiments/constructor-inner-certificate/drivers/`. That copy's
`lib.doc_digest` hashes a document after dropping a hand-written `VOLATILE` key
list, and the list dropped `elapsedMs` but **not** the five summary statistics
computed from it — so two runs of the *same* binary on the *same* gate hashed
differently every time:

```text
g1 bdfdecb4… vs d2872ad1…    g2 09d4226a… vs f566b887…
g3 2f8a707e… vs 6284e99a…    g4 29089c43… vs b6804532…
```

All four, flag-off against flag-off. A digest mismatch proved nothing and a
match would have been luck. `medianElapsedMs`, `minElapsedMs`, `maxElapsedMs`,
`firstQuartileElapsedMs` and `thirdQuartileElapsedMs` are now in `VOLATILE`, as
is `executableSha256` — which is the binary's own identity and must differ
whenever two binaries are compared, which is the entire use of the function.

To be fair to the branch this round replaces: **`ac9b890` got this right.** Its
own `docdiff.py` carried a complete `VOLATILE` list, quartiles and
`executableSha256` included, and counted differing fields directly rather than
hashing. Its "0 differing fields" claim was sound and is not among the things
§1 corrects. The defect is in the `constructor-inner-certificate` driver
lineage, and it is fixed here because this round inherited it.

`docdiff.py` is the paired instrument that found this and reports leaf-path
diffs against a same-binary noise floor. Its verdict on this change, per gate:

* noise floor (off vs off): 6 differing paths, all wall-clock;
* claim (off vs on-unarmed): 7 differing paths;
* **outside the floor: exactly one, `/executableSha256`.**

A second field had to join them for the same reason: `engineWorktreeStatus`,
which changes every time any file in the tree is edited, so the digest was also
a function of the author's editor. `relevantSourceTreeSha256`, `engineCommit`
and `engineWorktreeDirty` are in for the same reason.

With the repaired `VOLATILE`, four independent runs across two binaries produce
**identical digests on all four gates** (§7).

## 8. Tests

Both feature combinations, full suite, `EXIT=0` each:

| features | tests passed |
|---|---|
| `jagua-experimental` | 1250 |
| `jagua-experimental,se2-rigidity-certificate` | 1266 |

The 16 are the difference, and the combo suite is the one Sol review 6 flagged
as missing on the previous round. The known-flaky
`free_material_multi_eviction…` ran and passed in both, first time, so no rerun
was needed.

16 unit tests, `--features jagua-experimental,se2-rigidity-certificate`. The
ones Sol named specifically:

* `rotation_changes_the_verdict_end_to_end` — the case where `dθ ≠ 0` changes
  the *verdict*, not merely the number: against a reference between the two
  achievable depths, translation returns `positive-below-reference` and SE(2)
  returns `positive-reaches-reference`, both off exactly-validated numbers.
* `the_rotation_coefficient_matches_a_finite_difference_of_the_exact_motion` —
  real geometry, off-centre probe, non-axis-aligned normal.
* `the_rotation_coefficient_holds_on_a_mirrored_pose` — negative-determinant
  source map.
* `the_rotational_coefficient_and_its_slack_hold_on_a_miter_envelope` — the
  third geometry, and the one that produced a finding: `PolygonSet::offset`
  quantizes to the Clipper2 grid at 1000 units/mm, so the envelope's `max_y` is
  a **staircase** in theta and a finite difference at `h = 1e-5` returns exactly
  `0` against a coefficient of `-5.05`. The test uses a central difference at
  `h ∈ [0.01, 0.04]`, where the micron quantum is 0.5% of the signal; measured
  agreement 0.07%. It also checks that the relaxed row still bounds the exactly
  rotated envelope at the full angular cap.
* `a_pinned_pair_inside_the_translation_band_still_gets_a_row` — §1.5.
* `the_witness_is_exactly_valid_on_every_program_even_when_the_model_overreaches`
  — the line search must always return a validated layout and a real number.
* `the_parent_satisfies_its_own_depth_and_strip_rows_without_calibration` — §1.2.
* `rotation_does_not_help_a_shape_with_two_symmetric_peaks` — the companion
  negative; an axis-aligned rectangle's two top corners have equal and opposite
  lever arms, so rotation must not appear to help. This is the test that fails
  loudly if the rotational coefficients ever lose their sign.

Plus `the_rounded_sum_brackets_its_own_exact_value`,
`applying_a_zero_vector_returns_the_placement_unchanged`,
`a_piece_that_may_not_rotate_gets_no_angular_freedom`,
`the_strip_bound_is_separated_from_the_published_depth`,
`every_program_reports_a_consistent_bracket_on_a_packed_front`,
`an_isolated_piece_gets_a_positive_and_exactly_validated_witness`,
`the_rotational_guard_band_admits_at_least_the_translational_one`, and
`the_rotation_coefficient_is_the_normal_against_the_quarter_turn_generator`.

## 9. Drivers

| driver | what it does |
|---|---|
| `lib.py` | shared runner, the four pinned gates, the repaired `VOLATILE` |
| `gates.py` | the four gates against one binary |
| `docdiff.py` | paired leaf-path document diff against a same-binary noise floor |
| `certify.py` | the certificate over 4 parents x 6 trust radii |
| `verify_witness.py` | out-of-engine re-derivation of one witness |
| `verify_all.py` | `verify_witness.py` over all 96 cells |
| `gate_round.py` | the three-pass gate round: both binaries, a repeat, the paired diff |

Evidence in `evidence/`: `certify.json` (the whole sweep), `verify.json` (the 96
independent re-derivations), `gate-round.json`, and the two raw 1 mm
certificates for the record parents.

## 10. What this does not justify

It does not justify m33 in production, and it does not close the 0.422 mm hunt.
It says the front is not rigid, that the achievable-by-construction number on the
record parents is ~0.03–0.04 mm, that rotation is worth about 1.5–1.8x but only
under ~0.1 mm of trust, and that the model's own bound is too loose to decide
0.422 mm either way. The next instrument this points at is not a wider box — it
is running `contractValid` on the witnesses that already exist.
