# Grok review 12 — reading Sparrow (2026-08-23, two rounds)

The owner authorized reading Sparrow's source (/var/lib/t3/tmp/sparrow-bench,
rev 14f4868f, upstream-identical; paper arXiv:2509.13329). Round 1 = the
source report + independent design; round 2 = quorum convergence after
reading Sol's reply. Verbatim.

# ROUND 1

## 1. What Sparrow’s search actually does

Source of record: `/var/lib/t3/tmp/sparrow-bench/src/` (the binary that produced the committed 150.165 fixture). Algorithms numbered as in their comments, paper [arXiv:2509.13329](https://doi.org/10.48550/arXiv.2509.13329). **Their code shows** vs **I infer** is marked.

### 1.1 The 10 s mixed-61 run, from their own log

`/var/lib/t3/tmp/sparrow-bench/output/log.txt`, seed 0, `--global-time 10 --workers 8 --min-item-separation 5`:

| clock | what the log says |
|---|---|
| 0 s | LBF constructor at **214.027** (49.2 %). Then `[SEP] separating at width: 214.027 and loss: 0` — already legal, so the first act is a shrink. |
| 0–8 s | Exploration. `shrink_step` **0.1 %** multiplicative. Hundreds of successful bites, most `[s:0,i:0] (S) min_l: 0` (one separator iteration). Last legal: **150.796**. |
| 8 s | First exploration failure: 150.646, `min loss: 374`, after 241 iterations. Disrupt: `swapping two large items (id: 14 <-> 13)`. Explore time expires. |
| 8–10 s | Compression from 150.796. Bites 0.050 % → 0.009 %. One fail at 0.044 %, recover at 0.015 %. Final **150.165**. |

Our constructor on the same request is **182.976**. Their LBF is 31 mm *worse*. The 10 s win is the shrink+separate loop, not a better initial layout.

Polygon simplification is on (`poly_simpl_tolerance: Some(0.001)` in `config.rs`). The log shows e.g. 68→9 edges, 0.086 % area change. **We do not copy this.** Our Φ and both exact authorities are on source rings. S0 already proved that the fixture is legal on *our* geometry.

### 1.2 Moves — what they propose

**Their code shows: sequential single-piece global relocates of currently-colliding items. Not chain compaction. Not two-endpoint PGS. Not a gradient.**

`optimizer/worker.rs::SeparatorWorker::move_items` (Algorithm 5):

1. Collect every placed item with `ct.get_loss(pk) > 0`, shuffle.
2. For each still-colliding item: `sample::search::search_placement` (Algorithm 6), then **always** `move_item` to the returned transform.
3. `debug_assert!(new_w_l <= old_w_l * 1.001)` — the piece’s own **weighted** loss must not increase.

`search_placement` (`sample/search.rs`):

- Seed the pool with the **current** pose (so “stay put” is in the choice set).
- **25** focused uniform samples in the piece’s *current bbox* (`UniformBBoxSampler` on `pi.shape.bbox`).
- **50** **container-wide** uniform samples in the strip bbox.
- Keep top `n_coord_descents = 3` unique samples (`UNIQUE_SAMPLE_THRESHOLD = 0.05 × min_dim`, angle 1°).
- Coarse coordinate descent on each of the 3; finer CD on the winner.

That container-wide half is the operator. A colliding piece can leave its neighbourhood and sit in any collision-cheaper pose the sampler finds. Our PGS cannot name that move.

`separator.rs::move_items_multi` (Algorithm 10): `n_workers` clones (3 default, **8 on the fixture**), each a different shuffle, keep the worker with lowest **total weighted** loss, throw the rest away.

**I infer:** the “coordination” is (a) a full colliding-set sweep in Gauss–Seidel order, (b) N random orders with a winner-take-all, (c) the shrink itself translating a half-layout together (§1.5). It is not a joint pair projection.

### 1.3 Local refine, not PGS

`sample/coord_descent.rs::refine_coord_desc`. Axes: ±x, ±y, ±(x,y), ±(x,−y), ±θ. Step ×1.1 on success, ×0.5 on failure (`CD_STEP_SUCCESS` / `CD_STEP_FAIL`). **Accept if not worse** (`tell`: `if !worse { pos = candidate }`). Equal is legal. Two stages (`consts.rs`):

| stage | translation | rotation |
|---|---|---|
| pre-refine | 0.25→0.02 × min_dim | 5°→1° |
| final | 0.01→0.001 × min_dim | 0.5°→0.05° |

No subgradient, no MTV, no incident-strict-decrease ladder.

### 1.4 Acceptance

**Their code shows, three nested rules:**

1. **Per sample.** `SampleEval` (`eval/sample_eval.rs`): `Clear` (loss 0) < `Collision { loss }` < `Invalid`. Lexicographic: any collision-free pose of *this piece* beats any colliding pose, regardless of global density. Separator loss is **weighted** (`eval/specialized_jaguars_pipeline.rs::calc_weighted_loss` = `weight * quantify_collision_*`).
2. **Per piece.** Commit the best sample. Current pose is a candidate, so this is “never increase this piece’s weighted loss,” not “must strictly decrease.”
3. **Per sweep.** Keep the worker with min total weighted loss. After `iter_no_imprv_limit` without a **raw**-loss improvement of 2 %+ vs the strike’s best, strike; after `strike_limit` strikes, return the min-**raw**-loss snapshot (feasible if 0, else least infeasible). Rollback restores poses and losses but **keeps weights** (`tracker.rs::restore_but_keep_weights`).

No Metropolis, no SA, no global-Φ veto on a single relocate.

**I infer:** the missing per-piece strict-decrease is real, but it is not “accept a worse piece.” It is “global relocate to the best weighted-loss pose we sampled, stay if nothing is better.”

### 1.5 Weights / penalties and their schedule

`quantify/tracker.rs::update_weights` (Algorithm 8), **every separator iteration**, every pair *and* every container-item entry:

- `loss == 0`: `weight *= 0.95` (`GLS_WEIGHT_DECAY`), floor 1.0.
- `loss > 0`: `weight *= 1.2 + 0.8 * (loss / max_loss)` (`GLS_WEIGHT_MIN/MAX_INC_RATIO` 1.2 / 2.0).

Continuous `f32`, all rows, every sweep. Not our integer “increment the single max-utility row on a stall.”

The **loss** being weighted is not signed-gap. `quantify/mod.rs::quantify_collision_poly_poly` (Algorithm 4): `sqrt(overlap_area_proxy(poles) + ε²) * shape_penalty`, poles in `overlap_proxy.rs` (Algorithm 3). Container: bbox-area leak. **We do not port this.** S0 already pins our signed-gap Φ to zero on their fixture.

### 1.6 How they shrink the strip

Two phases, both **from a legal parent**, both **through an infeasible child**, both **split-and-close** — not affine Y-compression of all centroids.

`separator.rs::change_strip_width`: split at `strip_width/2` (explore) or a **uniform random** x (compress). Every item whose **centroid is on the far side** of the split translates by `delta = new_width − old_width` (negative). Then the container is resized and the collision tracker rebuilt.

**Exploration** (`optimizer/explore.rs`, Algorithm 12):

- `next_width = current_width * (1 − shrink_step)`, `shrink_step = 0.001` (**0.1 %**, `config.rs`).
- Trigger: `separate()` returned `total_loss == 0`. Then shrink, **clear** the infeasible pool.
- Failed bite: **do not restore the legal parent and do not grow the strip.** Push the infeasible snapshot into a loss-sorted pool, restore a Normal(0, 0.25)-biased draw (better losses more likely), **disrupt**, stay at the **same** width, separate again. Default `max_conseq_failed_attempts = None` (the 10 s run did not pass `-x`).

**Compression** (`optimizer/compress.rs`, Algorithm 13):

- Always restore `best_sol` (last legal), then bite.
- `ShrinkDecayStrategy::TimeBased` (default): `step` interpolates `shrink_range = (0.0005, 0.00001)` = **0.05 % → 0.001 %** against `elapsed / time_limit`. Failure-based (`-x`) is off on the fixture.
- Success: `best_sol ← compacted`. Failure: `n_failed += 1`, **best_sol unchanged** (legal-to-legal on the compression path).

**I infer:** “small bites legal-to-legal” is the compression phase. Exploration is legal-to-infeasible with persistence at the new width until success or timeout. Sol 16’s ~3 % is **30×** their explore bite and is the wrong number. 0.1 % of our 182.976 is **0.183 mm**, inside the S1 basin the current PGS already closed.

### 1.7 How they split 10 s, and what they do at a stall

`main.rs`: `--global-time` → `DEFAULT_EXPLORE_TIME_RATIO = 0.8`, `DEFAULT_COMPRESS_TIME_RATIO = 0.2` (`consts.rs`). **8 s explore, 2 s compress.** `optimizer/mod.rs::optimize` (Algorithm 11) arms a fresh terminator per phase. `term.kill()` is wall-clock (`util/terminator.rs`), checked in the explore loop and inside `separate`.

Stall handling is **not** our jump ladder:

| where | their code |
|---|---|
| Separator inner | 200 (explore) / 100 (compress) iterations without raw-loss improvement → strike. 3 / 5 strikes → return best snapshot. |
| Exploration fail | `disrupt_solution`: swap two **large** items (convex-hull area in the top 75 % of total CH area), snap each rotation onto that item’s allowed set, then move every item whose POI lies inside a swapped shape by the same rigid transform (`practically_contained_items`). |
| Compression fail | discard the child, keep `best_sol`, smaller (time-decayed) bite. |
| Improving strike | if `min_loss < 0.98 * initial_strike_loss`, **reset** the strike counter. |

The one-shot highest-pressure strip/ball jump we built is **not in their code**. Disruption is a two-item swap, and it fires on a **failed separation**, not on a stalled sweep.

### 1.8 Rotation

**Their code shows: sampled, then wiggled. Not a catalogue, not a free continuous θ on the uniform samples.**

- `UniformBBoxSampler`: `RotationRange::Continuous` → **16** equally spaced angles (`2π / 16 = 22.5°`). Discrete sets used as listed; `None` → 0.
- CD `wiggle` axis **only if** `allowed_rotation == Continuous` (`search.rs::prerefine_cd_config`).
- Swaps: `convert_sample_to_closest_feasible` maps θ onto the allowed set; continuous keeps the sample angle.

Mixed-61 is continuous (`mixed61-to-sparrow.mjs` emits no orientations; conversion report `rotations: "continuous"`). Mirror is not a Sparrow move; our converter audited reflection-equivalence-by-rotation and did not encode mirrors.

### 1.9 What they do **not** have that we assumed

| we assumed | their code |
|---|---|
| True two-endpoint PGS / joint pair projection (Sol 16) | **Absent.** Sequential single-piece relocates. |
| Chain / component Y-compaction as a search operator (Grok 11) | **Absent.** The half-layout translation is the **shrink**, not the separator. |
| 2–3 % epochs | Explore is **0.1 %**. Compress is **0.05 % → 0.001 %**. |
| One 10 % gulp / affine centroid squeeze | **Absent.** Split-and-close, from legal. |
| Strict `after < before` on incident energy as the only step | Absent. Sample-best, accept-equal, current pose in the pool. |
| One topology jump of the highest-pressure piece | Absent. Swap-two-large on failed **separation**. |
| Signed-gap / MTV / subgradient | Absent. Pole overlap-area **proxy**. |
| Integer GLS, one row, on stall only | Absent. All rows, every iteration, ×0.95 / ×[1.2, 2]. |
| Exact geometry in the loop | Absent. Proxy `loss==0` is “feasible.” |
| Frozen-θ catalogue | Absent. 16-sample + wiggle. |
| Our millimetre-scale repair | Absent. |

**I infer (not in code, from the 10 s log):** 0.1 % bites almost always re-legalize in one separator iteration until ~150.6. The attractor we measured at 1.6 mm is an artefact of a 6.7 mm one-shot, not of their operating regime.

---

## 2. The member, specified

**Name:** sequential global relocate of the colliding set, with Sparrow-schedule GLS, axis CD as local refine, and a two-item swap only on a failed separation. **Not** two-endpoint PGS. **Not** chain Y-compaction as a search move. The half-layout translation lives in the regime (§3).

Dynamics learned from Sparrow, **implemented on our signed-gap Φ, our incremental rows, our dual-valid publication.** No `jagua-rs` in the `overlap-ics` tree. No pole proxy. No polygon simplification. No verbatim functions.

### 2.1 Moves

One **relocate** of piece `i` (the Algorithm-5 analogue):

1. If incident raw Φ of `i` is 0, skip (their `loss == 0` filter).
2. Build a sample pool with our existing `counter_hash` / `rotated_halton` (no `Xoshiro`, no `rand`). Current pose is always a candidate.
3. **25** focused samples: translation uniform in the piece’s current AABB, θ from the 16-angle set if `allow_rotation[i]`, else frozen.
4. **50** container-wide samples: translation uniform in the **usable strip at current `T`** (centroid AABB that keeps the rotated source bbox inside the sheet: physical edge on left/right/bottom, `T − depth_top_inset` on top). Same 16-angle draw.
5. Keep 3 unique (`0.05 × min_dim`, 1°).
6. Axis CD on our **incident weighted Φ**, two stages, same ratios as `consts.rs` (cite, do not copy). Wiggle only if `allow_rotation[i]`. Accept-equal.
7. Commit the best sample. Objective is lexicographic: incident Φ = 0 beats any positive; else min incident **weighted** Φ. Boundary rows already live in Φ, so “outside the sheet” is Collision, not a second predicate.

One **sweep**: colliding pieces in a deterministic permutation of `counter_hash(seed, bite, iteration, …)`, Gauss–Seidel. **One worker** this round (Sparrow’s 3–8 are variance reduction, not the mechanism). After the sweep, GLS update.

**Disruption** (only on a failed `separate` in exploration): swap the poses of two pieces whose convex-hull area is in the top 75 % of total CH area, different enough in area **or** diameter (their 1 % test), map θ through each piece’s allowed set (continuous: keep). Then apply the same rigid follow to every piece whose centroid lies in the swapped shape’s ring (our analogue of POI-contained; we do not have their POI — use **centroid-in-ring**, and cite the difference). Cap followers at `n` so a bug cannot move the whole layout.

### 2.2 Acceptance

- Relocate: best sample, accept-equal, current pose in the pool. **Delete** `descent.rs:426` `if after < before` as the relocate’s gate. That line is what Sol 16 named; it stays **only** if we keep PGS as a locked-strip instrument, which we do not.
- Sweep: no global-Φ veto. A relocate that raises global raw Φ but lowers that piece’s weighted incident Φ **commits**.
- Separate: track min **raw** Φ. `raw == 0` is the proxy-feasible flag. **It does not shrink the strip and does not write `best_exact`.** Shrink and incumbent require §2.4.
- Strike: 200 iterations without a 2 % raw-Φ improvement vs the strike best → strike; 3 strikes → stop the separate. Compress uses 100 / 5. Rollback to min-raw snapshot, **keep weights**.

### 2.3 Weights

Replace `energy::guided_update` (one integer increment on the max-utility row, stall-only) with Sparrow’s **all-rows, every-sweep** schedule, on **our** row scalars:

- `v = 0`: `w *= 0.95`, floor 1.0.
- `v > 0`: `w *= 1.2 + 0.8 * (v / max_v)`.

`w` is `f64`, stored next to the existing integer `penalty` **or** by promoting `penalty` to `f64 weight` with `guided = w * v²`. I recommend promoting: one guided path, no two GLS dialects. `clear_penalties` on a **successful** width change (their tracker is rebuilt on `change_strip_width`). Persist weights across rollbacks **inside** a width.

Do not weight the pole proxy. Do not keep the stall-only increment as a second schedule.

### 2.4 Publication (unchanged contract)

`publish.rs` is the only writer of `best_exact`. Continuous poses → `GridSet::of` → Exclusive `r = 2.500`, allowance 0 → frozen-θ same-strip repair ≤ 4n rows, ≤ 16 µm/piece → untouched `validate_placements_against_contract`. Band 4 µm. Minimum improvement 1 µm.

**Shrink trigger is a dual-valid publication at the new `T`, not `Φ = 0`.** That is the one place we are *stricter* than Sparrow, on purpose. Their `loss==0` is a proxy; our judge is exact. A Φ = 0 state that fails Exclusive or the contract does **not** count as a legal parent.

Attempt a checkpoint when `max_g ≤ 4 µm` (existing band). Same pose-digest de-dup.

### 2.5 What survives from overlap_ics, what is replaced

**Survives (do not rewrite):**

| module | why |
|---|---|
| `state.rs` | poses, SoA geometry, `Contract`, `ExactIncumbent`, `compose_proposal` (CD wiggle still turns about the transformed centroid). |
| `contact.rs`, `decomposition.rs`, `broad_phase.rs` | signed-gap Φ. S0 pin. |
| `energy.rs` measure / fold / incremental rebuild / census | the field. Replace **only** `guided_update`. |
| `publish.rs` | the judge. Caps stay. |
| `corpus.rs` | numeric soundness. Affine compression **stays as a corpus factory**, not as the live start. |
| `diagnostics.rs` | work vector, quality points. Add relocate-eval / container-sample / focused-sample / cd-eval / disrupt / bite counters. |
| FAST geometry: S0, 1k/10k corpora, default-build isolation, `jagua-rs` hygiene, four pinned engine gates | regression floor. |
| Constructor as anytime floor | `construct_short_side_first` behind `InitialLayoutProvider`. Fingerprint still never a child. |

**Replaced:**

| piece | with |
|---|---|
| `descent.rs` proposal/acceptance core (gradient + ladder + `after < before`) | relocate + axis CD. |
| `descent.rs::jump` / `on_stalled_sweep` jump ladder | gone as a search operator. Disruption is a **failed-separate** swap, in the explore loop. |
| `energy::guided_update` | all-rows Sparrow schedule on `v`. |
| `Engine::run` locked-strip PGS loop | explore/compress loop in §3. |
| `Engine::from_constructor` affine squeeze onto `T` | start at constructor poses, `T = D*`. Affine squeeze remains a test helper. |
| `homotopy.rs` stub | split-and-close + 0.1 % / compress decay. |

**S1 and triangle-20** are no longer “PGS republishes in a locked strip.” They become **locked-`T` relocate regressions**: same pins (dual-valid inside 150.16547 / 70.742, repair ≤ 16 µm, giveback ≤ 0.050 mm), new mechanism, new work counters. If relocate cannot republish a 0.5 mm perturbation of a known-legal layout, the member is broken before any shrink is licensed.

FAST S0 is **untouched**: budget 0, fixture poses, `phi.to_bits()==0`, depth 150.16451, dual-valid, zero repair. Relocate must not run on S0.

---

## 3. The regime, specified

**Initial `W`:** constructor’s legal raw-source depth `D*` (182.976 on mixed-61). No shock. No C175. No `T₀ = D* − 0.10(D*−L)`.

**Bite (explore):** `W ← W × (1 − 0.001)`. Split at **mid-depth** (Sparrow’s explore `split_position = None` → centre). Mapping: their strip **width** is our long-axis **depth**. Pieces with centroid **Y above** the split translate by `delta` (negative). Left/right/bottom edges do not move. Cite `separator.rs::change_strip_width`.

**Bite (compress):** same split-and-close, split Y uniform in `(edge, W)`, step from TimeBased `(0.0005, 0.00001)` against **phase-elapsed / phase-limit**, read **between** bites.

**Trigger:** dual-valid publication at current `W` whose raw-source depth is a strict 1 µm improvement **or** the first dual-valid child at this `W` (the constructor floor is at the old `W`). Then bite.

**“Re-legalize,” operationally:** after split-and-close, run `separate` (relocate-sweeps + GLS + strikes) at the new `W`. Proxy-feasible (`raw Φ = 0` and `max_g ≤ 4 µm`) **licenses a publication attempt**. Dual-valid success **is** re-legalization. Failure of Exclusive/contract/repair caps **is not** — stay in `separate`, do not bite again.

**Failed bite:**

- Explore: keep the infeasible child at this `W`, disrupt, `separate` again, until publication or explore-phase time (wall) or a fixed-work separate cap. **Do not grow `W`.** Do not restore the parent as a way to skip the width.
- Compress: restore last dual-valid `best_exact` (and its poses), try the next (smaller) TimeBased step.

**Never:** affine-compress all centroids; 3 % epochs; 10 % gulps; shrink on Φ = 0 without a publication; enlarge `W` to rescue.

### 3.1 10 s allocation

Match their 80/20, clock **between bites only**:

| phase | wall | what |
|---|---|---|
| constructor | until first exact-valid complete, hard-cap 1.4 s | floor. Already dual-valid. |
| explore | 80 % of **remaining** time after constructor, so ~7–8 s of a 10 s budget | 0.1 % bites + disrupt. |
| compress | the rest (~2 s) | 0.05 %→0.001 % from best published. |

One process, **one separator worker**. Sparrow’s 150.165 used 8 workers; that is a cited difference, not a silent rescue. If 1-wide publishes children but is rate-limited short of 168.484, that is the **one named diagnosis** in §4 (“throughput, not basin”) and the only thing a FAIL may license. If 1-wide does not publish a child, workers will not help.

Inside a bite: no clock. Strike limits and `iter_no_imprv_limit` are the work caps (200/3 explore, 100/5 compress).

### 3.2 3 s / 30 s

**Reported, not fatal.** Same 80/20 of remaining time. Full anytime curve of **published** raw-source depth at 3/10/30, no interpolation of the staircase.

- **3 s:** constructor-dominated is allowed and expected. A 3 s number equal to 182.976 is not a fail of the 10 s gate; it is the honest curve.
- **30 s:** whether bites continue after 10 s. Aspiration, not a kill.

---

## 4. The pre-committed reading, verbatim

Bind this text before any wall number exists. After a number arrives, only a result section may be appended.

### 4.1 Pass / fail (the only judge)

From the **bare mixed-61 request**, the **new engine only** (no v3, no `plancal`, no m34, no old stack as a lane), nine distinct seeds **0..=8**, **10 s wall**, one process, one separator worker:

**PASS** iff **≥ 3 of 9** seeds publish at least one **strict non-constructor** child with

- exact-valid raw-source depth **≤ 168.484 mm**,
- Exclusive `r = 2.500`, allowance 0,
- untouched contract validator,
- every publication of every seed dual-valid (a single invalid publication is a **FAIL**, even if some other seed is under 168.484).

**FAIL** otherwise. C175 is not a cell. Intermediate-cell substitutes (Φ = 0, `max_g`, proxy depth, “would publish if the band were 0.2 mm”, first-bite-only, constructor depth) **do not count**.

Report the 3/10/30 curve for all nine. The 3 s and 30 s numbers **cannot pass or fail** this gate.

### 4.2 The bar is the pinned 168.484, not a contemporaneous wall-arm

**Gate against 168.484.** That is the campaign’s published 10 s wall-arm depth (`docs/shipped-surface.md`: 168.484 at 10.30 s). The owner named it.

**Do not** make a contemporaneous interleaved wall-arm a pass/fail clause.

Why:

1. That arm **reproduces 0 of 3**. A paired test is a lottery that can false-pass (wall having a bad day) or false-fail (wall having a good day).
2. Mixing the old stack into this battery re-opens attribution (Grok 9: ICS is the only lane).
3. 168.484 is already raw-source. No envelope-basis conversion.

Run the wall-arm **once**, on this box, **after** the nine seeds, as a **reported control**. Print its depth next to ours. It cannot rescue a FAIL and cannot kill a PASS.

### 4.3 Regression floor (any break is a defect in *this* round, not a retarget)

Must remain PASS, on the same pins:

- **S0:** 61 placements, `rawSourceDepthMm` **150.16451**, `phi.to_bits() == 0`, Exclusive `two_r = 5000`, contract accepts, **0** repair, giveback 0. Fixture is not a seed and not a parameter source.
- **Soundness:** 1,000-state FAST and 10,000-state HEAVY: 0 outside the 4 µm band, 0 containment false-feasible, 0 incremental mismatches, force 100 % on the scored (`compressed`) population.
- **Throughput:** cold Φ ≤ 200 µs, row rebuild ≤ 20 µs, ≥ 1 M cell-gap evals/s. Proposal-rate pin is **re-denominated** (see §5): ≥ 100 K **relocate-evals** projected in 8 s, with `rawPhiBefore/After` and `acceptedRelocates` printed beside it so a skip-loop cannot fake the rate.
- **Four pinned engine gates** (`g1` 206.869 / `8a7737381238fa4d`, `g2` 159.09233022733062 / `fa01012af1d559ae`, `g3` 159.07876040364795 / `e28fba007f8031d4`, `g4` 164.0375677990678 / `49f094d7e59a9008`): whole-document identity, `overlap-ics` compiled and unarmed vs `jagua-experimental` alone.
- **Default-build isolation** and **`jagua-rs` absent** from `cargo tree --features overlap-ics`.
- **S1 / triangle-20** as locked-`T` relocate regressions (§2.5): dual-valid inside the locked strip, repair ≤ 16 µm, giveback ≤ 0.050 mm. Work quota for S1: 200,000 **relocate-evals** (not PGS proposals). Two-process bit-identity on the stripped document.

### 4.4 Forbidden rescues

A PASS obtained by any of these is a FAIL:

- Widening the publication band, repair cap, or giveback cap.
- Growing `W` to rescue a bite.
- Affine-compressing the constructor as the live start.
- Shrink-step, sample counts, strike limits, or 80/20 **fitted to mixed-61 / 168.484**. Those numbers are Sparrow’s defaults, cited, frozen **before** the nine-seed wall.
- 3 % (or any %) substituted for 0.1 % after seeing a number.
- Arming 8 separator workers, v3, `plancal`, m34, or the old wall stack as a lane.
- Importing `jagua-rs` into the `overlap-ics` tree, porting Sparrow functions, or searching on simplified polygons.
- Using the Sparrow pose fixture as a seed, a warm start, or a parameter source.
- Counting Φ = 0, `max_g`, or proxy depth as quality.
- Millimetre-scale legalization.
- Dropping Exclusive or the contract validator.
- Interpolation of anytime depths.

### 4.5 What a FAIL licenses

**If FAST / unit vectors name an implementation defect of the same concreteness as the last four** (a code line **and** a measurement: e.g. container-wide samples never commit, shrink on Φ = 0, split on X, wiggle about the pose origin) **→ one named repair, one re-run of this same gate, then stop.**

**If the member is the member in §2, the regime is the regime in §3, the floor in §4.3 is green, and ≥3/9 ≤168.484 is still false → nothing further on this family.** No worker-count round, no bite-size search, no PGS-pair retrofit, no chain operator, no homotopy bisection. Write the kill as: Sparrow-faithful relocate + 0.1 % split-and-close, on our Φ and our dual-valid judge, did not beat 168.484 at 10 s on 3/9.

The **one** exception that is a diagnosis rather than a new family: if ≥3/9 produce **strict dual-valid children** and the 10 s depths sit in a tight band **above** 168.484 (constructor 182.976 minus a handful of 0.1 % bites), and the first 0.1 % bite **does** publish, that is **throughput, not basin**. It licenses **one** follow-up that only raises separator workers, with this same gate. It does not license a new operator.

A first 0.1 % bite that does **not** publish on mixed-61 is a **member** fail, not a throughput fail. 0.183 mm is inside S1.

---

## 5. Determinism and honesty

### 5.1 What changes vs the overlap-ics contract

**Unchanged:**

- Fixed-work trajectories are a function of `(request, seed, binary, x86, toolchain, features, workers=1, quota)`.
- Two-process bit-identity after stripping the `wall` object.
- No `Instant::now` (or any clock) **inside** a bite, a sweep, a relocate, or CD.
- Wall fields live in one object. FAST smoke strips that object.
- `libm` is not required for trig; `f64::sin_cos` from degrees, as `state.rs` already pinned, for identity with publication.

**Changed:**

- **Wall mode is the 10 s gate.** Clock is read **between bites** (and once to split explore/compress). That matches Sparrow’s terminator and matches our “wall between batches” rule; a bite is the batch.
- Fixed-work smoke **adds** a bite-sequence cell: `K` explore bites, no clock, two-process identity of poses, Φ, publications, work counters. S0 stays budget 0. S1 stays locked-`T` relocate, no shrink.
- Work currency: **relocate-eval** = one incremental incident-Φ evaluation of one sample. A relocate charges focused + container + CD evals actually performed. A PGS `piece_proposals` counter that skips zero-energy pieces must not be the 10 s story.
- RNG: **no** `Xoshiro256PlusPlus`. Samples, shuffles, compress split-Y, and disruption draws are `counter_hash` / `rotated_halton`. Cite Sparrow’s use of Xoshiro as the thing we refused, so wall trajectories stay two-process identical.

### 5.2 Self-deception modes of a shrink loop (pre-named)

| mode | how it would fake a PASS | how we refuse it |
|---|---|---|
| **Bite-size fitted to 168.484** | 3 % (or “whatever reaches 168 in 80 bites”) chosen after a scout run | 0.1 % / (0.0005, 0.00001) / 80/20 frozen from Sparrow defaults **before** the nine-seed wall. Changing them is a forbidden rescue. |
| **Constructor does all the work** | 10 s number is 182.976, or 182.8 from one lucky millimetre | PASS requires a **strict non-constructor** child ≤ 168.484. Constructor fingerprint is never a child. Report `from_constructor` on the incumbent. |
| **Giveback hiding in re-legalization** | Φ = 0 at `W`, shrink, exact later “repairs” by 0.5 mm, quote the proxy depth | Shrink only after dual-valid. Repair ≤ 16 µm or the checkpoint is discarded. Quality is published raw-source depth, never Φ, never `max_g`, never unrepaired proxy depth. |
| **Proxy-legal parent** | bite from `Φ = 0` that Exclusive would reject | Publication is the trigger. A FAST vector: a Φ = 0 unpublished state must leave `W` unchanged. |
| **Neutered relocate** | 50 container-wide samples evaluated then rejected by a leftover `after < before`, so only local CD commits | FAST vector: a relocate must be able to commit a pose farther than `ladder_top` from the current pose, and the counters must show `containerSamples > 0` **and** `containerCommits > 0` on a fixture with a distant vacancy. |
| **Wrong-axis split** | translate in X, or affine-squeeze leftovers in `from_constructor` | Unit vector: after a bite of `δ`, pieces below the split have `ty` unchanged to the bit; pieces above have `ty += δ`; `tx` and `θ` unchanged. `from_constructor` live path does not call `compressed`. |
| **Simplification / inflation** | search on smaller/fatter rings than publication | No Sparrow `poly_simpl_tolerance`. Source rings only. |
| **Fixture as a seed** | warm-start from S0 poses and “find” 150 | S0 is budget 0. Anytime path loads the bare request. Fixture path is cells `s0/s1/s2` only. |
| **Counting failed-bite infeasibles as quality** | min-loss 374 at 150.646 quoted as progress | Quality series is `best_exact` only. Trace may log min raw Φ; the gate document’s depth field is published depth. |

---

## 6. Workflow plan

### 6.1 Module boundaries

Stay in `search/overlap_ics/` (feature `overlap-ics`). No new crate. No `jagua-rs`.

```text
state.rs, contact.rs, decomposition.rs, broad_phase.rs, publish.rs, corpus.rs
    unchanged contracts

energy.rs
    guided_update → all-rows Sparrow schedule on v; fold/incident/rebuild stay

relocate.rs                          NEW
    search_placement analogue: 25+50 samples, uniqueness, two-stage axis CD
    on incident weighted Φ. No gradient.

disrupt.rs                           NEW
    two large-item swap + centroid-in-ring followers

homotopy.rs                          REPLACE stub
    split_and_close(delta, split_y), explore step 0.001, compress TimeBased
    affine compressed() stays as corpus helper, not live start

descent.rs                           STRIP
    jump ladder deleted. propose/sweep become thin wrappers around relocate
    or are deleted once Engine::run no longer calls them.

mod.rs / Engine::run                 REPLACE loop
    from_constructor legal; explore/compress; publication-gated bite;
    wall between bites; fixed-work bite quota

examples/overlap_ics_benchmark.rs
    cells: s0, s1, triangle20, corpus, throughput, bite-sequence,
           wall-9seed (3/10/30), first-bite canary
```

Cite Sparrow in the module docs of `relocate.rs`, `disrupt.rs`, and `homotopy.rs` (files/functions above). Do not paste their functions.

### 6.2 Agents and commit order

**Three agents, two waves.** The last four defects (clearance-split, neutered jump, latent trio, rotation pivot) all came from an operator that was specified in one place and emptied in another. Do not fan this out to five parallel writers of `Engine::run`.

**Wave 1 — parallel, no loop:**

| agent | owns | done when |
|---|---|---|
| **A — member** | `relocate.rs`, GLS in `energy.rs`, strip of `descent.rs`, unit vectors: uniqueness, accept-equal, container commit beyond `ladder_top`, CD about `compose_proposal`’s pivot, wiggle only if `allow_rotation` | `cargo test --features overlap-ics --lib search::overlap_ics::` green; S0 still bit-identical; a 2-piece fixture with a distant vacancy relocates into it |
| **B — regime primitives** | `homotopy.rs` split-and-close, `disrupt.rs`, vectors: one-side Y shift, `tx/θ` frozen, compress step vs a fake elapsed, swap+follower, `from_constructor` does not affine-squeeze | same suite green; no call from `Engine::run` yet |

**Wave 2 — one integration agent, after A and B merge:**

| agent | owns | done when |
|---|---|---|
| **C — loop + FAST + wall** | `Engine::run` explore/compress, publication-gated bite, wall-between-bites, fixed-work bite quota, FAST additions (§6.3), 9-seed wall driver, the §4 reading committed **before** the wall is run | FAST green; S1/triangle relocate regressions green; first-bite canary green; then and only then the 9×10 s wall |

C does not retune A/B numbers. If FAST names a defect, C stops and A/B repair under §4.5.

No fourth agent for “just the driver.” The driver is how C lies; it stays in C.

### 6.3 FAST-tier additions

Keep `docs/experiments/overlap-ics/drivers/fast.sh` stages 1–6. Add, still minutes:

1. **Relocate counters:** on the 2-piece vacancy fixture, `containerSamples ≥ 50`, `focusedSamples ≥ 25`, `containerCommits ≥ 1`, committed displacement `> ladder_top`.
2. **Φ = 0 does not bite:** construct a state with `raw Φ = 0` and a publication that is **forced to refuse** (e.g. repair cap 0 and a 5 µm residual injected after the proxy fold, or a mocked refuse). `W` after `on_epoch` equals `W` before.
3. **Split-and-close bits:** one mixed-61 (or triangle-20) constructor layout, one 0.1 % bite, pose-bit assertions of §5.2.
4. **First 0.1 % bite canary:** mixed-61, seed 0, one bite + separate to publication or strike-out. **PASS = dual-valid child at `W = 0.999 × D*`.** FAIL here is a member fail; do not run the 9-seed wall.
5. **Bite-sequence two-process:** `K = 8` explore bites, fixed work, no clock, stripped documents identical.
6. **S1 / triangle** quotas in relocate-evals, pins otherwise unchanged.
7. Hygiene grep for `jagua-rs` **and** for `Xoshiro` / `rand::` under `search/overlap_ics/`.

HEAVY (round boundary, after FAST): 10k corpus, four pinned gates both binaries, 9-seed 3/10/30 wall, first-bite on all nine (reported).

### 6.4 The single most likely implementation defect

**Neutered relocate: the 50 container-wide samples run, then a leftover incident-strict-decrease (or a leftover PGS ladder, or a “max step = ladder_top”) rejects every sample that leaves the neighbourhood, so only axis-CD from the current pose commits.**

That is the same species as the last four:

| defect | what emptied the operator |
|---|---|
| clearance-split | Φ and publication charged different edges |
| neutered jump | 16 relocations became 4 self-moves, then an uncommitted eval spent the allowance |
| latent trio | jump scale / snapshot / box collapsed the choice set onto one point |
| rotation pivot | torque about the centroid, step about the origin |

A relocate whose committed poses all lie inside `ladder_top` of the parent is PGS in a sampling costume. It will close S1 (0.5 mm, local) and die on the first bite that needs a piece to **leave** the cut. The FAST vector in §6.3.1 is the tripwire; if C’s first-bite canary fails and that vector was green, look at the **commit filter**, not the sampler.

Second, close enough to name: **shrink on Φ = 0** (giveback hiding). FAST vector 2 is the tripwire. I still pick neutered relocate as #1, because this campaign’s implementation failures have been silent filters on the new operator, not forgotten if-statements on the schedule.

---

## Bound, for the other consultant

1. Sparrow’s member is **not** the operators we named. It is sequential **global relocates** + all-rows GLS + swap-on-failed-separate. Their bite is **0.1 %** split-and-close, not 3 %.
2. We implement those **dynamics** on **our Φ** and **our dual-valid judge**. Geometry, publication, S0, corpora, four pinned gates survive. PGS proposal, the jump, stall-only integer GLS, and affine live-start die.
3. Gate: **≥3/9 ≤168.484 at 10 s**, pinned bar, no contemporaneous wall-arm as judge, constructor is not a child, no Φ substitutes.
4. FAIL licenses one named implementation repair **or** (children exist, band is tight and high) one worker-count follow-up. Nothing else.

If you disagree on the member (you still want two-endpoint PGS or chain compaction **in addition** to relocate), the disagreement that matters is whether a second operator is licensed **before** the first-bite canary of 0.1 % + relocate. My reading: it is not. Relocate is what their 10 s log actually did; 0.183 mm does not need a chain; a chain that we have never built is a third experiment hiding in a second.


# ROUND 2 — quorum convergence

## 1. Material differences

| # | Difference | Ruling |
|---|---|---|
| **M1** | Routine move: Sol’s true joint pair projection + component-Y vs my sequential global relocate (25 focused + 50 container-wide, 3 unique, two-stage axis CD) | **DISAGREE.** Relocate is the member. Joint PGS is not in Sparrow. Component-Y is not in Sparrow. Both are a second experiment. Sol’s own §1 already says their success is evidence for small bites plus broad single-piece sampling and aggressive GLS, *not* for joint PGS. Funding joint PGS anyway tests the pre-reading hypothesis and confounds the proven-regime test. |
| **M2** | 75-pose sampler this round: Sol rejects it as a confound and as “uncomfortably close to recreating their optimizer”; I specified it as the routine move | **DISAGREE.** Under the owner’s reading (see §3) Algorithm 6 on our Φ is Egeblad/Imamichi-class work. Refusing it because it resembles the paper’s separator is refusing the funded experiment. What remains unlicensed is source text, `jagua-rs`, the pole proxy, polygon simplification, and Xoshiro. |
| **M3** | Strip/ball jump of the highest-pressure piece | **AGREE.** Out of the live trajectory. Gate-0 replay only. |
| **M4** | GLS: Sol’s \(w(1+g/g_{\max})\) / \(1+\tfrac78(w-1)\) vs Algorithm 8 on our \(v\): \(\times 0.95\) if inactive (floor 1), \(\times(1.2+0.8\,v/v_{\max})\) if active, all rows, every sweep | **DISAGREE** on the formula. The *schedule* (all rows, every sweep, persist across rollback, reset on a new bite from the exact parent) is **AGREE**. The multipliers are published Algorithm 8, cited, frozen before the wall, implemented on our row scalars — not a port of `tracker.rs`. Changing them is a fit. |
| **M5** | What weights decide: Sol — row priority, not whether a projection fires; me — the relocate’s sample objective (incident weighted Φ) | **AGREE** they never veto a finite move. For relocate they *are* the lexicographic sample score (incident Φ = 0 beats any positive; else min incident weighted Φ). That is Algorithm 5/6, not a PGS priority queue. |
| **M6** | Eight workers: Sol’s 8 whole-bite repair replicas vs my round-1 “1 this round, 8 only on a throughput diagnosis” | **Converge, with a correction.** Eight workers *this round*, because the 150.165 log is `--workers 8` and the funded test is the proven regime. The model is Algorithm 10, not Sol’s whole-bite replicas: every separator iteration clones the master, each worker does one shuffled colliding-set relocate sweep, barrier on equal work, keep min total weighted Φ by `(weighted_loss, fingerprint, worker_ordinal)`. Re-sync from the winner. Completion order is never observable. They are not eight long-lived basins. Drop my “one worker-count follow-up” license — we are already at eight. |
| **M7** | Explore bite-failure: persist at \(W\) + disrupt vs restore-parent + halve \(\beta\) + new cut | **DISAGREE.** Algorithm 12 is persist at the new width, pool the least-infeasible, disrupt, separate again. Restore-parent + decaying bite is Algorithm 13 (compression), which we keep *there*. Halving \(\beta\) on explore-fail is a different homotopy and is how we would silently leave the 0.1 % regime that produced 350 children. The one disruption in the 10 s log did not create 150.165; it is still the specified fail path, not the routine move. |
| **M8** | Time split: 80/20 of remaining after constructor vs no-split continuous batches | **DISAGREE** with no-split. Algorithm 11 is explore then compress. Compress is the only path that restore-parents and decays the bite; deleting the split deletes half the regime. 168.484 is an explore-phase target, so missing compress successes cannot rescue or kill the gate — but the 3/10/30 tail is the compress path, and we run it. |
| **M9** | Wall-arm control: my run-once-after vs Sol’s interleaved AB/BA; 168.484 absolute judge either way | **KNOB, and I take Sol’s.** 168.484 remains the judge. Interleave AB/BA across seeds as a *reported* control. The historical arm is 168.484 at 10.30 s and reproduced 0/3; it is a quality reference, not an exact 10.000 s replay. The paired number cannot raise or lower the bar. I drop “run the wall-arm once after, never interleave.” |
| **M10** | Workflow: my 3 agents / 2 waves vs Sol’s spec-commit → core → schedule → evidence | **Converge to Sol’s staging**, with relocate modules rather than `projection.rs` / `component.rs`. Spec first was the miss in my round 1. Parallel A/B after the spec is still allowed; nobody but the schedule agent touches `Engine::run`. |
| **M11** | Clock: both start from the bare request; Sol names that Sparrow’s 10 s excludes import+LBF | **AGREE** on our clock. **Adopt** the finding. Document the asymmetry; do not move our start to post-constructor (see §2 and §5). |
| **M12** | Exact-parent drift at `checkpoint()` / `mod.rs:295` | **Adopt.** I named proxy-legal parents and giveback-hiding; I did not name this line. Mandatory. See §2. |
| **M13** | FAST `fast.sh:34` stale worktree default | **Adopt.** Defect, not a knob. See §2. |
| **M14** | Most-likely implementation defect: my neutered relocate vs Sol’s exact-parent drift | **Both.** Complementary species. Neutered relocate is this campaign’s silent-filter failure mode. Exact-parent drift is this loop’s silent-giveback failure mode. Each gets a FAST tripwire. Neither subsumes the other. |
| **M15** | First 0.1 % bite canary as a FAST *stop* before the nine-seed wall | **HOLD.** Sol did not name it as a stop. 0.183 mm is inside S1. A first-bite miss is a member fail, not a wall number. |
| **M16** | S1 / triangle-20 retargeted to locked-\(T\) relocate regressions | **HOLD.** Same pins, new work currency (relocate-evals). If relocate cannot republish a 0.5 mm perturbation of a known-legal layout, no shrink is licensed. |
| **M17** | `libm` for live trig | **DISAGREE.** Keep the existing pin: `f64::sin_cos` from degrees, same as publication (`state.rs:54–67`). Determinism is same-box, same-toolchain, same-target. Do not introduce `libm` on the live pose path. |
| **M18** | RNG | **AGREE.** `counter_hash` / `rotated_halton`. No Xoshiro, no `rand::` under `search/overlap_ics/`. |
| **M19** | Publication trigger is dual-valid, not \(\Phi=0\) | **AGREE.** The one place we are stricter than Sparrow, on purpose. |
| **M20** | Install `Publication.poses` on success; next \(D\) is published raw depth, not \(T\) | **AGREE**, and this is the operational content of M12. |
| **M21** | Gate: ≥3/9 ≤168.484 at 10.000 s, non-constructor, every publication Exclusive \(r=2.500\) and contract-valid | **AGREE.** |
| **M22** | FAIL license: my one named repair *or* worker-count follow-up vs Sol’s autopsy + rerun only for a line-level spec break | **Converge.** Eight workers are already in. FAIL licenses (a) one named implementation repair with a red/green vector, same gate, then stop; (b) a read-only funnel autopsy `bitesStarted → proxyBandReached → exactAttempted → dualValidPublished → ≤168.484`. No new operator, no \(\beta\) search, no PGS retrofit, no chain, no worker-count round. |
| **M23** | Throughput pin: 100 k relocate-evals vs 100 k joint-row projections | Relocate-evals. Joint-row projections die with the member. |
| **M24** | Weight cap \(2^{20}\) | **KNOB.** Harmless. Take it, plus floor 1.0. |
| **M25** | Cut sequence: explore always centre; compress seed-derived analogue of their random cut | **AGREE** in substance. Sol’s “advance to a low-discrepancy cut on explore-fail” is part of M7 and dies with restore+halve. |

---

## 2. Findings I did not name — ruled

All three are real. All three go into the spec commit. None of them changes the member.

### 2.1 Sparrow’s 10 s excludes import + LBF — **document, do not adjust**

Verified. `main.rs:38` turns `--global-time 10` into 8 s + 2 s. `optimizer/mod.rs:33–48` runs LBF (or a warm start) *then* `terminator.new_timeout(expl_config.time_limit)`. Import is in `main` before `optimize`. Their published “10 s” is optimizer-only.

Our clock starts at the decoded bare request. The 168.484 bar is our product’s 10 s wall-arm, not Sparrow’s terminator. Constructor (~1.4–1.5 s) sits inside our 10.000 s.

**Do not** start our clock after construction to mimic them. That would make a PASS incomparable to 168.484 and would hide the product cost. **Do** write the three-way asymmetry into the spec ledger and the gate document:

| clock | what it includes | what the number is |
|---|---|---|
| Sparrow `--global-time 10` | explore+compress only | 150.165; import and LBF extra |
| Historical wall arm | engine wall, max 10.30 s | 168.484, reproduced 0/3 |
| This round | bare mixed-61 request → 10.000 s | constructor inside the budget |

Report `constructor_wall_s` and `search_wall_s` on every seed. Quote neither as the judge.

### 2.2 Exact-parent drift at `mod.rs:295` — **mandatory; complementary to neutered relocate**

Verified. `Engine::checkpoint` writes `ExactIncumbent` from `publication.placements` / depth / fingerprint and leaves `state.poses` as the pre-repair continuous state. `Publication.poses` exists (`publish.rs:84`) and is the repaired layout. A homotopy that bites from the pre-repair poses, or that sets the next \(D\) from \(T\), stops being legal-to-legal and hides cumulative giveback.

This is the shrink-loop form of the proxy-legal-parent deception I named in §5.2. I missed the line. The forced-repair two-bite FAST vector is in the spec: inject a publication that spends repair, prove `Publication.poses` are installed, every cache rebuilt, next \(D\) equals measured published raw depth, next bite’s parent fingerprint matches that publication.

### 2.3 FAST driver’s stale worktree default — **fix in the spec-commit wave, before any core edit**

Verified. `fast.sh:34`:

```bash
ROOT="${ICS_ROOT:-/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_7f77514b-f9a-1}"
```

An agent can go green on the wrong tree. Default must be the current workspace (script-relative `git rev-parse --show-toplevel`, or require `ICS_ROOT` with no stale fallback). This is a round-validity defect, not an experiment knob.

---

## 3. The owner’s framing — explicit rulings

**Is relocate + GLS + disrupt within the no-copying constraint?** **Yes**, under the reading the owner just gave.

The campaign already designed from Egeblad 2007 and Imamichi 2009: published algorithms, cited, on our geometry and our judge. Sparrow’s comments name Algorithms 4–13 of arXiv:2509.13329. Implementing those *descriptions* on source-ring signed gap, incremental rows, Exclusive \(r=2.500\), and the untouched contract validator is the same method. The no-copying line still forbids:

- pasting Sparrow or jagua functions;
- linking `jagua-rs` into `overlap-ics`;
- their pole-area proxy, polygon simplification, CDE, or Xoshiro;
- using the 150.165 fixture as a seed, warm start, or parameter source.

It does **not** forbid a 25+50 sampler, axis CD, all-rows multiplicative GLS, or a two-large swap on failed separation, provided each is cited as the paper’s algorithm and executed on our field. Sol’s “uncomfortably close to recreating their optimizer” is the old “never open Sparrow” constraint. The owner amended that: read, cite, do not port. Recreating the *family’s published regime* is the assignment. Recreating their collision representation is not.

**Which experiment is funded?** **“Can OUR engine beat 168.484 with the family’s PROVEN regime.”**

Not “does our novel coordinated member work.” That was the pre-reading hypothesis (Sol 16 two-endpoint PGS; Grok 11 chain Y). Both of us opened the source. Both of us found sequential single-piece relocates and a cut-close shrink. The owner’s sentence is the disambiguation: Sparrow manages 0.1 % legal-to-infeasible bites with that separator, so can we, on our Φ and our dual-valid judge.

Consequences:

- Joint pair projection + component-Y is exactly the novel member. It is a separately funded proposal. Sol’s own failure-license language already says so about “another move family.”
- Adding it *this* round would confound the funded test the same way Sol says the sampler would confound *his* test. Under the owner’s question, the confound runs the other way.
- The cut-close translation of the far side *is* the routine coordinated move (we agree). It lives in the regime, not in a joint PGS operator.

---

## 4. One member for this round

**Final position:** `CutCloseRelocate`. One frozen combination.

Routine move = Algorithm 5–6 analogue: colliding pieces only, 25 focused + 50 container-wide, current pose in the pool, 3 unique, two-stage axis CD, accept-equal, commit best incident weighted Φ, no leftover `after < before`, no `ladder_top` cap on a relocate. Disruption = Algorithm 12’s rare fail path, not the normal move. GLS = Algorithm 8 on our \(v\). Homotopy = 0.1 % centre cut-close in explore; TimeBased 0.05 % → 0.001 % in compress, restore-parent on compress fail. Judge = Exclusive \(r=2.500\), allowance 0, untouched contract, shrink only after dual-valid publication. Geometry, SoA, rows, S0, corpora, four pinned gates survive.

**Minimal amendment that makes Sol’s design acceptable to me:** replace §2’s routine move (joint \(\lambda\)-projection, `rebuild_two_piece_rows`, component-Y Jacobi, “do not add 75-pose sampling”) with the relocate above. Keep his cut-close 0.1 %, publication trigger, exact-parent install, eight-worker *barrier discipline* (reinterpreted as Algorithm 10’s per-sweep tournament, not whole-bite PGS replicas), spec-first workflow, timer ledger, parent-drift FAST vector, and interleaved control. Change explore-fail from restore+halve to persist+disrupt. Use Algorithm 8’s published multipliers, not \(1+g/g_{\max}\). Add the neutered-relocate FAST vector and the first-bite canary as stops.

That is a full replacement of the operator and a small amendment to the rest of his spec. I will not sign a round whose live move is joint PGS or component-Y. If he will not drop them, there is no joint spec; the remaining refusal is **M1**.

---

## 5. The other knobs, converged

**Workers.** Eight this round, Algorithm 10 shuffle tournament, deterministic permutations from `(request_seed, bite, iteration, worker_ordinal)`, fixed work, barrier, serial ordinal merge. Not one worker. Not eight bite-replicas that diverge until publication.

**Explore bite-failure.** Persist at \(W\), least-infeasible pool, two-large swap + centroid-in-ring followers (cite the POI→centroid difference), separate again, until publication, explore-phase time, or the strike cap. Do not grow \(W\). Do not restore the parent to skip the width. Do not halve \(\beta\).

**Compress bite-failure.** Restore last dual-valid parent (installed poses), next TimeBased step, seed-derived cut.

**Time allocation.** Constructor once, inside the 10.000 s. Then 80 % of *remaining* wall to explore, 20 % to compress, clock read between bites only. Publications past a checkpoint do not count for that checkpoint. Same 80/20 on the 30 s continuation. No interpolation.

**Timer.** Document §2.1. Do not subtract constructor. Do not add a post-LBF grace.

**Wall-arm control.** Interleaved AB/BA across seeds, reported. 168.484 is the absolute judge. Historical 10.30 s / 0/3 noted.

**Workflow.** Four staged seats, three agents if core and schedule cannot overlap in time:

1. Spec commit: this gate, this member, Sparrow ledger at `14f4868f` with file anchors and the timer finding. No engine code. Fix `fast.sh` in this commit.
2. Core: `relocate.rs`, `disrupt.rs`, GLS in `energy.rs`, strip of `descent.rs`. Must not touch `homotopy.rs` or `Engine::run`.
3. Schedule, after the core API: `homotopy.rs`, exact-parent install, eight-worker barrier, `mod.rs`.
4. Evidence/red-team: example driver, FAST, fixed-work replay, wall control. Reviews the solver; does not rewrite it.

**FAST union** (every iteration; Sol’s tripwires plus mine):

- default-build isolation and `jagua-rs` / `Xoshiro` / `rand::` exclusion;
- module tests, S0, 1 000-state corpus;
- **neutered-relocate:** 2-piece distant vacancy, `containerSamples ≥ 50`, `focusedSamples ≥ 25`, `containerCommits ≥ 1`, committed displacement \(>\) `ladder_top`;
- **Φ = 0 does not bite** when publication is forced to refuse;
- **cut-close bits:** far-side \(t_y += \delta\), near-side \(t_y\) unchanged, \(t_x\) and \(\theta\) frozen;
- **exact-parent drift:** forced nonzero-repair publication then a second bite; poses installed, caches rebuilt, \(D\) = published raw depth, parent fingerprint matches;
- **first 0.1 % bite canary:** mixed-61 seed 0, same 8-worker binary, one explore bite; PASS = dual-valid child at \(W = 0.999 D^*\); FAIL here does not run the nine-seed wall;
- two-process fixed-work \(K=8\) explore bites, stripped documents identical;
- S1 / triangle-20 locked-\(T\) relocate regressions, 200 k relocate-eval quota on S1;
- one release throughput sample, ≥ 100 k relocate-evals projected into 8 s.

---

## 6. The spec we would both sign

Remaining refusal, if Sol still holds §2: **the routine move is relocate, not joint PGS.** Everything below assumes that amendment. I will not sign the block if `JointSmallBiteSeparator` is still the named member.

### 6.1 Name and freeze

**`CutCloseRelocate`**, feature `overlap-ics`, one combination. Source of dynamics: arXiv:2509.13329 Algorithms 4–13, as read at Sparrow `14f4868f`, cited, not copied. Field and judge: ours.

### 6.2 What survives

`state.rs` (poses, SoA, `compose_proposal`, `ExactIncumbent`), `contact.rs`, `decomposition.rs`, `broad_phase.rs`, energy measure/fold/incremental rebuild/census, `publish.rs` (band 4 µm, repair ≤ 4n rows and ≤ 16 µm/piece, Exclusive \(r=2.500\), allowance 0, untouched contract), `corpus.rs` (affine compression remains a **corpus factory**, not the live start), `diagnostics.rs`, S0, 1 k/10 k corpora, default-build isolation, `jagua-rs` hygiene, four pinned engine gates, constructor as anytime floor (`from_constructor` fingerprint is never a child). Live trig stays `f64::sin_cos`.

### 6.3 Routine move

One relocate of piece \(i\) with incident raw Φ > 0:

1. Sample pool includes the current pose.
2. 25 focused: translation uniform in the piece’s current AABB; θ from 16 equally spaced angles if `allow_rotation[i]`, else frozen.
3. 50 container-wide: translation uniform in the usable strip at current \(W\) (centroid AABB that keeps the rotated source bbox inside the sheet: physical L/R/B, \(W -\) depth-top inset on top); same 16-angle draw.
4. Keep 3 unique (\(0.05 \times \min\dim\), 1°).
5. Two-stage axis CD on incident **weighted** Φ; ratios cited from their `consts.rs` (pre-refine 0.25→0.02 × min-dim, 5°→1°; final 0.01→0.001 × min-dim, 0.5°→0.05°); wiggle only if `allow_rotation[i]`; accept-equal; rotation about the transformed centroid via `compose_proposal`.
6. Commit the best. Lexicographic: incident Φ = 0 beats any positive; else min incident weighted Φ. Delete `descent.rs` `after < before` as this move’s gate. No `ladder_top` cap.

Sweep: colliding set, Gauss–Seidel, permutation `counter_hash(seed, bite, iteration, worker_ordinal)`. Eight workers, Algorithm 10 tournament as in M6. Then GLS.

Disruption (explore failed-separate only): two pieces in the top 75 % of total convex-hull area, distinct enough in area or diameter (their 1 % test, cited); map θ onto each allowed set; followers = pieces whose **centroid** lies in the swapped ring (we do not have their POI; cite the difference); cap followers at \(n\).

### 6.4 GLS

Replace `energy::guided_update`. All pair and boundary rows, every sweep, on our \(v\):

- \(v = 0\): \(w \leftarrow \max(1,\, 0.95 w)\)
- \(v > 0\): \(w \leftarrow w\,(1.2 + 0.8\, v/v_{\max})\)
- cap \(2^{20}\)

`guided = w v^2`. Persist across rollback inside a width. Reset on a successful width change and when starting a different bite from the exact parent. No stall-only integer increment. No pole proxy.

### 6.5 Regime

Start at constructor legal raw depth \(D^*\) (mixed-61: 182.976). No C175, no affine live-start, no \(T_0 = D^* - 0.10(D^*-L)\).

Explore bite: \(W \leftarrow W(1-0.001)\), centre cut, translate every piece whose transformed source centroid is above the cut by \(\Delta = T-D\) along the long axis; \(\theta\) and mirror frozen. Separate through infeasible. Success = Exclusive-valid, contract-valid publication with raw depth \(\le T\). Then install `Publication.poses`, rebuild geometry and all rows, set \(D\) to `Publication.raw_source_depth_mm`, keep \(\beta = 0.001\), next bite.

Explore fail: §5 persist + disrupt. Never enlarge \(W\).

Compress: always from the installed exact parent; TimeBased step \((0.0005, 0.00001)\) against phase-elapsed/phase-limit, read between bites; seed-derived cut; fail = discard child, parent unchanged.

Strike caps: explore 200 iterations without 2 % raw-Φ improvement vs strike-best → strike, 3 strikes → stop; compress 100 / 5. Rollback to min-raw snapshot, keep weights. `raw Φ = 0` and `max_g ≤ 4 µm` licenses a publication attempt; only dual-valid success bites.

Minimum improvement 1 µm. Repair giveback beyond the target refuses the checkpoint (`publish.rs:418`).

### 6.6 Wall and curve

Clock starts on the decoded bare request. Read between bites. Constructor once. 80/20 of remaining to explore/compress. One process, eight separator workers. 3/10/30 is one 30 s trajectory sampled as a step: last dual-valid incumbent completed *before* each stamp. No interpolation. Constructor-only at 3 s is allowed and expected.

### 6.7 Pre-committed gate (verbatim)

> **ROUND VALIDITY.** One release binary, feature `overlap-ics`, eight Algorithm-10 workers, seeds 0 through 8, one run per seed. S0 remains bit-identical at raw depth 150.16451 with Φ bits zero, Exclusive `two_r=5000`, untouched contract-valid, zero repair rows and zero giveback. Both numeric-soundness populations retain zero false-feasible, zero containment false-feasible and zero incremental mismatch. Cold-Φ, row-rebuild and cell-gap throughput thresholds remain green. The legacy proposal microbenchmark remains recorded under its original meaning; the new member additionally sustains at least 100,000 relocate-evals projected into eight seconds. All four pinned engine gates pass on default, feature-compiled-unarmed and armed builds. `fast.sh` has no stale worktree default.
>
> **PASS.** From the bare mixed-61 request, at the 10.000-second checkpoint, at least 3 of 9 distinct seeds have published a non-constructor layout with raw-source depth ≤168.484 mm. Every emitted publication at every time passes Exclusive `r=2.500` and the untouched publication contract. The complete non-interpolated 3/10/30 curve, all nine seeds, is reported. A contemporaneous interleaved AB/BA wall-arm control is reported and cannot raise or lower 168.484.
>
> **FAIL.** A valid round with fewer than 3 of 9 qualifying seeds fails the funded `CutCloseRelocate + 0.1% cut-close` member. Proxy depth, best seed, median alone, constructor depth, or a publication completed after 10.000 s cannot change that verdict.
>
> **FORBIDDEN RESCUES.** No Sparrow fixture as a seed or warm start; no Sparrow or jagua code linked into the engine; no pole proxy, polygon simplification, or Xoshiro; no `general_relaxed`, portfolio, m34, crot, contact-block, old jump, Union kernel, allowance, 2.502 radius, enlarged repair band, alternate feature arm, seed substitution, wall interpolation, post-result bite change, \(\beta\) fitted to mixed-61, 8 workers re-read as a later rescue, joint-PGS or component-Y retrofit, or rerun selected by outcome.
>
> **FAILURE LICENSE.** A failing valid round licenses one read-only funnel autopsy: `bitesStarted → proxyBandReached → exactAttempted → dualValidPublished → ≤168.484`. It licenses one rerun only for a line-level violation of this frozen specification accompanied by a red/green minimal vector. Otherwise the member is closed; joint projection, component-Y, a different sampler, and a different homotopy are separately funded proposals.

### 6.8 Determinism and counters

Same request, seed, binary, toolchain, features, worker count, work cap → bit-identical epoch targets, cut IDs, poses, weights, publications, document. No clock in a bite, sweep, relocate, or CD. Wall publications record their fixed-work ordinal.

Mandatory counters: constructor wall/work/depth; bite start depth, target, \(\beta\), cut; relocate-evals, focused/container samples and **commits**, CD evals; GLS updates; disrupt counts; PGS-sweep analogue (relocate sweeps); proxy-band arrivals; exact attempts and refusal reason; target-to-published giveback; exact-parent fingerprint before and after every success; successful/failed bites and work per bite; incumbent at actual 3/10/30; `constructor_wall_s` / `search_wall_s`.

### 6.9 Commit order

1. spec + Sparrow ledger `14f4868f` + timer finding + `fast.sh` default;
2. relocate + uniqueness / accept-equal / container-commit-beyond-`ladder_top` vectors;
3. GLS all-rows;
4. disrupt + follower cap;
5. cut-close + Φ=0-does-not-bite + forced-repair two-bite parent install;
6. eight-worker barrier/merge;
7. explore/compress loop, publication-gated;
8. FAST tier including first-bite canary;
9. frozen 3/10/30 gate run;
10. HEAVY only after the result is frozen.

### 6.10 Remaining refusal

I sign this spec. I do not sign Sol round-1 §2 as written. The amendment is the operator, not the gate. If the owner arbitrates for `JointSmallBiteSeparator` despite “family’s proven regime,” that is an owner call; it is not a consultant consensus.
