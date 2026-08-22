# Gate A: Sparrow's 150.165 is contract-legal, the miter envelope refuses it, and the join is 100% of the reason

Grok review 6 §2 asked for one round: map the committed Sparrow 10-second x86
solution into an engine pose set and take three verdicts on it — contract only,
composite miter (HEAD's acceptance authority), composite round. Its
interpretation table then reads the answer off. Kimi review 1's margin note
asked for one more thing: the 131.98 mm area lower bound belongs to the retired
5.5/5.25 contract and needs re-pinning.

This is a **diagnostic round**. No default changed, nothing was promoted, no
search path was touched. The deliverable is three verdicts and what they mean.

---

## 0. The answer

| verdict | envelope radius 2.502 (from-request) | envelope radius 2.500 (`total_padding/2`) |
|---|---|---|
| **(a) contract only** — `validate_placements_against_contract` | **ACCEPTS** | **ACCEPTS** |
| **(b) composite miter** — `validate_and_measure_placements`, HEAD's authority | **REJECTS**: 37/1830 pairs, 4/61 boundaries | **REJECTS**: 31/1830 pairs, 2/61 boundaries |
| **(c) composite round** — same envelope, round join, shadow | REJECTS: 2/1830 pairs, 0 boundaries | **ACCEPTS**: 0/1830, 0/61 |
| composite square — Grok's alternative, shadow | REJECTS: 26 pairs, 2 boundaries | REJECTS: 19 pairs, 2 boundaries |

**Grok's case 3 obtains: miter rejects and the contract accepts.** The
representation *is* the residual, and Sol review 11's Certified Round-Envelope
Kernel is the named spend.

The round makes it sharper than the table asked for. At the contract radius —
`total_padding/2 = 2.5 mm`, the radius both Sol and Grok name — the round join
accepts the layout **entirely**, so:

> **31 of 31 pair refusals and 2 of 2 boundary refusals are caused by the miter
> join shape alone.** Not one of them is caused by the envelope radius.

At the shipping from-request radius (2.502 mm, i.e. the 0.002 mm search
allowance) two more pairs fall, and those two are radius-caused: their material
clearance, 5.000840 mm and 5.002879 mm, is below `2 × 2.502 = 5.004 mm`. That
is a second, separate tax — 0.004 mm of pair clearance against the join's
0.5057 mm median on a refused pair, a factor of 126 — and it is the only part
of the rejection a round envelope would not remove.

The price of the join, per pair, in material millimetres:

| population | max | median | min |
|---|---:|---:|---:|
| the 31 pairs the miter refuses at radius 2.5 | **2.3343 mm** | **0.5057 mm** | 0.0157 mm |
| all 56 pairs the shadow bisected, miter | 2.3343 mm | 0.0591 mm | −0.0014 mm |
| the same rows under the **round** join | +0.0022 mm | — | −0.0012 mm |

The round join's whole range is inside the derived quantization budget of
0.0036 mm — a round envelope costs the material *nothing* the grid can measure —
while the miter's median cost on a refused pair is **140 times that entire
budget**.

---

## 1. The import, and why it is not the artefact

A conversion artefact would fake all three verdicts, so the import is audited
before it is used. `drivers/import10s.py` loads the **committed** converter
(`docs/experiments/persistent-vacancy-descent/sparrow-to-hint-fixture.py`) as a
module, points its `SOLUTION` at `solution-10s-x86.json` instead of its
hard-coded `solution-3s.json`, calls its `main()`, and then re-derives every
placement and every transformed vertex independently.

**Every transformation, named.**

| | |
|---|---|
| units | millimetres in both frames; no scale (`depthStretchFactor = 1.0`) |
| rigid map | `(x_e, y_e) = (2000 − y_s, x_s)`; linear part `[[0,−1],[1,0]]`, determinant **+1**, **no mirroring** |
| rotation | `rotationDeg = sparrowRotation + 90`, normalised to `[0, 360)`. Both frames use `x' = x cos − y sin + t_x`, `y' = x sin + y cos + t_y`; `cos(θ+90) = −sin θ` and `sin(θ+90) = cos θ` make the +90 exactly the map's linear part |
| translation | `translateShortAxis = 2000 − t_y`, `translateLongAxis = t_x` |
| origin | both frames put the piece's local origin at the source DXF origin. Sparrow's `shape.data` is the request's `geometry.segments` `(x1, y1)` chain verbatim (`mixed61-to-sparrow.mjs:71`), and the engine's `PolygonRing` keeps that same untouched `f64` ring as `source_points`. **Neither recentres** to bounds or centroid |
| item map | `piece_ids[item_id]`. Verified, not assumed: Sparrow `items[i].id == i` and `items[i].dxf == request.pieces[i].id` for all 61, every `demand == 1` |
| sheet inset | Sparrow ran with a 5 mm inset on all four strip edges `(0, strip_width) × (0, 2000)`. Three map onto engine sheet edges; the fourth (`x_s = strip_width`) maps to the engine long axis at `y = 150.165`, **interior** to the 2700 mm sheet, so it imposes nothing here |

**Cross-checks against the committed validation** (`sparrow-mixed61/validation-10s-x86.json`):

| check | committed | this import | agreement |
|---|---|---|---|
| piece count | 61, each `demand` 1 | 61 placements, 61 distinct ids, covers the request exactly | exact |
| ring identity | — | all 61 Sparrow rings vertex-for-vertex equal to the request source chain | exact |
| pose identity | — | worst vertex error over all 61 rings | **2.27e-13 mm** |
| bounding extent | `minX 5.002254`, `maxX 145.16451`, `minY 5.011440089189676`, `maxY 1994.9937694148275` | `minY 5.002254`, `maxY 145.16451`, `minX 5.006230585172496`, `maxX 1994.9885599108104` | worst axis discrepancy **7.1e-15 mm** |
| hand-verified pair 1 | min pair `[38, 39]` at **5.000840472766719** | `[38, 39]` at 5.000840472766861 | **Δ 1.4e-13 mm** |
| hand-verified pair 2 | not named upstream (the runner-up) | `[50, 52]`: engine frame 5.002879392045225, Sparrow frame 5.002879392045327 | **Δ −1.0e-13 mm** |

Pair 1 is checked against the committed number; pair 2 has no committed number,
so it is checked against the invariant instead. The rigid map is a rotation, so
it must preserve distances — both pairs are recomputed in **both** frames with a
Python port of the committed `validate-sparrow-solution.mjs` measure and agree to
1e-13 mm, which is what says the map really is rigid rather than merely
relabelled.

**The one place the committed validation does not transfer.** Sparrow's
`minimumBoundaryDistance = 5.000959999999992` is `strip_width − maximumX`, its
*far* strip edge. That edge maps into the interior of the engine's 2700 mm
sheet, so the engine-frame binding edge is a different one: the long-axis origin,
at `min source y = 5.002254 mm` (which is Sparrow's `minimumX`). Every boundary
number below is that one.

**Depth.** In the engine's own published convention
(`raw_source_long_axis_depth_mm` = `max source y + sheet_edge_clearance`) the
imported pose set measures **150.16451 mm**, 0.00096 mm under Sparrow's reported
`strip_width` of 150.16547. Quote 150.16451 when comparing against this
engine's records.

Fixture: `fixture/sparrow-10s-x86-poses.json`. Its `placements` array is the
converter's output byte for byte — asserted equal to the independent
re-derivation, `converterMatchesIndependentDerivation: true`. Everything the
driver edits is metadata and is listed here rather than left to be diffed:
`description` and `engineContractSeparationMm`/`engineContractBoundaryMm`, which
still described the retired 5.5/5.25 era; `independentDepthMm`, which the
converter sets to Sparrow's `strip_width` and which this sets to the engine's own
`max source y + 5.0` convention (150.16451 against 150.16547); and the converter
identity and request-fixture provenance fields. The reason for each is recorded
in the fixture's own `contractNote`.

---

## 2. The instrument, and why a boolean was not the deliverable

`crates/polygon-nesting-core/src/search/import_gate.rs`, compiled only under
`import-gate-shadow` (default off, named by nothing in `src/` outside itself).
Verdicts (a) and (b) are the engine's own functions called unmodified. Verdict
(c) is built there and reaches nothing: no search path, no scorer, no
publication route. `PolygonSet::offset` is untouched; the shadow adds
`offset_with_join_shadow`, which takes the join as an argument, and **asserts at
run time** that its miter configuration reproduces `PolygonSet::offset` byte for
byte on every piece it measures before it trusts a single round number. That
assertion held on all three radii.

"The composite rejects" is compatible with two different worlds that demand
opposite spends — the join shape rejecting, or the radius rejecting — so the
instrument measures a quantity that separates them.

### The critical radius `r*`

For a fixed pair of poses and a fixed join,

```
r*(i, j) = max { r in integer micrometres : offset(P_i, r) and offset(P_j, r)
                 have zero intersection area }
```

Offsets are nested and increasing in `r`, so disjointness is monotone and `r*`
falls out of a bisection. It is exact: the canonical grid step **is** one
micrometre, so there is nothing to interpolate.

* For an exact disc join, two offsets are disjoint iff the **material** distance
  `d > 2r`, so `2 · r*` **is** `d` — to the grid, `2·r* ≤ d < 2·r* + 0.002`,
  which is the leading term of the quantization budget below.
* For any join containing the disc — miter and square both do — `2 · r* ≤ d`,
  and the deficit `join cost = d − 2 · r*` is the material clearance the
  representation spends on that pair and cannot give back.
* The composite accepts the pair iff `expansion ≤ r*`, so `r* − expansion` is
  the margin in the envelope's own units and `2(r* − expansion)` is it in
  material millimetres.

The boundary test gets the same treatment: `b*` is the largest radius at which a
placement's envelope still fits the inset rectangle.

### The round join's discretisation, and which way it errs

Clipper emits round-join points **on** the circle, so a round envelope is
*inscribed* — an under-approximation of `P ⊕ disc(r)`, and an under-approximating
envelope can accept what the true disc refuses. Clipper's default deviation is
`radius / 500` = **5 µm** at this radius: five whole grid steps, and larger than
every margin this gate measures — it would have decided the verdict by itself,
in the permissive direction. The driver
therefore sets the arc tolerance explicitly to **0.1 grid units = 0.0001 mm**, a
tenth of the grid step, and the resulting envelopes carry 20 601–20 669 vertices
against the miter's 377.

Five soundness checks ran on every row (`evidence/summary.json`):

| check | result |
|---|---|
| the shadow's miter reproduces `PolygonSet::offset` | **true on all three radii** |
| **the shadow agrees with the real validator**: `validate_and_measure_placements` short-circuits and names the lowest-indexed placement whose envelope leaves the inset sheet; the shadow enumerates. The shadow's lowest-indexed boundary failure must be the piece the real one names | **holds on all three radii** — `c5135087-…-copy-2` at 2.502, `0aff79d8-…-copy-1` at 2.5005 and 2.5 |
| containment: `r*_miter ≤ r*_round` and `r*_square ≤ r*_round` | **holds**; 9 rows invert by **exactly one grid step** (0.001 mm), the inscribed-arc-plus-vertex-rounding floor, and none exceeds it |
| disc identity: `d − 2·r*_round` inside the derived quantization budget `2·grid + √2·grid + 2·arcTol = 0.003614 mm` | **holds**, observed signed range **[−0.001243, +0.002226] mm** |
| monotone: failure counts may not rise as the radius falls | **holds** |

The second is the one that says the shadow's envelope half *is* the composite's
envelope half rather than a second implementation that happens to agree in
aggregate. The fourth is the one that says the round census really is measuring
`P ⊕ disc(r)` and not a coarse polygon.

Rows whose bisection saturates its ceiling — a placement so far inside the sheet
that its envelope fits at four times the contract radius — are **labelled**
(`criticalRadiusSaturated`) and excluded from every statistic, because a
saturated `r*` is a floor and not an answer. No *pair* row saturates in this
round; the saturated rows are all boundary rows with no boundary information in
them.

The decomposition in §0 is a **set intersection, not a subtraction of counts**,
and the difference is real. Round failures are not a subset of miter failures:
the round join is inscribed, so on a pair whose margin is under one grid step it
can land one micrometre *below* the miter's own `r*` and refuse a row the miter
accepts. That happens on exactly one row — pair 38·39 at radius 2.5005 — and
subtracting counts would have reported it as a join-shape failure that does not
exist. Those rows are counted separately as `roundRefusesMiterAccepts` and there
are no others at any radius.

Five unit tests pin the arithmetic the geometry does not show on its face — the
bisection's three outcomes (found exactly, saturated, refused at zero radius),
Clipper's arc-tolerance substitution rule, and that the production spec really is
miter at the production constants. They are the only tests suite 4 has that
suites 1–3 do not, which is the point of running it.

---

## 3. Verdict (b): which pairs, and by how much

Every refused row is in `evidence/miter-failures.json`, by Sparrow `item_id`.
At the contract radius 2.5 mm, the worst by join cost:

| items | material clearance | miter credits (`2·r*`) | join cost | clearance shortfall |
|---|---:|---:|---:|---:|
| 21 · 57 | 7.0843 | 4.750 | **2.3343** | −0.250 |
| 47 · 54 | 5.3377 | 3.578 | 1.7597 | −1.422 |
| 43 · 45 | 5.2544 | 3.496 | 1.7584 | −1.504 |
| 49 · 53 | 5.0721 | 3.400 | 1.6721 | **−1.600** |
| 14 · 56 | 5.3614 | 3.806 | 1.5554 | −1.194 |
| 15 · 18 | 5.1243 | 4.024 | 1.1003 | −0.976 |
| 18 · 33 | 5.1260 | 4.104 | 1.0220 | −0.896 |

Read the first row: items 21 and 57 have **7.08 mm** of material between them —
2.08 mm more than the contract asks — and the miter grid credits them **4.75 mm**,
which is less than the 5.0 mm the contract itself requires. The layout is not
tight there in any physical sense; the representation is.

Both boundary refusals at radius 2.5:

| item | binding material clearance | contract surplus | `b*` | shortfall | caused by |
|---|---:|---:|---:|---:|---|
| 14 | 5.125014 | +0.125014 | 2.188 | −0.312 | join shape |
| 15 | 5.206890 | +0.206890 | 2.213 | −0.287 | join shape |

Both are on the long-axis origin edge, and their miter envelopes cross the inset
line by 0.375 mm and 0.351 mm respectively while the material is 5.1–5.2 mm from
the sheet edge.

`validate_and_measure_placements` short-circuits: the verdict it actually
returns names only the first failure it meets, which at the from-request radius
is `piece c5135087-12f0-44f9-bd91-4bcf67affd8b-copy-2 violates the
canonical-grid sheet boundary`. The census above is what that one message is
standing in for — and the shadow's lowest-indexed boundary failure being exactly
that piece, at all three radii, is the soundness check in §2 that ties the two
together.

---

## 4. The boundary-semantics question

Grok asked it explicitly, because an asymmetry here changes the interpretation.

| authority | sheet-edge clearance it demands of the material |
|---|---|
| Sparrow, as validated | 5.0 mm (all four strip edges) |
| our **contract** (`validate_sheet`) | **5.0 mm** = `sheet_edge_clearance + sag` = 5.0 + 0.0, all four sheet edges, on `f64` source rings |
| our **composite**, round join | `inset + radius` = **5.002 mm** at the from-request allowance (5.0005 at the record lineage's 0.0005, 5.0 at zero). Flat — a disc reaches exactly `radius` past the material in every direction |
| our **composite**, miter join | `inset + k · radius`, where `k = 1/sin(half-angle)` capped at `CLIPPER_MITER_LIMIT = 2.0`. **`k` is a property of the pose, not of the contract.** |

Measured on this layout, `k = (binding material clearance − inset) / b*`:

| pose | item | binding material clearance | `b*` | measured `k` | miter demand at radius 2.502 |
|---|---:|---:|---:|---:|---:|
| 5 | 14 | 5.125014 | 2.188 | **1.19973** | **5.5017 mm** |
| 6 | 15 | 5.206890 | 2.213 | **1.22318** | **5.5604 mm** |
| 16 | 4 | 5.004304 | 2.500 | 1.00172 | 5.0063 mm |
| 4 | 24 | 5.128978 | 2.501 | 1.05117 | 5.1300 mm |
| 59 | 31 | (round join, for contrast) 5.002254 | 2.502 | **1.00010** | 5.0023 mm |

**So yes, there is an asymmetry, and it is not the allowance.** The allowance
asymmetry is +0.002 mm — real, and it is what refuses two of the 37 pairs. The
*join* asymmetry on this layout is **+0.56 mm** on the binding placement, with a
structural ceiling of `inset + 2 × radius = 7.504 mm` where the contract and
Sparrow both say 5.0 mm. A piece whose convex corner points at the wall is
charged **11% more** edge clearance than the contract asks on this layout, and up
to **50% more** in the worst case the miter limit allows — and nothing in the
contract says so.

---

## 5. Interpretation, per Grok's table

Grok review 6 §2's five-way read, applied:

> **3. Se il composito miter rifiuta e il contratto (o il round) accetta: la
> rappresentazione è il gap residuo da 5 mm; un A/B 10 s miter vs round è il
> round successivo, non questo.**

That is the case that obtains, and the round join accepting at the contract
radius makes it the strong form: the legal set of the *contract* contains
Sparrow's 150.165 layout, the legal set of `P ⊕ disc(2.5)` contains it, and the
legal set of the **miter grid does not**. Case 4 (miter accepts) and case 5
(contract rejects) are both excluded by measurement.

**What this does and does not license.**

* It licenses Sol review 11's Certified Round-Envelope Kernel as the named next
  spend, and it prices what that spend is buying: on this layout, the join is
  worth up to 2.3343 mm of pair clearance and 0.56 mm of edge clearance.
* It does **not** discharge Sol's own gate. Sol's item 1 asks for the round
  shadow against the source-ring validator on *three* populations — the canonical
  corpus, the committed material-valid/canonical-invalid proposals, and a ±1 µm
  boundary sweep — with **zero false accepts**. This round ran one pose set. It
  offered no opportunity for a false accept at all, because the contract accepts
  every row of it. The soundness checks here say the shadow is the composite's
  envelope half and that its round join is the disc; they say nothing about
  false accepts, and this document does not claim otherwise.
* It does **not** say a round authority would *find* 150.165. It says the
  authority would stop forbidding it. Grok's own §1 is untouched: the constructor
  saturates at ~180 mm in 1.4 s, 40 M → 120 M work buys +5.964 mm, and none of
  that is a representation question.
* It does **not** retire the allowance. Two of the 37 refusals at the shipping
  radius are radius-caused and a round kernel at 2.502 would still refuse them.
  Whether the allowance should be 0.002 is a separate and much smaller question
  (0.004 mm of pair clearance), and Grok review 6 §A.1 records that probe as
  having closed to noise.
* The **grid itself** is the floor under all of this. At radius 2.5 the round
  envelope admits the layout with **exactly zero** grid margin on pair 38·39 (its
  material clearance is 5.000840 mm, i.e. 0.42 µm of radius margin — *below* the
  1 µm canonical grid step) and +0.001 mm on the next. An outward-only
  discretisation with the error inside the margin, which is what Sol specifies
  and what a promotion would require, would refuse pair 38·39 at radius 2.5. The
  honest statement is therefore: **the miter join is the whole of the
  multi-millimetre refusal; the last micrometre of it belongs to the 1 µm grid,
  not to the join.**
* **It is one layout.** n = 1 pose set, 61 placements, 1830 pairs. That is
  enough to *falsify* "the legal set already contains Sparrow" — one
  counterexample does that — and enough to price the join on the rows it
  refuses. It is not a distribution over layouts, and this document does not
  report one. What generalises is the mechanism (`offset_miter(P, e) ⊇ P ⊕
  disc(e)`, strictly, at every convex corner) and the fact that it is worth
  millimetres rather than micrometres where it bites; what does not generalise
  is "31 pairs".
* **Legality is not reachability, and a second barrier is already measured.**
  Sparrow ran with continuous rotations. **57 of the 61 imported poses are off
  the engine's 2.5° surrogate-angle lattice** (`SURROGATE_ANGLE_STEP_DEG`,
  `general_relaxed.rs:75`; `canonical_angle` snaps to it), worst deviation
  **1.24586°**, and the 61 pieces carry 59 distinct rotations. A round envelope
  would stop *forbidding* these poses; the default relaxed lane still cannot
  *propose* them. That barrier is orthogonal to this round's finding and to
  Sol's kernel, and its own arm — `continuous-rotation` — is a measured
  −3.7 mm at ten seconds with `sparse-rotation` a null. Anyone costing the
  round-envelope kernel should cost this alongside it.

---

## 6. Kimi's re-pin: the area lower bound

`depth-lower-bound-evidence.json`'s `contract_bound_strengthened_mm =
131.97838540260466` is for the retired 5.5 mm pair / 5.25 mm boundary contract.
Re-pinned for this branch's exact-clearance 5.0/5.0:

|  | value |
|---|---:|
| **contract-native, strengthened** | **130.19990218310795 mm** |
| contract-native, plain (no depth-metric argument) | 125.19990218310794 mm |
| **composite-native** (the authority that actually publishes, radius 2.502) | **130.2140326353513 mm** |
| naive (raw area / 2000) | 105.30951629727141 mm |
| superseded 5.5/5.25 figure | 131.97838540260466 mm (**−1.7785 mm**) |

Derivation, every inequality in the safe direction — `r = 2.5`; pair separation
`≥ 2r` makes the disc-inflated pieces pairwise disjoint; material `x ∈ [5, 1995]`
puts them in a strip of width `2000 − 2(5.0 − 2.5) = 1995`; a `y`-extent `E`
gives `SUM ≤ 1995(E + 5.0)`; and `D = y_max + 5.0 = E + y_min + 5.0 ≥ E + 10.0`
with `y_min ≥ 5.0`, so `D ≥ SUM/1995 + 5.0`.

The construction is the committed script's, unchanged — the shoelace areas, the
exact Steiner formula for the convex pieces, the certified 0.02 mm grid lower
bound for the nine non-convex stars. Only the contract constants moved and the
depth term was re-derived. The check that says so is that the certified
`r = 2.5` inflated area agrees with the retired file's own
`sparrow_bound_mm × 2000` to **0.0 mm²**.

**Kimi's suggested replacement, 124.887, is not the right number either.** That
figure is `SUM_2.5 / 2000`: the full 2000 mm width, no boundary term, no
depth-metric term. It was written as a calibration of an outside packer under an
assumption about what Sparrow counts, not as this engine's bound.

**And the finding that dies with the old contract**: the retired file attributed
~7.09 mm of the engine-vs-Sparrow gap to "contract overhead". Sparrow's 5.0 mm
separation and this branch's contract are now the same number, so there is one
bound and not two. At the bound level the entire residual asymmetry is the
**0.0141 mm** the search allowance adds.

The bound has never been the binding constraint on this instance and still is
not: Sparrow's 150.16451 mm sits **19.965 mm** above the re-pinned bound, the
record 155.264 mm sits 25.064 mm above it.

Evidence: `docs/experiments/depth-lower-bound/depth-lower-bound-exact-clearance-evidence.json`.
Marked superseded in place in `depth-lower-bound-evidence.json` (a `SUPERSEDED`
block naming the four figures not to quote) and under the "defensible depth
floor" paragraph in `docs/next-generation-engine-plan.md`. The two review
transcripts that quote 131.98 (`grok-review-5-stop-and-consolidate.md:76`,
`kimi-review-1-the-band-audition.md:49`) are verbatim records of what a reviewer
said and are left untouched.

---

## 7. What was added to the tree, and what it can reach

| file | what |
|---|---|
| `crates/polygon-nesting-core/src/search/import_gate.rs` | the shadow instrument, `#[cfg(feature = "import-gate-shadow")]` (new file) |
| `crates/polygon-nesting-core/examples/sparrow_import_gate.rs` | its driver, `required-features = ["import-gate-shadow"]` (new file) |
| `geometry/general_polygon.rs` | `offset_with_join_shadow` + `production_offset_join_shadow`, both feature-gated. `PolygonSet::offset` is byte-for-byte unchanged |
| `validation/general_polygon.rs` | `material_pair_distance_mm` and `material_sheet_clearance_mm`, feature-gated: the distances the real validator already computes, without the comparison |
| `search/mod.rs` | the `#[cfg]`-gated module declaration |
| `Cargo.toml` | the feature and the example target |

With the feature off none of it compiles, and nothing outside it names any of it,
so a default build and both protocol feature sets are the shipping engine
exactly. The diff against the four existing files is **153 insertions and 0
deletions** (`Cargo.toml` 31, `general_polygon.rs` 66, `mod.rs` 5,
`validation/general_polygon.rs` 51) — no line of the production path was edited,
only added beside — and
the four pinned gates were re-run on a binary rebuilt from this tree and all four
hit:

| gate | pinned | reproduced |
|---|---|---|
| g1 | 206.869 / `8a7737381238fa4d` | yes |
| g2 | 159.09233022733062 / `fa01012af1d559ae` | yes |
| g3 | 159.07876040364795 / `e28fba007f8031d4` | yes |
| g4 | 164.0375677990678 / `49f094d7e59a9008` | yes |

The shadow instrument is deterministic across two processes as a whole document,
`executableSha256` included (`evidence/determinism.json`).

---

## 8. Reproduction

```sh
bash docs/experiments/gate-a-sparrow-import/drivers/collect.sh
bash docs/experiments/gate-a-sparrow-import/drivers/run-suites.sh
python3 docs/experiments/gate-a-sparrow-import/drivers/gates.py \
  gatea "$PWD/target/release/examples/general_request_benchmark"
```

`collect.sh` rebuilds both binaries, re-runs the import audit, the three
verdicts, the interpretation table, the named failures, the two-process
determinism check and the lower-bound re-pin, then writes `evidence/binaries.txt`
and stamps it into every evidence document. `run-suites.sh` runs the protocol's
three suites plus a fourth for the shadow feature, each with its exit status read
directly and never through a pipe — and do not pipe `run-suites.sh` itself into
`tee` or `tail` either, or you will read the pipe's status instead of the
script's. That is exactly how the first attempt at suite 4 here looked green when
it had exited 101.

Suite 4 is run as `jagua-experimental,import-gate-shadow` rather than
`import-gate-shadow` alone, for a **pre-existing** reason:
`examples/general_request_benchmark.rs` names `search::portfolio`, which is
`#[cfg(feature = "jagua-experimental")]`, and it declares no
`required-features`, so `cargo test` builds it for every invocation and any
feature set without `jagua-experimental` fails to compile it. Verified on the
base commit `1ca3315` with a feature set this round does not touch:
`cargo check --features shadow-rescore --examples` produces the same three errors
(E0432 plus two E0433 on `search::portfolio`). Running suite 4 on the shadow
feature alone would have measured that, and nothing about this round.

### Suites

Totals and exit statuses: `evidence/suites.json`, logs alongside.

| suite | features | passed | failed | ignored | exit |
|---|---|---:|---:|---:|---:|
| 1 | `jagua-experimental` | 1294 | 0 | 2 | 0 |
| 2 | the protocol's full combo | 1358 | 0 | 2 | 0 |
| 3 | the example harness (`--example general_request_benchmark`) | 19 | 0 | 0 | 0 |
| 4 | `jagua-experimental,import-gate-shadow` (this round's feature; in neither protocol set) | 1299 | 0 | 2 | 0 |
| — | *the flaky run of suite 1, kept* | 970 | **1** | 0 | 101 |

Suite 4's 1299 is suite 1's 1294 plus this round's five unit tests, which is the
only difference between them.

**The known flake, reported both ways as the protocol requires.** The campaign's
`free_material_multi_eviction_shrinks_retained_container_capacity` asserts
`cache.entries.capacity() < entries_capacity_before`
(`search/layout_scorer.rs:1450`) — an allocator property, not a search one. In
this round it **passed** on one run of suite 1, **failed** on another
(`evidence/suite-jagua-run2-flaky.log`: 883 passed, 1 failed; `cargo test` aborts
the remaining targets at the first failing one, which is why that log carries 5
result blocks and not 62), and **passed on the rerun**, which is the
`suite-jagua.log` on disk. It passed in suites 2 and 4 as well. `run-suites.sh`
now performs that rerun itself and reports both; the relationship between the
transcript and the logs is spelled out in `evidence/suites-runner-note.txt`.

### Caveat on wall-clock

Another campaign round was running on this box for part of this session, so no
wall-clock number here is a measurement of anything. Nothing in this round
depends on one: every claim is a verdict, a pinned depth, a fingerprint, or an
exact geometric quantity on a 1 µm grid.
