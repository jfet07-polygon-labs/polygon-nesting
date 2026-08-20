# CurrentPoseOverlay: isolating the 2.5-degree snap, and the composability prize that didn't land

Sol review 5 §3 (`docs/sol-review-5-se2-and-pose-freedom.md`) asked for one thing
before any `CurrentAssignment`/`DirectionalPenetration` comparison is trusted:
build `StructuredGrid + CurrentPoseOverlay` so the entry-damage claim
(`+0.448 mm` median, `0.825 mm` at the 155.4 depth, both from the record-line
and orientation-floor rounds) can be measured **without also swapping the
catalogue, the pair-NFP table and the pressure model** the way
`CurrentAssignment` does today. This round builds that overlay, campaigns it
against the grid, and reports what happened — including a real bug the campaign
itself caught, and a negative on the prize the review actually cared about.

> **Round 6 corrections.** Sol review 6 §2
> (`docs/sol-review-6-premerge-v5.md`) returned this branch **MERGE CON
> CORREZIONI** with eight named findings. Every one of them is applied here and
> the numbers below are the *corrected* ones, not the v5 round's. Where a v5
> claim did not survive, the old claim is quoted and retracted rather than
> quietly overwritten:
>
> | § | Finding | Where |
> |---|---|---|
> | 2.1 | the regression test did not prove the overlay was consulted | [§0.2](#02-the-regression-tests-that-actually-fail-against-the-bug) |
> | 2.2 | the catalogue was deep-cloned to add a handful of entries | [§0.3](#03-killing-the-deep-clone) |
> | flag-off | the hot key sites called a helper by value; gates prove semantics, not speed | [§0.4](#04-flag-off-cost-lane-local-and-measured) |
> | 2.3 | the campaign ran `rollback=32`, a certified negative | [§2](#2-campaign-setup-rollback0-this-time), [§3.1](#31-downstream-the-v5-claim-does-not-transfer) |
> | interpretazione | the `+9` pairs were unclassified | [§4](#4-classifying-the-9-pairs-conservative-at-the-contract-boundary) |
> | interpretazione | the README's "we asked −0.3 mm" explanation of the 0/15 flips was false | [§7](#7-the-composability-prize-still-negative) |
> | 2.4 | `currentPoseOverlayEntries` was described as a piece count | [§1.1](#11-two-counters-not-one) |
> | 2.5 | no full suite on the feature combo the overlay compiles under | [§6](#6-gates-suites-determinism) |
>
> The deliverable is unchanged in kind: an **off-by-default experimental seam**.
> Nothing in this round enables it in the coordinator.

## 0. What was built

`GeneralRelaxedSettings::current_pose_overlay: bool` (compiled only under
`compression-schedule`, off by default, every existing constructor sets it to
`false`). When armed, `drive_compression_schedule` (mode 34):

* builds the `StructuredGrid` catalogue exactly as before —
  `build_surrogate_catalog`'s `StructuredGrid` branch is untouched, so the
  candidate space `random_candidate`/`seed_angle` can propose is the one it
  always proposed;
* separately computes `CurrentPoseOverlay`: one `OrientedSurrogate` per piece
  whose parent rotation is not already a 2.5-degree grid angle, keyed the same
  way the grid keys its own entries (`build_current_pose_overlay`), and folds
  it **in place** into the catalogue this call already sole-owns
  (`merge_current_pose_overlay`) — see [§0.3](#03-killing-the-deep-clone);
* seeds `initialize_complete_state` with the parent's own continuous rotation
  instead of `canonical_angle`-snapping it, for every combination this used to
  snap (not only `DirectionalPenetration`/`ContinuousUniform`/`CurrentOnly` —
  those already kept continuous angles);
* keeps the pressure model at `StructuredTrianglePoles` and the collision
  backend at `RollbackTriangle` throughout. No path to `DirectionalPenetration`
  opens because the overlay is armed.

Every consumer of `catalog.orientations` in the file is a point `.get()`/
`.contains_key()` — nothing enumerates the map — so adding entries changes
nothing about what a fresh `StructuredGrid` build produces or what candidate a
lane can propose; it only lets a placement that still holds its exact parent
pose *resolve* instead of erroring. That is the literal meaning of "a separate
lookup used only for warm-start/repair": once a candidate's own rotation is
accepted for a piece, `seed_angle` has already snapped it back onto the grid,
and the piece is grid-native again for the rest of the run.

### 0.1 The bug the campaign caught

The first campaign run reported **bit-identical** entry loss between the grid
and overlay arms on every one of 15 parents, despite the overlay correctly
counting 8-50 off-grid pieces per parent. The cause: rotation-key derivation
for the non-directional backend ran through `derive_rotation_key`, whose `else`
branch calls `canonical_angle` **unconditionally** — so even though
`initialize_complete_state` correctly seeded a placement's `rotation_deg` at the
continuous angle, every lookup that turned that placement into a catalogue key
re-snapped it before ever reaching the overlay entry. The overlay's own entries
sat in the map, unread; every score was still computed from the grid-snapped
shape.

The fix is `continuous_rotation_keys`, a single predicate that governs *key
derivation only* (never which pressure model's whole scoring branch runs — that
stays gated on `uses_directional_pressure()` exactly as before): true for
`DirectionalPenetration`, and now also true when `current_pose_overlay` is
armed.

This is worth stating plainly: **without this fix, a `CurrentPoseOverlay` that
"worked" (compiled, ran, produced valid output, never errored) would have
silently done nothing.** The bug was caught only because the campaign compared
arms and found a suspiciously exact match, not because anything failed loudly.

### 0.2 The regression tests that actually fail against the bug

> Sol review 6 §2.1: *"Il regression test non prova che l'overlay venga
> consultato. Usa quadrati simmetrici lontani … Sarebbe passato anche col bug
> originale."*

He was right, and it is measurable rather than arguable. The v5 round's only
overlay test,
`compression_schedule_without_overlay_snaps_continuous_parent_rotation`, uses
two symmetric, well-separated squares: a square's proxy surrogate barely moves
with rotation and the two pieces never interact, so both arms produce identical
numbers whether or not a lookup ever reached the overlay.

This round adds six tests built on deliberately rotation-sensitive,
*interacting*, asymmetric geometry — a 30x30 L with a 22x22 bite taken out of it
and a 26x5 bar, at `13.37°` (which `canonical_angle` snaps to `12.5°`, an
`0.87°` gap) — and covers all three of the options §2.1 offers:

| test | what it pins | §2.1 option |
|---|---|---|
| `current_pose_overlay_builds_the_exact_pose_not_the_grid_snap` | the overlay entry is field-for-field a directly-constructed surrogate at `13.37°`, and observably *unequal* to the `12.5°` grid entry | "bounds/score coinciding with a directly-constructed surrogate" |
| `current_pose_overlay_is_consulted_by_every_lookup_path` | `rotation_key`, `surrogate_key`, `memoised_surrogate_key` (miss *and* the cached hit after it), `oriented`, `local_shape_bounds`, plus the lane-local `continuous_rotation_keys` bit itself, on two lanes over one catalogue differing only in the flag | "test separati per tutti i percorsi di lookup e per entrambe le varianti di scan" |
| `current_pose_overlay_changes_the_scored_state_on_interacting_geometry` | `score_state` — the whole-state scorer the campaign's entry measurement is taken from — differs between the arms, with a guard asserting the fixture actually interacts so the inequality cannot be vacuous | "bounds/scores che differiscono dalla grid" |
| `compression_schedule_overlay_changes_entry_measurement_on_asymmetric_parent` | mode 34 end to end: `startDepthMm` and the first step's `boundaryLossBefore` differ between arms | the campaign's own measurement, at unit scale |
| `current_pose_overlay_counts_entries_and_off_grid_pieces_separately` | three bars of one geometry class, two at one off-grid pose: `entries == 1`, `offGridPieces == 2` | §2.4 |
| `current_pose_overlay_merges_into_the_catalogue_without_cloning_it` | `Arc::as_ptr` unchanged across the merge; grid entries preserved; overlay entry reachable | §2.2 |

**The verification that matters** is that these fail against the original bug
and the v5 test does not. Re-introducing the bug verbatim — deleting the
`#[cfg(feature = "compression-schedule")] let directional = directional ||
relaxed_settings.current_pose_overlay;` pair from `continuous_rotation_keys`, so
the predicate is again the pre-overlay directional-only one — and running all
seven overlay tests gives (full log:
`evidence/regression-test-bug-injection.log`):

```
compression_schedule_overlay_changes_entry_measurement_on_asymmetric_parent ... FAILED
compression_schedule_without_overlay_snaps_continuous_parent_rotation      ... ok      <- the v5 test
current_pose_overlay_builds_the_exact_pose_not_the_grid_snap               ... ok      <- catalogue-level, bug is downstream
current_pose_overlay_changes_the_scored_state_on_interacting_geometry      ... FAILED
current_pose_overlay_counts_entries_and_off_grid_pieces_separately         ... ok      <- counting, bug is downstream
current_pose_overlay_is_consulted_by_every_lookup_path                     ... FAILED
current_pose_overlay_merges_into_the_catalogue_without_cloning_it          ... ok      <- installation, bug is downstream
test result: FAILED. 4 passed; 3 failed
```

The three that fail are exactly the three that assert a *lookup returned the
overlay's shape*; the four that pass are the ones testing construction,
counting and installation, which the bug left intact. That is the shape a
correct regression suite should have, and it is the concrete demonstration that
the v5 suite did not have it.

### 0.3 Killing the deep clone

> Sol review 6 §2.2: *"`catalog.orientations.clone()` clona poligoni, triangoli,
> assi, poles e indici per tutte le rotazioni solo per aggiungere poche entry. È
> esattamente il tipo di costo setup che non possiamo introdurre nel path 10s."*

`catalog_with_current_pose_overlay` is gone. `merge_current_pose_overlay` takes
the `Arc<SurrogateCatalog>` `build_surrogate_catalog` just returned — a fresh
`Arc::new` handed to exactly one caller, with the lane that will share it
(`LegacyLaneSearch::new`) constructed *after* the merge returns — takes
`Arc::get_mut` on it, and moves the overlay's surrogates into the existing
B-tree. No polygon, triangle, cell-axis set, pole or cell index is copied.
`or_insert` keeps the grid's own entry wherever the two collide, so the base
catalogue's meaning is unchanged; `Arc::get_mut`'s `expect` is a loud local
failure in the tests that cover the overlay rather than a silent
reintroduction of the clone, and it is unreachable by construction (strong
count 1, weak count 0 at that point).

**Measured, before and after**, on the campaign's own fifteen parents. The
engine now reports its own setup price as `currentPoseOverlaySetupMs`
(`Instant` around the build plus the install, nothing else), so both sides are
measured with the *same* instrumentation: the "before" arm is a scratch build
of this same tree with only the installation step reverted to the v5 clone.
Driver: `drivers/setupcost.py`; evidence: `evidence/setup-cost.json`.

| installation | median | min | max | total over 15 parents |
|---|---:|---:|---:|---:|
| v5 `catalog.orientations.clone()` | **7.997 ms** | 0.035 ms | 9.050 ms | 102.216 ms |
| `Arc::get_mut` + move (this round) | **0.323 ms** | 0.032 ms | 0.617 ms | 4.514 ms |

Median **23.7x** cheaper, a median **7.62 ms** saved per schedule drive. The
`min` column is the one parent (`port-seed1`) whose parent happened to be
entirely grid-native, so the overlay is empty and both arms take the early
return: that row is the measurement's own control, and it correctly shows no
difference. The remaining 0.32 ms is `build_current_pose_overlay` itself —
building 8-50 real surrogates — which is the overlay's irreducible cost, not
the installation's.

For scale against the binding priority: 8 ms is 0.08% of a ten-second envelope
*per mode-34 invocation*, and the coordinator raises one `Schedule` action per
basin (`portfolio.rs`, `(ActionClass::Schedule, ActionPayload::Basin { .. })`),
so the v5 installation was a recurring tax rather than a one-off. Nothing in
this round arms the flag from the coordinator — `current_pose_overlay` appears
in exactly two files, the settings/engine and the benchmark example's
environment reader — so today that tax is paid only by a run that asks for it
on the command line.

### 0.4 Flag-off cost: lane-local, and measured

> Sol review 6 §2, "Flag-off": *"Quello che i gate non provano è l'assenza di
> regressione prestazionale: ora quei siti hot chiamano un helper passando
> `GeneralRelaxedSettings`. Renderei il booleano lane-local/inlined e misurerei
> il flag-off con la feature compilata."*

Both halves are done.

**Lane-local.** `continuous_rotation_keys` reads only `relaxed_settings`, which
a lane never mutates, so it is a lane constant. `LaneSearch` now carries
`continuous_rotation_keys: bool`, evaluated once in `LaneSearch::new`; all six
hot sites — the cached-pose-bounds path, both free scan bodies, `rotation_key`,
`memoised_surrogate_key` and the uncached derivation — read that field. The
free function survives with `#[inline]` and exactly one caller
(`LaneSearch::new`); the two scan bodies get the bit out of the same `self`
destructuring they already do. Grep for the call sites: one construction site,
five field reads, and two test assertions.

**Measured**, because the gates prove semantics and not speed. See
[§5](#5-flag-off-wall-ab-with-the-feature-compiled).

## 1. What the counters mean

### 1.1 Two counters, not one

> Sol review 6 §2.4: *"`currentPoseOverlayEntries` non conta i pezzi. Conta le
> key uniche `(geometry_class, angle, mirror)`. Istante duplicate possono
> collassare. Separare `offGridPieceCount` da `overlayEntryCount`."*

Correct, and the v5 README described the entry count as "pieces off the
2.5-degree grid", which it is not. The diagnostics now carry both:

* `currentPoseOverlayEntries` — how many *surrogates* the overlay had to build.
  A catalogue size. Two instances of one geometry class at one continuous pose
  share a key and collapse into one entry. **This is the cost measure.**
* `currentPoseOverlayOffGridPieces` — how many of the parent's *placements*
  arrived off the grid, i.e. how many pieces the snap would have moved.
  Always `>=` the entry count. **This is the damage measure.**

Honest note on how much this changed the numbers: **on all fifteen campaign
parents the two counts are equal**, so no published figure moves. The mixed-61
fixture's repeated pieces do not, on these parents, land two instances of one
geometry class at the same continuous angle. The fix is a correctness and
naming fix whose divergence is demonstrated by unit test
(`current_pose_overlay_counts_entries_and_off_grid_pieces_separately`: one
entry, two off-grid pieces), not by a campaign row. Both numbers are now in
`evidence/ab-campaign.json` per parent, so a future parent where they diverge
will show it.

## 2. Campaign setup (`rollback=0` this time)

Fifteen parents, all mode 34 (compression schedule), all from the same
request (`tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`):

* the twelve compression-schedule port parents (171.61-179.62 mm), replayed at
  the from-request allowance `0.002`;
* the three true-contract pins `156.9188`, `156.0914`, `155.4223`
  (`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-line-cascade/`),
  replayed at the record lineage's own allowance `0.0005`.

Two arms, equal work:

* **A — `grid`**: `StructuredGrid`, today's default (`current_pose_overlay =
  false`).
* **B — `overlay`**: `StructuredGrid + CurrentPoseOverlay`, same pressure
  model, same catalogue, same candidate order.

> Sol review 6 §2.3: *"La campagna usa `rollback=32`, negativo già certificato …
> Il claim downstream 12/15 contro 9/15 non è trasferibile alla configurazione
> che spedisce. Rerun con `rollback=0` e settings reali v4."*

The v5 campaign ran `POLYGON_NESTING_COMPRESSION_SCHEDULE=past=1,rollback=32,
work=3341379`. `rollback_after_steps = 32` is not a neutral knob: the
compression-schedule port had already certified that arming it costs a paired
median of ~11 mm of published depth, which is exactly why
`CompressionScheduleSettings::default()` and the coordinator's own mode-34 call
site (`portfolio.rs`, "the port's own measured defaults, unmodified") both set
it to `0`. Every arm here now runs

```
POLYGON_NESTING_COMPRESSION_SCHEDULE=sweeps=6,confirm=4,rollback=0,repair=micro,step=1,past=1,work=3341379
```

which is coordinator v4's configuration written out in full — six repair sweeps
per step, a confirmation due every fourth step, `micro_legalize` on a refused
confirmation, one canonical grid unit per step, no rollback. The one
deliberate deviation from the shipping call site is `past=1` plus a work cap:
the shipping coordinator uses the slice *as* the bound and no cap, whereas an
equal-work A/B needs both arms to spend the same budget rather than stop at
whichever bound one of them reaches first. That deviation is stated here
rather than buried; `rollbacks: 0` and `exitCause: "workCap"` on all thirty
runs are recorded in the evidence so the configuration is checkable rather
than asserted.

Driver: `drivers/campaign.py`. Reduced table: `evidence/ab-campaign.json`
(built by `drivers/summarize.py`); raw per-run dumps are not committed
(~40 MB) and are reproducible from the driver.

## 3. A vs B: entry damage, not a promised gain

The overlay covers 0-50 off-grid pieces per parent, median 28. The `155.4223`
pin shows **49** — the exact number the record-line-cascade evidence and Sol's
own review already reported for that parent ("49 rotazioni su 61 fuori dalla
griglia 2.5°"), an independent cross-check that the count measures the right
thing.

| metric (overlay − grid, 15 parents) | median delta | direction |
|---|---:|---|
| entry loss (proxy tier's own units) | **−1420.4** | reduced on 11, increased on 3, unchanged on 1 (the parent with 0 overlay entries) |
| entry boundary violations (count) | **−2** | reduced on 11, increased on 0, unchanged on 4 |
| entry collision pairs (count) | **+9** | reduced on 0, increased on **14**, unchanged on 1 |
| entry proxy-feasible (bool) | — | **false on all 15, both arms** |

These are **unchanged from the v5 round**, and that is expected rather than
suspicious: every one of them is measured on the parent's entry state, before
the schedule takes a single step, so no schedule knob — `rollback` included —
can reach them. Sol said as much (*"gli arm sono appaiati, quindi la misura
d'ingresso resta utile"*). What the rollback rerun changes is everything
*downstream*.

So: **not zero, and not uniformly a gain.** Undoing the snap on the rotation
half of the pose recovers some of the fit the parent's translations were
calibrated for — boundary overflow falls, and total entry-loss magnitude mostly
falls — but the proxy tier's own pairwise collision-pair count rises on 14 of
15 parents. [§4](#4-classifying-the-9-pairs-conservative-at-the-contract-boundary)
takes that number apart pair by pair.

### 3.1 Downstream: the v5 claim does not transfer

The v5 README said:

> | | grid (A) | overlay (B) |
> | parents publishing below their own parent | 9 / 15 | **12 / 15** |
> | total drop summed over publishing parents | 4.136 mm | **5.984 mm** |

**Retracted.** At `rollback=0`, on the same fifteen parents at the same budget:

| | grid (A) | overlay (B) |
|---|---:|---:|
| parents publishing below their own parent | **13 / 15** | 12 / 15 |
| total drop summed over publishing parents | 16.315 mm | **18.553 mm** |
| confirmations attempted (= accepted, both arms) | 3,078 | 2,533 |
| rollbacks | 0 | 0 |
| median process queries/s (run 1 / run 2) | 1,005,057 / 1,144,952 | 986,700 / 1,165,986 |

Both arms publish far more, and far deeper, without the rollback — 16.3 and
18.6 mm of total drop against 4.1 and 6.0 — which is the port's certified
finding reproduced here. The **direction of the publication-count claim
reverses**: the grid arm now publishes on more parents, not fewer.

Counting publications is in any case the weaker statistic, because an arm can
publish on more parents and still end shallower on most of them. Paired on
`rawSourceDepthMm` (what each arm actually ended at, on the same parent, at the
same budget):

| paired published depth (overlay − grid) | value |
|---|---:|
| parents where overlay ends deeper | 7 |
| parents where grid ends deeper | 4 |
| tied | 4 |
| median delta | **0.000 mm** |
| sum of deltas | −2.238 mm |

7-4-4 with a zero median is **not a downstream result**. A two-sided sign test
on the eleven non-tied parents gives p ≈ 0.55. The honest statement is: at
`rollback=0`, on these fifteen parents, the overlay's downstream effect is
indistinguishable from noise, and the v5 round's "publishes more" claim was an
artefact of a schedule configuration that does not ship.

Queries/s came out 1.8% *lower* on the overlay arm in the first run of this
campaign and 1.8% *higher* in the second, with every semantic number identical
between the two (§6). This is a work-budgeted campaign on a box shared with two
other agents, so that field is reported for completeness and **no wall-clock
claim is made from it**; the wall measurement that is claimed is
[§5](#5-flag-off-wall-ab-with-the-feature-compiled), which is paired and
interleaved.

## 4. Classifying the `+9` pairs: conservative at the contract boundary

> Sol review 6, "Interpretazione dei numeri": *"Il `+9` coppie su 14/15 … non è
> neppure 'il prezzo atteso' finché non è classificato. Dice che il surrogate
> continuo è più conservativo, o più inaccurato, proprio alle rotazioni dei
> parent. Per ogni coppia nuova misurerei: collisione esatta material/envelope;
> risultato proxy grid; risultato proxy continuous; margine dal confine."*

`GeneralCompressionScheduleDiagnostics::parent_pair_classification` records
exactly those four things per pair, behind its own separate setting
(`current_pose_overlay_classify_pairs`, env
`POLYGON_NESTING_CURRENT_POSE_OVERLAY_CLASSIFY`) because it runs an exact-tier
offset-and-overlap bisection per pair: a run carrying it is a **diagnostic**,
never a timing measurement, and `campaign.py` explicitly unsets it on measured
arms. Driver: `drivers/classify.py`; evidence: `evidence/classification.json`.

For each pair, on the parent's entry state:

* `continuousProxyPenalty` — the proxy verdict with both pieces at the parent's
  exact poses (arm B's verdict);
* `gridProxyPenalty` — the same with both pieces snapped onto the grid (arm A's
  verdict);
* `materialOverlap` / `envelopeOverlap` — `polygons_overlap_exact`, the
  authoritative validator's own tier, on the parent's true geometry and on that
  geometry offset by the search envelope;
* `envelopeMarginMm` — signed distance from the envelope-feasibility boundary,
  by bisecting the symmetric offset at which the two *material* polygons first
  touch (40 iterations, ≈1e-12 mm resolution).

The margin is measured only *outward*, so a pair whose material polygons
already overlap saturates at `-2 * collisionExpansion`; `materialOverlap` is
the flag that says so, and it is `false` on every pair of every parent here
(these are exact-valid parents), so every margin below is a real measurement.

**Self-validation first.** The classification reconstructs both arms' verdicts
from a single run, by resolving each placement at both its continuous and its
snapped pose. That reconstruction reproduces **both** campaign arms' own
`parentCollisionPairs` counters exactly, on **all fifteen parents**
(`reconstructionMatchesAll: true`). Without that, none of the rest would be
worth reading.

### 4.1 The result

Across the fifteen parents: 410 pairs both proxies call colliding, **194 added**
by the overlay, **73 removed**.

| | count | envelope margin (min / median / max) |
|---|---:|---|
| flagged by both proxies | 410 | 0.001 / 0.001 / 0.002 mm |
| **added** by the overlay | 194 | 0.001 / 0.001 / **0.002 mm** |
| **removed** by the overlay | 73 | 0.001 / **0.113** / **1.025 mm** |

| verdict | count |
|---|---:|
| added, `conservative-at-boundary` (margin inside the band the grid proxy's own flagged pairs occupy) | **194** |
| added, `inaccurate` (margin outside that band) | **0** |
| added, `catches-real-conflict` (exact envelope really overlaps) | 0 |
| removed, `drops-a-grid-false-positive` (exact envelope clear, margin outside the band) | **67** |
| removed, `optimistic-at-boundary` | 6 |
| removed, `misses-real-conflict` | **0** |

**The answer to Sol's question is "more conservative", and it is not close.**

* Every single pair the continuous surrogate adds sits **1-2 µm** from the
  envelope-feasibility boundary — the *same* band, to the micron, as the pairs
  the grid surrogate already flags on its own. By margin, the overlay's new
  calls are indistinguishable from the shipping proxy's existing calls. Not one
  of the 194 is outside that band.
* Every pair the overlay *drops* that is not itself at the boundary — 67 of 73
  — is a pair with **0.1-1.0 mm** of genuine envelope slack that the grid proxy
  was flagging wrongly. Median removed margin is 0.113 mm, a hundred times the
  band.
* The overlay never drops a real conflict (`removedEnvelopeOverlap: 0`).

Why the band is 1-2 µm at all: this is the 5.0/5.0 exact-clearance contract, and
these parents are compression outputs, so their close pairs sit essentially
*on* the contract. The material gap of a flagged pair is 5.005 mm (port
parents, `0.002` allowance) or 5.003 mm (true-contract pins, `0.0005`
allowance) against a collision expansion of 2.502 / 2.5005 mm — leaving
0.001 / 0.002 mm of envelope clearance. At that separation *any* pole-and-
triangle proxy will disagree with the exact tier somewhere; the question is
only where, and the answer is that the continuous resolution disagrees on more
boundary pairs while agreeing with the exact tier on 67 pairs the grid
resolution gets wrong.

### 4.2 What this does and does not license

It **does** retire the v5 README's guess that "the round 2.5-degree angles the
catalogue always builds are not obviously a worse operating point for that
specific proxy". They are a worse operating point: the grid resolution produces
67 clear-cut false positives on pairs with real slack, and the continuous one
produces none.

It **does not** let anyone call the overlay "more accurate" outright, and this
is a limit of the parents rather than of the method: an exact-valid parent has
no real conflicts, so `catches-real-conflict` is 0 *because it cannot be
anything else*. Deciding conservative-versus-accurate in the direction that
would favour the overlay needs a parent that actually conflicts — which is
Sol's own next step ("costruirei una sweep causale intorno al confine
proxy-feasible") and is not this round's.

It also **does not** make `+9` free. 194 extra boundary-tight pairs are 194
extra pair penalties the schedule's repair sweeps have to work against, which
is visible in the campaign as the overlay arm attempting 2,533 confirmations
against the grid arm's 3,078 at the same budget. The price is real; what §4
establishes is that it is the price of conservatism at the contract boundary,
not of an inaccurate surrogate.

## 5. Flag-off wall A/B, with the feature compiled

Three binaries, all built `--features jagua-experimental,compression-schedule`
— the flag *compiled in* — and all run with
`POLYGON_NESTING_CURRENT_POSE_OVERLAY` unset:

* **`pre`** — `f32c629`, the commit before the overlay landed;
* **`v5`** — `f527bea`, the overlay with the by-value
  `continuous_rotation_keys(relaxed_settings)` call at every hot site;
* **`fix`** — this round, the lane-local `bool`.

Twelve paired rounds on two gate streams (`g1` = mode 20, ~26 s; `g2` = mode
22 on the 159.092 record parent, ~3.3 s), all three arms run back to back
within a round and the arm order reversed on odd rounds so a monotone drift in
machine load cancels between arms. The statistic is the **median of the paired
per-round deltas**, never a difference of separately-pooled medians. Every one
of the 72 runs re-checked its gate's pinned depth and fingerprint:
`gateMisses: 0`. Driver: `drivers/flagoff.py`; evidence:
`evidence/flagoff-ab.json`.

| gate | comparison | paired median delta | faster in | sign-test p |
|---|---|---:|---:|---:|
| g1 (m20) | `v5` − `pre` | +0.112 s (+0.42%) | 3 / 12 | 0.146 |
| g1 (m20) | `fix` − `pre` | +0.125 s (+0.48%) | 3 / 12 | 0.146 |
| g1 (m20) | `fix` − `v5` | +0.015 s (+0.06%) | 5 / 12 | 0.774 |
| g2 (m22) | `v5` − `pre` | −0.001 s (−0.03%) | 6 / 12 | 1.000 |
| g2 (m22) | `fix` − `pre` | +0.020 s (+0.60%) | 5 / 12 | 0.774 |
| g2 (m22) | `fix` − `v5` | +0.062 s (+1.93%) | 4 / 12 | 0.388 |

**Nothing here is significant, and that includes the fix.** Reported plainly
because it is a negative on my own change:

* The overlay commit's flag-off cost against the pre-overlay baseline is at
  most **+0.5%** on a 26-second mode-20 stream, at p = 0.146. Consistently
  signed (3 of 12 rounds faster, both arms) but not distinguishable from noise
  at this round count.
* **The lane-local rewrite does not measurably recover it.** `fix` − `v5` is
  +0.06% on g1 and +1.9% on g2, both directions, both p > 0.38. Whatever the
  by-value helper cost, it was below this measurement's floor — plausibly
  because `GeneralRelaxedSettings` is `Copy` and the predicate is two integer
  comparisons, so LLVM already had everything it needed to hoist the call out
  of the loop. Sol's instinct was reasonable and the change is still worth
  keeping — a lane constant read from a field is the honest expression of a
  lane constant, and it is what makes the `assert!(armed.continuous_rotation_keys)`
  line in the lookup test possible — but it is **not** a measured speedup and
  is not claimed as one.

Honest caveats on this measurement:

* The box is shared with two other agents. Three of the twelve rounds (9, 10,
  11) carry visible external contention — a 37 s g1 run and two ~8.7 s g2 runs
  against 26.4 s and 3.3 s medians. The paired median is insensitive to them;
  recomputing on the nine clean rounds only moves g1 `fix` − `pre` from +0.48%
  to +0.43% and leaves every p above 0.18. Both figures are in the evidence
  file's raw per-round rows.
* Twelve rounds with a sign test cannot call anything below roughly the
  round-to-round spread. That is the resolution being reported, not a proof of
  equality.
* The gates prove the flag is semantically inert; this proves only that no
  *large* flag-off regression is hiding behind that.

## 6. Gates, suites, determinism

**Four pinned regression gates**, on the gate binary the protocol names
(`cargo build --release --example general_request_benchmark --features
jagua-experimental` — `current_pose_overlay` compiled out entirely, because it
lives behind `compression-schedule`, which this build does not carry). All four
hit:

| gate | mode | pinned | reproduced |
|---|---|---|---|
| g1 | 20 | `206.869` / `8a7737381238fa4d` | yes |
| g2 | 22 | `159.09233022733062` / `fa01012af1d559ae` | yes |
| g3 | 22 | `159.07876040364795` / `e28fba007f8031d4` | yes |
| g4 | 22 | `164.0375677990678` / `49f094d7e59a9008` | yes |

Evidence: `evidence/gates-round6-flag-off.json`.

**Re-checked with the feature compiled in and the flag off** — the build the
overlay actually lives in — all four hit again:
`evidence/gates-round6-compression-schedule-flag-off.json`.

**Whole-document diffs**, not just the pinned scalars. `drivers/docdiff.py`
compares every leaf of the two benchmark documents with only the fields that
legitimately vary between runs of the same code removed:

| comparison | leaves compared per gate | leaves differing per gate |
|---|---:|---:|
| pre-round `f32c629` (`jagua-experimental`) vs this round's gate binary | 3,271 / 3,252 / 3,252 / 3,252 | **7** |
| this round `jagua-experimental` vs `jagua-experimental,compression-schedule`, flag off | 3,271 / 3,252 / 3,252 / 3,252 | **6** |

Every differing leaf is a build or run artefact rather than engine output:
`executableSha256`, the five wall-clock summary fields
(`min`/`firstQuartile`/`median`/`thirdQuartile`/`max` elapsed ms), and — in the
first comparison only — `engineWorktreeStatus`, which differs because more
documentation had been edited by the time the second run happened. (Note that
`engineWorktreeStatus` and `relevantSourceTreeSha256` are read from the
worktree at *run* time, not baked into the binary, so they identify when a run
happened rather than what ran; `executableSha256` is the field that identifies
the binary.) Every depth, fingerprint, placement and diagnostic count is
identical. Evidence: `evidence/docdiff-preround-vs-final.json`,
`evidence/docdiff-jagua-vs-csched.json`.

All gate and doc-diff evidence above was produced by binaries built from the
**committed** source. The campaign, classification, setup-cost and flag-off
measurements were run slightly earlier, from a source that differs from the
committed one by one rustdoc comment (the saturating-case note on
`envelope_margin_mm`); that binary's SHA-256 is recorded inside each evidence
file.

**Both suites.** Sol review 6 §2.5 asked for the full suite on the combo the
overlay compiles under, not only the protocol's `jagua-experimental`. Both were
run, each redirected to a log with the exit status captured on its own line
(`drivers/suites.sh`):

| suite | exit | test binaries | passed | failed |
|---|---:|---:|---:|---:|
| `--features jagua-experimental` | **0** | 55 | 1,250 | **0** |
| `--features jagua-experimental,compression-schedule` | **0** | 55 | 1,271 | **0** |

The 21-test difference is the `compression-schedule` module's own tests plus
this round's six overlay tests, none of which are compiled at all in the
`jagua-experimental`-only build — which is exactly Sol's point about the v5
round committing only the latter log. All six overlay tests appear as `ok` in
the combo log. `free_material_multi_eviction_shrinks_retained_container_capacity`
(the known-flaky one) passed on the first attempt in both suites; no rerun was
needed. Logs: `evidence/suite-jagua-experimental.log`,
`evidence/suite-jagua-experimental-compression-schedule.log`.

**Campaign determinism.** The `rollback=0` campaign was run twice end to end.
Every semantic number reproduces exactly — publications 13/12, total drops
16.314804 / 18.553122 mm, all three entry-delta tables, the paired
published-depth table. The only field that moved is process queries/s, which
went from "overlay 1.8% *below* grid" to "overlay 1.8% *above* grid" between
the two runs. That is the measurement saying, in its own voice, that queries/s
on this box is not a signal.

## 7. The composability prize: still negative

The prize the review actually named is not the entry-loss number — it is
whether an already-published state that fails `parentProxyFeasible` under the
grid *passes* under the overlay, because that is what would make an
`m33`/`m22`-produced continuous state directly composable with mode 34 instead
of paying the snap tax it pays today.

**Zero of fifteen parents flip**, unchanged from v5 and unaffected by the
rollback rerun (`parentProxyFeasible` is an entry measurement). Every parent
that is proxy-infeasible under the grid — all fifteen are — remains
proxy-infeasible under the overlay.

The v5 README explained this as:

> The overlay removes the *rotation-snap* contribution to infeasibility; it
> does nothing about the much larger contribution from asking a 0.3 mm
> compression a parent was not already sitting on.

> Sol review 6: *"`parentProxyFeasible` è misurato prima della compressione: la
> spiegazione 'chiedevamo −0.3mm' nel README è falsa."*

**That sentence was false and is retracted.** `parent_proxy_feasible` is taken
from `search.score_state(&state)` on the entry state, before the schedule's
first step and therefore before any depth below the parent's own is ever asked
for; the requested drop cannot contribute to it. The correct explanation is
[§4](#4-classifying-the-9-pairs-conservative-at-the-contract-boundary)'s: these
parents are exact-clearance layouts whose pairs sit *on* the contract boundary,
where the `StructuredTrianglePoles` proxy — under either resolution — reports
tens of collisions on a layout the exact tier certifies as valid. Proxy
feasibility is not within a sub-degree rotation correction of being reachable
on such a parent; it is dozens of boundary-tight pairs away, on both arms.

Per the task's own condition ("if yes, one demonstration"): no parent flipped,
so there is no demonstration to run. Sol's own next step —
*"costruirei una sweep causale intorno al confine proxy-feasible"* — is
correctly next-round work, and §4 now says exactly where that boundary is
(the contract-tight band), which is the input such a sweep needs.

## 8. Arm C: architecturally unreachable, not run

The plan asked for `C: CurrentAssignment + DirectionalPenetration, the
existing other engine, for reference`. It could not be run, and the reason is
itself a finding worth recording plainly rather than working around:

* Mode 34 (and every other persistent-vacancy mode) is reachable from the CLI
  through exactly one path: `dispatch_persistent_vacancy_mode`, called only
  from `run_coupled_dynamic_separator_experiment`, which is gated by
  `coupled_separator_configuration_error`. That gate **unconditionally**
  requires `pressure_model == StructuredTrianglePoles` (plus
  `RollbackTriangle`, `StructuredGrid` seed policy, and the exact protected
  8-lane/40-sweep/10-10-sample/5-refinement shape) — for *both* the control
  and the treatment arm, regardless of which mode is asked for. Setting
  `relaxed-pressure-model=directional` with `persistent-vacancy=34` produces
  no `persistentVacancyPopulation` at all; the JSON instead carries
  `treatment.skippedReason: "coupled dynamic separator requires the
  protected 8-lane, 40-sweep, 10/10-sample, 5-refinement structured route"`.
* The obvious workaround — warm-start the base engine from the same parent
  fixture (`warm-start-fixture`, argument 46) with `pressure-model=directional`
  and clamp the sheet to the target depth (`sheet-long-axis-override-mm`,
  argument 15) — also fails, and not for a reason specific to
  `DirectionalPenetration`: `load_pinned_vacancy_parent` validates a fixture's
  recorded sheet dimensions against the settings *after* the override is
  applied, so **any** sheet-long-axis override makes **every** pinned or
  warm-start fixture fail to load (`"parent fixture settings mismatch:
  sheetLongAxisMm fixture=2700 effective=173.908"`), independent of pressure
  model.

So today there is no CLI-reachable way to ask "the existing other engine" to
compress an already-complete parent toward a target depth at all — not a
narrow gap specific to mode 34, but a property of how the coupled-separator
gate and the fixture-settings check compose.

## 9. Honest limits

* Fifteen parents is what the task specified, not a claim about generality.
* The entry-collision-pair rise is now classified (§4) rather than explained by
  a plausible mechanism, but the classification cannot separate "conservative"
  from "accurate" on these parents *by construction*: an exact-valid parent has
  no real conflicts for a proxy to catch, so `catches-real-conflict` is 0
  because it cannot be anything else. What §4 does establish is the margin
  asymmetry between the pairs the overlay adds and the pairs it drops.
* The downstream result is a null (§3.1), not a gain. This branch is a seam and
  a measurement, not a depth improvement.
* The two overlay counters coincide on all fifteen parents (§1.1); their
  divergence is covered by unit test only.
* `past=1` and a work cap are a deliberate deviation from the shipping mode-34
  call site, needed for an equal-work A/B (§2).
* The box was shared with two other agents throughout. Every wall number here
  is paired and interleaved; the work-budgeted campaign makes no wall claim.
* Arm C is a documented non-result (§8), not a negative measurement.

## Files

* `drivers/campaign.py` — the A/B campaign (fifteen parents, both arms, equal
  work, `rollback=0`).
* `drivers/summarize.py` — reduces the campaign's raw per-run dumps to the
  evidence table.
* `drivers/classify.py` — the per-pair classification of §4.
* `drivers/setupcost.py` — the overlay setup cost, before and after §0.3.
* `drivers/flagoff.py` — the paired interleaved flag-off wall A/B of §5.
* `drivers/gates.py`, `drivers/lib.py` — the four pinned regression gates,
  copied from `constructor-inner-certificate` with `ROOT` repointed at this
  worktree.
* `drivers/docdiff.py` — the whole-document gate diff of §6.
* `drivers/suites.sh` — both full suites of §6.
* `evidence/ab-campaign.json` — the reduced `rollback=0` campaign table.
* `evidence/ab-campaign-v5-rollback32.json` — the v5 round's table, kept so the
  §3.1 retraction is checkable rather than asserted.
* `evidence/classification.json` — per-pair verdicts and margins (§4).
* `evidence/setup-cost.json` — the before/after setup measurement (§0.3).
* `evidence/flagoff-ab.json` — the paired wall A/B, with every round's raw
  values (§5).
* `evidence/gates-round6-flag-off.json`,
  `evidence/gates-round6-compression-schedule-flag-off.json` — the four gates on
  the `jagua-experimental` and `jagua-experimental,compression-schedule` builds,
  flag off in both.
* `evidence/docdiff-preround-vs-fix.json`,
  `evidence/docdiff-jagua-vs-csched.json` — the whole-document gate diffs.
* `evidence/regression-test-bug-injection.log` — the seven overlay tests run
  against the re-introduced bug (§0.2).
* `evidence/suite-jagua-experimental.log`,
  `evidence/suite-jagua-experimental-compression-schedule.log` — both full
  suites (§6).

Raw per-run dumps (campaign, classification, setup-cost and flag-off runs) are
not committed — roughly 100 MB of full benchmark documents — and are
reproducible from the drivers above. The binaries the measurements used are
under `/var/lib/t3/tmp/cpo6/bin/`, with their SHA-256 recorded inside
`evidence/ab-campaign.json`, `evidence/classification.json` and
`evidence/setup-cost.json`.
