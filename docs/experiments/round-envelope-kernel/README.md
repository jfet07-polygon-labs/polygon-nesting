# The certified round-envelope kernel: exact, sound, 8x cheaper — and the grid step it exposes belongs to the miter

Sol review 11 named the Certified Round-Envelope Kernel as the one spend left
inside the composite contract. Sol review 12 §3.2 set its kills; Grok review 7
§2 kept them unmodified and added one: *"Inscribed Clipper round is not this
kernel."* This round builds it and runs its soundness battery.

**No default changed. Nothing was promoted.** The feature is off at compile
time, off at run time even when compiled, and armed only by the v3 coordinator
for the duration of one run. The four pinned regression gates reproduce on a
fresh build **and** on a build that carries the feature compiled but unarmed —
per gate, as whole documents.

The search-side comparison — the twelve-parent matched gate at equal operator
wall — is deliberately **not** in this round. It is the next agent's assignment,
and §7 says exactly what this round hands it.

---

## 0. The answer

| Sol/Grok kill | required | measured | |
|---|---|---|---|
| zero false accepts on the three populations and the ±1 µm cases | 0 | **0 of 82 kernel accepts on 194 material-valid / canonical-invalid proposals** | ✅ |
| every currently canonical-valid layout stays valid | all | **`union`: 17/17 layouts at all three radii, 0 rows lost of 96 441.** `exclusive`: 13 rows lost, every one at exactly **−1 µm** | ⚠️ see §3 |
| the Sparrow differential's four pre-committed expectations | all | **4 of 4** | ✅ |
| the ±1 µm sweeps agree with the exact material distance, monotonically | all | **30 of 30 monotone, 0 steps disagreeing outside the canonicalization band, flip within one grid step on 30 of 30** | ✅ |
| confirmation-class cost ≤ 1.25x | ≤1.25 | envelope half **0.121x** median; whole confirmation **0.904x** (`exclusive`) and **1.005x** (`union`); with `fast-contract-validator` armed, **0.470x** and **1.046x** | ✅ |
| two-process determinism on every battery document | byte-identical | **identical**, and identical again across a *second binary* built with a different feature set | ✅ |

The one qualified row is the round's main finding and it is not a defect in the
kernel:

> **The shipped miter authority re-quantizes its offset output to the 1 µm
> canonical grid, so at *contact* it admits pairs whose canonical separation is
> `2r − 1` µm — one grid step permissive of its own declared envelope — and the
> short-side-first constructor places pieces exactly at contact.**

All 13 losses are that one row, at the same 1 µm, with the miter envelopes'
intersection area measured at exactly `0.0` mm², and on **11 of the 13** the
untouched source-ring clearance is itself below what the composite demands. The
kernel is never the permissive one anywhere in this battery.

`KernelMode::Union` — admit what either half admits — closes the row by
construction and is the mode a promotion would be asked for. It is Sol review 12
§3.2's *"serve un ibrido"* and Sol review 8's internal-filter architecture, and
it costs 1.005x of a HEAD confirmation.

---

## 1. What the kernel is

The engine publishes on a conjunction: the **material contract** on untouched
`f64` source rings, **and** a **canonical envelope** — every placement's source
ring canonicalized to the 1 µm grid, offset outward by `collision_expansion_mm`
with `JoinType::Miter` at `CLIPPER_MITER_LIMIT = 2.0`, required to fit the inset
sheet and to be pairwise disjoint.

`crates/polygon-nesting-core/src/validation/round_envelope.rs` is a second
implementation of **that second half only**, with the miter join replaced by a
disc. The material contract validator is untouched and remains the final
authority in every mode; §7's file table is what says so — the wire point is
one branch, and the contract half is not inside it.

### It is exact, and that is a correction to Sol's specification rather than a shortcut around it

Sol review 11 asked for *"discretizzazione soltanto outward, con errore
formalmente incluso nel margine"*. Gate A then measured the case that answer
cannot serve: Sparrow pair 38·39 has **0.42 µm** of radius margin at `r = 2.5`,
*below* the 1 µm canonical grid step, so any outward polygonal approximation of
the disc with the error charged to the margin must refuse a legal layout. Gate
A's own README says so, and Grok review 7 §2 repeats it as the promotion's unit
test.

So there is no approximating polygon here at all. The canonicalized rings are
integers on a 1 µm grid, and both questions have exact integer answers:

* **pair.** `P_i ⊕ disc(r)` and `P_j ⊕ disc(r)` have disjoint interiors iff the
  material sets do not overlap **and** their minimum boundary distance is at
  least `2r`. Squared distances between integer points and integer segments are
  *rationals*, so `d² ≥ (2r)²` is one `i128` comparison after
  cross-multiplication. The interior-projection branch — where an `f64`
  implementation has to round `|cross| / |v|` — becomes
  `cross² < threshold² · |v|²`, with no division and no rounding.
* **boundary.** The inset rectangle is axis-aligned and a disc reaches exactly
  `r` past the material in every direction, so `P ⊕ disc(r)` fits it iff the
  material's integer box grown by `r` fits it. Four integer comparisons.

The verdict is therefore a function of the integer grid alone: no rounding mode,
no error budget, bit-identical across platforms.

**Containment is asked separately.** Minimum *boundary* distance is not a
legality test on its own — a piece strictly inside another has a large positive
boundary distance and is an overlap — so the pair predicate refuses on
containment before it credits any distance, exactly as the contract validator's
`material_sets_overlap` does. `a_contained_piece_is_refused_at_every_radius` is
the unit test; a pure minimum-distance kernel would false-accept there.

**The `i128` bound is evaluated, not argued.**
`the_domain_bound_keeps_every_intermediate_inside_i128` computes each of the six
products the predicates form at the domain's own extreme
(`DOMAIN_MAX_MICRON = 2²⁸` µm = 268.4 m) and requires each to leave four bits of
`i128` headroom, then states the campaign's actual sheet — 2000 × 2700 mm — and
requires the domain to be ≥ 99x its long axis. A coordinate outside the domain
gets **no** certificate: `GridSet::of` returns `None` and the caller uses the
miter authority. Fail-closed, like `CLEARANCE_SLAB_MAX_COORDINATE_MM`.

### The one domain restriction, and why it is fail-closed rather than approximate

The kernel does not certify at **zero** expansion. At `2r = 0` the question
stops being "are these at least `2r` apart" and becomes "do these overlap with
positive area", which boundary distance cannot answer: two squares sharing an
edge are legal, and two overlapping squares whose every vertex lies on the
other's boundary are not, and both have minimum boundary distance zero and no
strictly-contained vertex. Deciding that needs an area authority, which
`polygons_overlap_exact` is and this module deliberately is not.

It never binds in production — `collision_expansion_mm` is 2.500–2.502 mm on
every configuration this campaign has run — and the wire point checks
`certifies()` and falls through to the miter authority when it is false.

### Economy: the same broad phase, in integers

An axis-aligned integer box gap of at least `2r` proves both clauses at once.
Over the whole canonical corpus at the shipping radius that certifies
**29 659 of 31 110 pairs (95.34%)** — the `fast-contract-validator`'s own ~96%,
without its floating-point margin, because integers do not need one. Below it, a
ring-level box test and then a segment-level box test; the narrow path runs only
on segment pairs no box test could separate. A whole 61-piece confirmation
reaches the exact narrow predicate **254 times** on parent-seed1 and 275 times
on the Sparrow pose set.

| envelope | vertices carried, 61 pieces |
|---|---:|
| **the kernel** (the canonical source rings; no offset is built) | **290** |
| the production miter offset | 377 |
| Gate A's inscribed Clipper round at 0.0001 mm arc tolerance | 20 601 – 20 669 |

Sol review 12 §3.2's *"Non promuovere il round Clipper attuale: produce 20.6k
vertici contro 377, circa 55x, ed è inscribed"* is discharged on both counts:
this carries 290 and it is not inscribed, because it builds no polygon.

---

## 2. The Sparrow differential — Gate A's pre-committed expectations, as a pass/fail

`evidence/battery.json` → `population3SparrowDifferential`,
`evidence/summary.json` → `population3`.

| radius | contract | composite miter (HEAD) | **the kernel** | pairs refused | boundaries refused |
|---|---|---|---|---|---:|
| **2.500** (`total_padding/2`) | ACCEPTS | REJECTS | **ACCEPTS** | **0 of 1830** | **0 of 61** |
| 2.5005 (record lineage) | ACCEPTS | REJECTS | REJECTS | 1 — pose `[0, 1]` | 0 |
| **2.502** (from-request, HEAD's shipping radius) | ACCEPTS | REJECTS | REJECTS | **2 — poses `[0, 1]` and `[42, 44]`** | **0** |

Every one of the four pre-committed expectations is met:

1. at `r = 2.500` the kernel accepts **all 1830 pairs and all 61 boundaries**;
2. at `r = 2.502` it refuses **exactly** the two radius-caused pairs Gate A
   named — pose indices `[0, 1]` (Sparrow items 38·39, material 5.000840 mm) and
   `[42, 44]` (items 50·52, material 5.002879 mm) — and **zero** boundaries;
3. **pair 38·39 at `r = 2.500` is ACCEPTED.** Its exact critical clearance is
   `2r* = 5.001` mm against the 5.000 mm demanded. The exact narrow phase
   decides it analytically, which is the row Sol's outward-only discretization
   would have refused;
4. the contract accepts the pose set at every radius.

The 2.5005 row is a bonus agreement with Gate A that was not pre-committed:
`2r = 5.002` mm, and pair 38·39 alone falls (5.001 < 5.002) while 50·52 survives
(5.002 ≥ 5.002). Gate A recorded exactly one `roundRefusesMiterAccepts` row at
that radius and it was this pair.

**So Grok's case 3 obtains under an exact authority, not only under an inscribed
shadow.** The legal set of `P ⊕ disc(2.5)`, decided in integer arithmetic with
no error budget, contains Sparrow's 150.16451 mm layout. The miter grid's does
not.

---

## 3. Population 1 — the canonical corpus, and the grid step

17 layouts × 3 radii = 51 cells: the 12 pinned from-request parents
(`docs/experiments/contact-block/drivers/parents12.json`), the four pinned
parents the g1–g4 gates run from, and one **fresh** short-side-first constructor
output built in the battery's own process. **93 330 pair rows and 3 111 boundary
rows** compared, per row, against `PolygonSet::offset` itself.

| radius | miter accepts | kernel (`exclusive`) accepts | **`union` accepts** | layouts `union` loses | pair rows the kernel admits and the miter refuses | pair rows the miter admits and the kernel refuses |
|---|---:|---:|---:|---:|---:|---:|
| 2.502 | 14/17 | 7/17 | **14/17** | **0** | 143 (+52 boundaries) | 11 |
| 2.5005 | 17/17 | 15/17 | **17/17** | **0** | 0 | 2 |
| 2.500 | 17/17 | 17/17 | **17/17** | **0** | 0 | 0 |

(The three layouts the miter refuses at 2.502 are the g2/g3/g4 record-lineage
parents, which were produced at the 0.0005 allowance; running them at 0.002 is
not a regression and is where the 143+52 join-price rows come from.)

### The 13 losses, attributed

Every one, with its two attributing quantities:

| layout | radius | pair | kernel `2r*` | shortfall | miter envelope intersection area | source-ring clearance | demanded `2r` | source < demanded |
|---|---|---|---:|---:|---:|---:|---:|:--:|
| constructor-fresh | 2.502 | `[3, 33]` | 5.003 | **−1.0 µm** | **0.0 mm²** | 5.004018225 | 5.004 | no |
| constructor-fresh | 2.502 | `[15, 16]` | 5.003 | −1.0 µm | 0.0 mm² | 5.003355714 | 5.004 | **yes** |
| parent-seed0 | 2.502 | `[36, 38]` | 5.003 | −1.0 µm | 0.0 mm² | 5.003431181 | 5.004 | **yes** |
| parent-seed2 | 2.502 | `[52, 55]` | 5.003 | −1.0 µm | 0.0 mm² | 5.002987466 | 5.004 | **yes** |
| parent-seed4 | 2.502 | `[23, 30]` | 5.003 | −1.0 µm | 0.0 mm² | 5.004001045 | 5.004 | no |
| parent-seed5 | 2.502 | `[8, 35]` | 5.003 | −1.0 µm | 0.0 mm² | 5.003877978 | 5.004 | **yes** |
| parent-seed5 | 2.502 | `[33, 36]` | 5.003 | −1.0 µm | 0.0 mm² | 5.003258596 | 5.004 | **yes** |
| parent-seed5 | 2.502 | `[40, 58]` | 5.003 | −1.0 µm | 0.0 mm² | 5.003203640 | 5.004 | **yes** |
| parent-seed10 | 2.502 | `[6, 31]` | 5.003 | −1.0 µm | 0.0 mm² | 5.003376670 | 5.004 | **yes** |
| parent-seed10 | 2.502 | `[8, 9]` | 5.003 | −1.0 µm | 0.0 mm² | 5.002890665 | 5.004 | **yes** |
| parent-seed11 | 2.502 | `[46, 58]` | 5.003 | −1.0 µm | 0.0 mm² | 5.003633417 | 5.004 | **yes** |
| gate-g2-parent-159.092 | 2.5005 | `[7, 40]` | 5.000 | −1.0 µm | 0.0 mm² | 5.000956821 | 5.001 | **yes** |
| gate-g3-parent-159.079 | 2.5005 | `[7, 40]` | 5.000 | −1.0 µm | 0.0 mm² | 5.000841823 | 5.001 | **yes** |

**The attribution is a proof, not a hypothesis.** `offset_miter(P, r)` contains
`P ⊕ disc(r)` exactly, at every corner. So if the kernel's *exact* minimum
boundary distance is below `2r`, the **true** miter envelopes overlap with
positive area. The measured Clipper intersection area is exactly `0.0` on all
thirteen. The only thing between "the true miter overlaps" and "the measured
miter does not" is `do_round()`, which re-quantizes the offset output to the
canonical grid (`clipper/offset.rs`). Each envelope's outward face can land up
to `√2/2` µm inside where the exact offset would put it; two of them is
`√2 ≈ 1.414` µm, and 1 µm is what was observed.

And the direction matters: on **11 of the 13** the untouched `f64` source-ring
clearance is *itself* below the composite's own demanded `2r`. On those rows the
shipped authority is not merely rounding — it is admitting pairs that do not
meet the radius it declares. The kernel is what makes the declaration true.

**None of the thirteen is a contract violation.** Every one is above the 5.000 mm
the material contract asks, and the material contract validator runs unchanged
in every mode.

### Why `union` is the promotion candidate

Because the constructor places pieces *at* contact, this is not a rare row — it
is the regime the constructor works in. `evidence/smoke.json` records the
consequence end to end: with `rek=2` (exclusive) a bare-request coordinator run
aborts with

```
InvalidInput("pieces 604bc424-…-copy-1 and 604bc424-…-copy-2 overlap on the canonical collision grid")
```

from `general_fast::validate_result` — **the constructor's own self-check
refusing the layout the constructor just built**, because the geometry the
constructor reasons in is the miter envelope and the geometry that confirms it
is the disc. With `rek=1` (union) the same run completes and produces a document
that differs from the unarmed one, which is the kernel doing work.

`KernelMode::Union` admits a layout when the kernel admits it **or** the miter
authority does. Two properties follow from the disjunction without a
measurement, and the battery confirms both:

* **no canonical-valid layout is lost** — 0 of 51 cells, at all three radii;
* **no new false-accept surface beyond HEAD's own** — the union is bounded by
  the two halves it is made of, and the contract validator is untouched.

The kernel is asked *first*, so the miter is built only on the rows the kernel
refuses.

---

## 4. Population 2 — where a false accept could have happened, and did not

Sol's second population is *"proposte material-valid/canonical-invalid già
committate"*. **The contact-block round's refused outputs were not committed
with their placements** — its evidence carries per-round statistics and cell
tables, not pose sets (`blockprobe*.json`, `matched*.json`, `why*.json`: no
`placements` key in any of them). So the population is **constructed** instead,
which also makes it regenerable:

a near-critical pair of a pinned parent is lifted into a two-placement
sub-layout and walked across the miter threshold in whole micrometres; every
step where the **contract accepts and the composite miter refuses** is a
material-valid / canonical-invalid proposal. Boundary walks against the sheet
edge contribute the same way.

**194 proposals. The kernel accepts 82 of them. Zero false accepts.**

The false-accept test is against the **contract validator's own** material
distance — `material_pair_distance_mm` / `material_sheet_clearance_mm`, the
functions `validate_publication` itself reduces to a boolean — because asking a
second distance function is exactly how a soundness battery fakes itself. A
kernel accept is a **false accept** when that source-ring clearance is below the
demanded `2r` by more than the **canonicalization budget**:

> every source vertex is snapped to the nearest micrometre, moving it by at most
> `√2/2` µm; an interior point of an edge is a convex combination of its
> endpoints and moves by at most the same; two rings, so the canonical distance
> and the source distance differ by at most **`√2` µm = 0.0014143 mm**.

Three of the 82 accepts sit *inside* that band — source clearance
5.003944 / 5.003704 mm against 5.004 mm, and one boundary at 5.001985 against
5.002 — by 0.056, 0.296 and 0.015 µm. Those belong to the canonical grid, which
both authorities are built on and neither invented, and they are reported as
their own count (`insideCanonicalizationBudgetCount: 3`) rather than folded into
either verdict.

---

## 5. Population 4 — the ±1 µm sweeps

30 sweeps: 20 near-critical **pairs** and 10 **boundaries**, drawn as the
tightest rows by exact critical clearance from five corpus layouts, each walked
17 steps in whole micrometres.

The window is **centred on the crossing**, located first by doubling and
bisecting on the kernel's own pair predicate, because a window centred on the
parent's pose only straddles the threshold if the pair happened to start within
8 µm of it, and a sweep that never crosses proves nothing about a flip point.

| | |
|---|---|
| monotone in the material clearance | **30 / 30** |
| monotone in the step index | **30 / 30** |
| steps whose kernel verdict disagrees with the source-ring measurement, outside the canonicalization band | **0** |
| kernel flip minus material flip, in grid steps | **−1 on 3, 0 on 19, +1 on 8** |
| worst \|kernel `2r*` − source clearance\| | **1.970 µm**, against a 2.4143 µm budget |
| sweeps outside that budget | **0** |

The sweep budget is 2.4143 µm and not 1.4143: `critical_two_r_micron` returns
the largest *integer* micrometre at which the pair is still admissible — the
floor of the canonical distance — so the comparison carries one whole grid step
on top of the canonicalization. A first pass of this battery used the smaller
number and reported three floors as disagreements; the constant is now named
`SWEEP_FLOOR_BUDGET_MM` next to the comparison it guards.

**A methodological correction worth recording.** The first pass also read two of
the thirty as non-monotone. They were not: the walk direction is the two
placements' box-centre difference, and for two interlocking concave pieces a
step *away* along that direction walks the mover toward a **sheet edge**. The
whole-layout verdict then flips for a reason that has nothing to do with the
pair — and the contract, the miter and the kernel all flipped on the same step,
which is what said so. The sweep summary is now taken on
`kernelPrimaryAdmissible`, the pair question alone; the whole-layout verdicts
stay, because a *proposal* is a whole layout and population 2 is classified on
them.

---

## 6. Economy

`envelopeHalf*` is a **full scan on both arms** — no early exit on either — so it
is comparable on every cell. `confirmation*` is one call to
`validate_and_measure_placements`, which is where Sol's `≤1.25x` is written; a
refusing confirmation short-circuits and the three modes short-circuit in
different places, so only a cell every mode admits prices the same work twice.
Those cells are flagged and the medians are taken over them alone.

| quantity | HEAD (miter) | kernel (`exclusive`) | `union` |
|---|---:|---:|---:|
| **envelope half**, median ratio over 17 cells | 1.000 | **0.1207** | — |
| envelope half, worst cell | 1.000 | 0.1337 | — |
| **whole confirmation**, median over the 7 comparable cells | 1.000 | **0.904** | **1.005** |
| whole confirmation, with `fast-contract-validator` armed | 1.000 | **0.470** | 1.046 |

The envelope half is **8.3x cheaper**: it builds no offset at all, walks 290
integer vertices instead of 377 floating-point ones, and its broad phase
certifies 95.3% of pairs with four integer comparisons.

The whole-confirmation number is smaller than that because the contract
validator is the other 90% of a confirmation
(`docs/experiments/parallel-compression-schedule/` §3 measured 97.9% of a
mode-34 confirmation's milliseconds in that loop). Arming
`fast-contract-validator` — which is verdict-preserving, and §8's equivalence
check says so on this round's own corpus — shrinks that half and the kernel's
saving becomes **2.1x on a whole confirmation**: a median 0.8114 ms → 0.3708 ms per
confirmation on the seven comparable cells.

`union` costs 1.005x / 1.046x, because on the cells the kernel admits it *is*
the kernel (0.46–0.48x with the certificate armed) and on the cells the kernel
refuses it pays the kernel and then the miter. Both are inside the budget.

**Wall-clock caveat.** Other campaign rounds run on this box. Timings are
medians over 15 **interleaved** passes — never blocked, so a load spike lands on
both arms — and the claim they support is a factor of 8, not a percentage. The
counts next to them (`boxCertifiedPairs`, `narrowSegmentPairs`,
`envelopeVertexTotal`) are exact and carry the same conclusion without a clock.

---

## 7. What is in the tree, what it can reach, and what it hands the next round

| file | what |
|---|---|
| `src/validation/round_envelope.rs` | the kernel, `#[cfg(feature = "round-envelope-kernel")]` (new file) |
| `src/search/round_envelope_gate.rs` | the battery's census instrument, same gate (new file) |
| `examples/round_envelope_battery.rs` | the battery driver, `required-features = ["round-envelope-kernel", "import-gate-shadow"]` (new file) |
| `search/general_fast.rs` | the one wire point + `round_envelope_layout_metrics`, feature-gated. `validate_and_measure_placements`'s miter path is unchanged |
| `geometry/general_polygon.rs` | `PolygonRing::grid_path`, feature-gated and read-only. `PolygonSet::offset` is byte-for-byte unchanged |
| `search/portfolio.rs` | `PortfolioSettings::round_envelope_kernel` and the `RoundEnvelopeArming` RAII guard, feature-gated |
| `examples/general_request_benchmark.rs` | the `rek` spec key, feature-gated |
| `search/mod.rs`, `validation/mod.rs`, `Cargo.toml` | the `#[cfg]`-gated module declarations, the feature, the example target |

**469 insertions and 0 deletions** against the seven pre-existing files. No line
of the production path was edited, only added beside.

### Arming, and how a mistake is refused rather than absorbed

* **compile time**: `round-envelope-kernel`, off by default. With the feature
  off none of it compiles.
* **run time**: `KernelMode::Off` by default *even when compiled* — the
  difference from `fast-contract-validator`, whose certificate is
  verdict-preserving and therefore defaults to armed. This one changes the
  acceptance authority, so an armed run is a different engine and has to be
  asked for.
* **who may arm it**: only `run_portfolio`, only on the v3 path, through an RAII
  guard that puts the previous value back — because a leaked arming would
  silently make every later request in the same process a different engine.
* **the key**: `rek=0|off`, `rek=1|union`, `rek=2|exclusive`. A build without
  the feature exits non-zero with `unknown portfolio spec key "rek"`; a build
  *with* the feature exits non-zero on `rek=yes` with
  `rek takes 0/off, 1/union or 2/exclusive`. A mode key that fell back to a
  boolean would silently pick an arm.

`evidence/smoke.json` runs all six cells and all six checks pass.

### The metric basis moves with the envelope, and the next round must say which it reads

A miter corner reaches `expansion / sin(half-angle)` along its bisector, capped
at `2 × expansion`; a disc reaches exactly `expansion`. So the round envelope's
bounding box is the material's box grown by `expansion` and **nothing more**,
and an armed run's `used_long_axis_depth_mm` is smaller than a miter run's on
the same layout by whatever the binding corner's excursion was — Gate A §4
measured 0.377 mm of that on one placement. The **raw source** depth
(`raw_source_long_axis_depth_mm`), which this repository's records are quoted in,
reads no envelope at all and is untouched by either.

In `union` mode the round envelope's metrics are reported **even on the rows the
miter half admitted**, deliberately: a depth that changed basis at the
one-micrometre row where the two authorities disagree would put a step of the
miter excursion into the quantity the search minimises, at a boundary the search
can walk across.

### For the next agent

1. Use **`rek=1`**. `rek=2` aborts a bare-request run at the constructor's own
   self-check, for the reason in §3, and that is a property of the constructor's
   geometry rather than a bug to fix in the kernel.
2. The twelve pinned parents are all `union`-valid at 0.002, and all twelve are
   `exclusive`-valid at 0.0005 and at 0.0 — so a matched gate can load any of
   them in any mode at the record-lineage allowance. **Six of the twelve** are
   not `exclusive`-valid at 0.002 (seeds 0, 2, 4, 5, 10, 11), and neither is a
   fresh constructor output.
3. Compare depths on the **raw source** basis, or say explicitly that the
   envelope basis moved. §7's paragraph above is the reason.
4. Sol review 12 §3.2's remaining kill is untouched by this round:
   *"equal-operator-wall: ≥8/12 vittorie e ≥1 mm mediano contro miter"*. Nothing
   here is evidence for or against it.

---

## 8. Reproduction, gates, suites, determinism

```sh
bash docs/experiments/round-envelope-kernel/drivers/collect.sh
bash docs/experiments/round-envelope-kernel/drivers/run-suites.sh
```

Do **not** pipe either into `tee` or `tail`: you will read the pipe's status
instead of the script's. Every exit status inside them is read directly on the
line after the command.

### The four pinned gates

Run on binaries rebuilt from the committed tree, twice: with the feature absent,
and with the feature **compiled but unarmed**.

| gate | pinned | feature OFF | feature COMPILED, unarmed |
|---|---|---|---|
| g1 | 206.869 / `8a7737381238fa4d` | ✅ | ✅ |
| g2 | 159.09233022733062 / `fa01012af1d559ae` | ✅ | ✅ |
| g3 | 159.07876040364795 / `e28fba007f8031d4` | ✅ | ✅ |
| g4 | 164.0375677990678 / `49f094d7e59a9008` | ✅ | ✅ |

`ALL_PASS: true` on both, and stronger than the pinned scalars: the two
binaries' **whole-document digests are identical on all four gates**, wall-clock
fields stripped. `evidence/gates-rek-base.json`,
`evidence/gates-rek-compiled.json`.

### Suites

`evidence/suites.json`, logs alongside. Every one `--release`, as the protocol
asks.

| suite | features | passed | failed | ignored | exit |
|---|---|---:|---:|---:|---:|
| 1 | `jagua-experimental` | 1293 | 0 | 2 | **0** |
| 2 | the protocol's full combo | 1357 | 0 | 2 | **0** |
| 3 | the example harness (`--example general_request_benchmark`) | 19 | 0 | 0 | **0** |
| 4 | `jagua-experimental,round-envelope-kernel` | 1307 | 0 | 2 | **0** |
| 5 | *supplementary*: the kernel's 14 tests in the **debug** profile | 14 | 0 | 0 | **0** |

Suite 4's 1307 is suite 1's 1293 plus this round's 14 unit tests, which is the
only difference between them. Suite 5 is not one of the protocol's four and is
run because every other suite here is `--release`, which is the one profile that
compiles the kernel's own `debug_assert!` on its certification domain *out*.

The campaign's known flake,
`free_material_multi_eviction_shrinks_retained_container_capacity`, **passed on
every suite in this round**; `run-suites.sh` carries the rerun-and-report-both
logic anyway and it did not fire.

Suite 4 is stacked on `jagua-experimental` for the **pre-existing** reason Gate
A verified on the base commit: `examples/general_request_benchmark.rs` names
`search::portfolio`, which is `#[cfg(feature = "jagua-experimental")]`, and it
declares no `required-features`, so any feature set without it fails to compile
the example.

### Determinism

* **Two processes, whole document less the timings**: identical, including
  `executableSha256`. The stripped set is listed in `determinism.py` by name
  rather than inherited, because `gatelib.strip_times` does not know these field
  names — and the protocol's own note about `milliseconds`,
  `leafMilliseconds` and `leafSharePercent` is honoured there too.
  `evidence/determinism.json`.
* **Two binaries**: `battery.json` and `battery-fcv.json` are produced by builds
  that differ only in `fast-contract-validator`, and `equiv.py` requires them to
  agree on **every** verdict — every exact critical clearance, every refused-row
  index list, every false-accept count. **0 differences.** That is both a second
  determinism check and this round's own confirmation, on its own corpus, that
  the contract certificate is verdict-preserving.

---

## 9. Caveats, stated rather than left to be found

* **One request.** Every number here is mixed-61 at the 5.0/5.0 exact-clearance
  contract, plus the Sparrow pose set on the same request. The mechanism
  generalises (`offset_miter(P, e) ⊇ P ⊕ disc(e)` strictly at every convex
  corner; `do_round` quantizes the output on every offset); "13 rows" does not.
* **No search ran.** This round measures verdicts and per-confirmation cost. It
  is not evidence that a round authority *finds* anything, and Grok review 7 §3
  is untouched: legality is not reachability, and 57 of the 61 Sparrow poses are
  off the 2.5° lattice *and* off 1.0°.
* **The 12-parent matched gate is not here**, by instruction. Neither is any
  search-side benchmark.
* **`exclusive` mode is not a promotion candidate** and this round does not
  present it as one. It is the certified-exact arm, and its 13 losses are the
  measurement that produced the union.
* **The `union` mode's soundness argument is structural, not exhaustive.** "The
  union cannot lose what the miter admits" is true by the definition of a
  disjunction; the battery confirms it on 51 cells rather than proving it on
  all layouts.
* **Wall-clock is polluted** (§6). Prefer the counts.
* **Population 2 is constructed, not inherited.** The contact-block round's
  refused outputs carry no placements, so this round built its own population;
  §4 says exactly how, and the construction is in the committed driver.
* **The canonicalization budget is derived, not measured.** `√2` µm follows from
  nearest-micrometre snapping and convex combination. The battery's largest
  observed deviation, 1.970 µm on a *floored* comparison, is consistent with it
  and does not test it independently.
* **"Bit-identical across platforms" is a derivation, not a measurement.** It
  follows from the predicates being integer comparisons with no division and no
  `f64`, and this round ran on x86_64 only. What *is* measured is bit-identity
  across two processes and across two binaries with different feature sets, on
  this machine.
* **The parallel confirmation path is not parallel when the kernel decides.**
  `validate_and_measure_placements_parallel` routes through the same wire point
  and the kernel's own loops are serial. On the measured corpus the kernel's
  envelope half is 0.05 ms against the miter's 0.44 ms, so this has not mattered
  yet; a round that puts the kernel under `parallel-compression-schedule` at
  scale should re-price it.
* **`GridSet::of` allocates.** One `Vec<(i64, i64)>` per ring per confirmation.
  It is inside the measured 0.05 ms and it is not free; a promotion that runs
  the kernel inside a candidate scan rather than a confirmation should reuse the
  buffers, as `relaxed-row-buffer-reuse` did for the scorer.
