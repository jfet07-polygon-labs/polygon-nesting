# Grok review 13 — the strike predicate (2026-08-23)

Implementation review of CutCloseRelocate round 1 (merged 2aaf681), mandated
by the owner. Verdict: (A) line-level defect — the inner strike-reset
predicate accepts any raw-Phi minimum where the frozen spec (and Sparrow's
separator.rs:106-108) requires 2%; Algorithm 12 starved on every stuck
bite 22. One repair + one unchanged rerun licensed. Verbatim.

---

### Exact control-flow (explore-fail)

```text
explore_bite: W ← W(1−0.001), centre cut          // homotopy.rs:176
reset weights, last_attempt_pose_digest = None     // mod.rs:916-917
pool = []
attempt = 0
loop:
    separate(Explore, 200/3)                       // strikes LOCAL = 0
    accumulate iterations / strikes / exact_attempts
    if Published → commit, next bite
    persist at W; push_pool(best-raw snapshot)
    attempts += 1
    if stop == Deadline OR phase_done OR attempts_exhausted → break  // NO disrupt
    attempt += 1
    restore Normal(0,0.25)-biased pool entry (that entry's own weights)
    last_attempt_pose_digest = None
    disrupt(...)                                   // even if !fired, loop continues
    → separate AGAIN at the same W
if unpublished → end explore, compress from last dual-valid parent
```

Citations: [`mod.rs:904-1024`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs), inner loop [`mod.rs:936-997`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs).

That matches Sparrow `explore.rs`: failed `separate` → pool → Normal(0,0.25) restore → `disrupt_solution` → `separate` again at the same width; `max_conseq_failed_attempts = None` on the fixture.

**A “bite”** is one 0.1 % cut-close at a new `W` (`BiteRecord.ordinal`).  
**A “separation attempt”** is one `Engine::separate` call (`BiteRecord.attempts` counts *failed* ones).  
**A “strike”** is a 200-iteration no-new-min-raw plateau *inside* one `separate`.

---

### Whether strikes reset

**Yes, every `separate` call.** They are not stored on `Engine`, `State`, or a bite object.

```756:756:crates/polygon-nesting-core/src/search/overlap_ics/mod.rs
        let mut strikes = 0u32;
```

`Engine` persistent fields that could leak across disrupt: only `last_attempt_pose_digest`, and that is cleared after pool restore (`mod.rs:975`) and at each new width (`mod.rs:917`). No strike field on `Engine` (`mod.rs:173-185`).

Per-separation ladder (`mod.rs:796-809`):

- 200 iterations without beating **this separation’s global `min_raw`** → strike.
- If `min_raw < 0.98 * strike_entry_raw` → `strikes = 0` (improving reset).
- Else `strikes += 1`; at `strikes >= 3` → `SeparateStop::Struck`.
- Rollback keeps weights (`restore_keeping_weights`, `mod.rs:1169`).

`BiteRecord.strikes` is a **report sum**, not a live cap:

```948:948:crates/polygon-nesting-core/src/search/overlap_ics/mod.rs
                record.strikes += separation.strikes;
```

New `BiteRecord { strikes: 0, ... }` per bite (`mod.rs:919-931`). After `Struck`, the next `separate(...)` allocates a new `strikes = 0`.

---

### Caps / guards that could fake a single-shot

| analogue | wall behaviour |
|---|---|
| `max_conseq_failed_attempts` | **None.** `Pacer::attempts_exhausted` is `false` in `Budget::Wall` (`mod.rs:1643-1648`). Fixed-work only. |
| “already disrupted this width” | **absent** |
| single-shot flag | **absent** |
| pool cap 64 | memory guard (`POOL_CAPACITY`, `mod.rs:1281`); not a schedule stop |
| deadline skip of disrupt | **yes, deliberate:** `stop == Deadline` breaks *before* pool-restore/disrupt (`mod.rs:964-968`) |

Wall explore will keep Struck → pool → disrupt → separate until the 80 % explore clock fires. That is Sparrow’s `term.kill()` in both `separate` and the explore loop, not a leak.

Fixed-work `attempts_per_bite: 1` *does* skip disrupt (FAST fixtures). That is not the wall arm.

---

### Metric: what wall `"strikes"` counts

It is **`sum` of in-separation strike increments at each `separate` return**, **not strike-outs**.

- `Struck` returns with `strikes == 3` (the cap).
- `Deadline` can return with `strikes ∈ {0,1,2}`.
- Seed 8 10 s: **1 strike, 0 disruptions** ⇒ one `separate`, one counted 200-iter plateau, **no strike-out**.
- Seed 8 30 s: **4 strikes, 1 disruption** ⇒ first `separate` struck out (3) → disrupt → second `separate` added 1 more then died on the clock. If the counter had leaked at 3, the second call would strike out again after one more 200-iter stall and you would expect **≥2 disruptions**, not 1.

---

### Why seed 8 / 10 s is 922 iters, 1 strike, 0 disruptions, ~7 s burned

Constructor ~2.32 s; search ~7.69 s; explore = 80 % of remaining ≈ 6.14 s (`README.md` §12). Bites 1–21 publish in ~0.3 s. Bite 22 (`W = 178.99252`) then occupies **the rest of explore inside a single `separate`**.

922 ≫ 3×200, so strike-out was *possible*. It did not happen because `since_improvement` resets on any new global `min_raw` (`mod.rs:768-774`), and the 2 % reset zeroes the strike count (`mod.rs:800-801`). 53 exact attempts at `min raw Φ = 6.35e-5` means the band is hit often and Φ is still ticking; almost every 200-window is an *improving* window. One plateau counted (`strikes = 1`); then the barrier read `elapsed >= explore_deadline` → `SeparateStop::Deadline` → outer loop **breaks without disrupt** (`mod.rs:964-968`).

Caveat 8 is exactly this: disrupt only runs when a separation *ends* with time left; at 10 s most bite-22 separations end on the deadline (`README.md:605-609`). Across nine 10 s cells**Verdict: NOT disrupt still fired A DEFECT**

This 25 times ( is notother an inset/` seeds’W` mismatch of strike-outs), the clearance-split so family. Strip the operator-top residual is not and `proxy_ dead —depth` share it the is rare same sag at that-less reference budget.

30 s. The 53 is the existence invisible proof of attempts re-entry: are the 4 3825 master µm band vs a iterations strict `proxy, 4 strikes, **_depth > T` inequality1 disruption**, still unpublished.

---.

Then

## Mechanism explore

 ends### 1. (`None` branch Attempt gate and `mod.rs: why checkpoints vanish1020-102

`separate3`) and compress` starts from the last counts dual an-valid parent ( exact attempt asbite 21’ soon as `maxs published `_g ≤ bandD`),` which (4 µm). is why seed 8 ` still shows compress bitespublish::attempt` 23–24 then b.

---

###ails ** Is this a silentbefore** `work filter on disrupt.exact_checkpoints += 1`?

**Not and ** a leakedbefore** an- `ExactCheckpoint`strike filter is built:

.** Re```264:274-entry is implemented:crates/polygon and-nesting- observedcore/src/ atsearch/overlap_ 30 s.

ics/publish.rs**It
    let is a trigger proxy_depth =/ super::state::budgetraw_source_ filter:** disruptdepth_mm(& is gated on “state.geometry, contract);
    iffailed !(max_violation `separate` *and* not deadline_mm <= limits.”.band_mm A) {
        return slowly improving separator None;
    }
 can    if proxy_ burndepth > state.target_depth_mm the whole explore remainder without {
        return None ever returning;
    }
    `Struck if proxy_depth`/` > incumbent_depthRefused`, so_mm - limits Algorithm.minimum_improvement 12’_smm fail operator {
        never runs. return None;
    That is the same }
    work. triggerexact_checkpoints += Sparrow uses 1;
``` (disrupt

```776: on failed793 separation:crates/polygon, not on a-nesting- stalled sweep — grok-review-core/src/search/overlap_ics/mod.rs12 §1.
            if totals7). It is **not** the.max_violation_ “silentmm <= band {
 filter on the new                band_reached operator” family of = true;
                a leftover exact_attempts += 1;
                let outcome = self.attempt_publication cap eating the retry.

`disrupt`();
                if itself does let Some(publication not fail) = outcome. closedpublication {
                    return on mixed-61 SeparateOutcome {
                       :

 published: Some(- `count <publication),
                        2` → // idle (`disrupt ...
                    };
.rs:258                }
                if-260`).
 totals.raw <=- Empty 0.0 large-set → {
                    break Separate firstStop::Refused piece;
                }
            drawn }
```

` from *proxyall* pieces;_depth` is second from any **-distinct fallback (`not** kerneldisrupt.rs:261/`-286GridSet` depth`).
. It is max- Zero transformed followers still `fired source: true`-ring ` (swap happenedy` plus **).
- `sheet edge!fired` still only loops** (no sag back into):

```491 `separate`:499:crates (`mod.rs:/polygon-nest994-997ing-core/`).

Homsrc/search/otopy neveroverlap_ics/ grows `W` andstate.rs
pub fn raw_source never restores the parent_depth_mm to skip a width(geometry: & (`homotopy.rsGeometry, contract:` only &Contract) -> shrink f64 {
   s; persist let mut deepest = is the f64::NEG explore_INFINITY;
    loop). Compress for point in & always reinstallgeometry.ring_s `points {
        deepestparent_poses` = deepest.max( (`mod.rs:point[1]);103
    }
   5`). deepest + contract.

---

###sheet_edge_ Tests

-clearance_mm Caps
}
```

 only`target_depth: `the__mm` isstrike_caps_ the biteare_the_’spublished_two_ locked `W`hundred_three_ (and_one_set tohundred_ `fivewidth`_ (`tests.rs:after_mm`2647`). immediately after the
- Persist cut;-at-W see below). On on a shrink Φ=0 refuse: `a_, `T < incumbentrefused_publication`,_never_adv so the incumbentances_the_−width` —1 µm gate `attempts == is not the one 3` at `attempts firing_per_bite. The 53 events: 3` die (`tests.rs: on `proxy_2365-depth > T`.2447`)

--- ⇒

### 2 the inner. Strip loop *does*-top row uses re-enter; the same ` it does **notT` and the** assert `disrupt same inset

```ions`.
-46:59: Disruptcrates/polygon- unit tests cover swapnesting-core/src/search/followers, not/overlap_ics the schedule/broad_phase cycle.rs
pub (` fn boundary_residualstests.rs:166(
    box_1+`).
mm: [f- No vector64; 4 that `],
    contractStruck → pool → disrupt: &Contract,
 → separate    depth_target with strikes==_mm: f0`.

---64,
) ->

** [f64;Confidence: high** 4] {
 on NO    let physical = LEAK / contract.physical_ resetedge_clearance / 10_mm();
 s deadline    let strip_ storytop = depth_ (controltarget_mm - flow + contract.depth_ 30 s 4-strike/top_inset_mm();
   1-disrupt let split sheet_top = contract.sheet). **_long_axisMedium-high** on_mm - physical the improving;
    [
       -reset explanation of (physical - box “_mm[0922]).max(0 iters /.0),
 1 strike” (inferred        (box_mm[2] from the ladder - (contract. + Φsheet_short_/axis_mm -exact physical)).max(-attempt evidence0.0),; per
        (physical-iteration strike - box_mm trace[1]).max is(0.0 not in `),
        (wall.json`).box_mm[3] - strip_top.min(sheet_top)).max(0.0),
    ]
}
```

```169:184:crates/polygon-nesting-core/src/search/overlap_ics/state.rs
    pub fn physical_edge_clearance_mm(&self) -> f64 {
        self.sheet_edge_clearance_mm + self.flattening_sag_tolerance_mm
    }

    pub fn depth_top_inset_mm(&self) -> f64 {
        self.sheet_edge_clearance_mm
    }
```

L/R/B (and physical sheet top) charge `edge + sag`. The **locked strip top** charges sag-less `T − sheet_edge_clearance_mm`, which is the same convention as `proxy_depth`.

`measure_edges` copies those residuals into Φ / `max_g`; `fold` takes the max over pair rows and all four sides. No extra epsilon, no extra inset.

The unit test `the_strip_top_is_sag_less_while_the_sheet_edges_are_not` (`tests.rs:767–809`) locks this: a box with `max_y = T − depth_top_inset` has top residual **0**; `+1 µm` is a **1 µm** top residual.

Repair slack uses the same top (`publish.rs:627–629`). Relocate’s sample box does too (`relocate.rs:480–482`).

---

### 3. Φ / `max_g` vs proxy depth — same geometric reference

| Quantity | Geometry | Formula |
|---|---|---|
| Top residual / `max_g` | AABB of **source outer ring** (`piece_bounds[3]` from `ring_points`) | `max(0, Y_max − (T − edge))` |
| `proxy_depth` | **same** `ring_points` | `Y_max + edge` |

`piece_bounds` is `bounds(ring_points)` (`state.rs:442`, `contact.rs:68–77`). The ring is the untouched outer source vertices (`decomposition.rs:38–41, 71–78`). Centroids are **not** in either measurement (`split_and_close` uses centroids only to pick the far side).

Algebra, with `T ≪ sheet_long_axis` so the strip binds:

```text
top_residual = Y_max − (T − edge) = (Y_max + edge) − T = proxy_depth − T
max_g ≥ top_residual
```

So:

```text
max_g ≤ 4 µm  AND  proxy_depth > T
    ⇒  0 < proxy_depth − T ≤ 4 µm
```

The top row **is active**. The state is not “band-clean with material above `T` by a different convention.” It is **inside the 4 µm band with a few micrometres of top residual**, which is exactly the depth overshoot.

The counterfactual “material above `T` ⇒ `max_g > 4 µm`” is the error: an active top row implies `max_g ≥ overshoot`, not `max_g > band`. Overshoot in `(0, 4 µm]` licenses the attempt and fails the strict depth gate.

That is the same reading as `docs/experiments/overlap-ics/cutclose-round1/README.md:349–352`.

---

### 4. After `split_and_close`, `T` is the new `W`; energy is rebuilt against it

```909:913:crates/polygon-nesting-core/src/search/overlap_ics/mod.rs
            let bite =
                homotopy::explore_bite(&self.sources, &mut self.state.poses, width_mm);
            width_mm = bite.width_after_mm;
            self.state.target_depth_mm = width_mm;
            self.refresh_all();
```

`refresh_all` re-transforms every piece and `rebuild_all`s every row against `state.target_depth_mm`. There is no stale-`T` window.

Explore cut: `W ← 0.999 W`, `delta = −0.001 W`, split at `W/2` (depth convention, not physical edge).

```142:172:crates/polygon-nesting-core/src/search/overlap_ics/homotopy.rs
/// **Split-and-close.** Every piece whose transformed source centroid lies
/// strictly above `split_y_mm` gets `ty_mm += delta_mm`. Nothing else changes.
pub fn split_and_close(...) -> usize {
    ...
        if transformed_centroid(source, *pose)[1] > split_y_mm {
            pose.ty_mm += delta_mm;
```

Near-side pieces (centroid `≤ W/2`) do **not** move. A near-side vertex that was flush with the old strip sits `0.001 W` above the new strip (~0.179 mm at `W ≈ 179`). That is **~45× the band**, so it cannot hide: top residual ≈ 179 µm until the separator pushes it down.

Far-side pieces that were flush stay flush: `Y' − (W_new − edge) = 0`.

Successful publish then sets `T` to **published** raw depth, not the bite target (`install_publication` / `install_poses` at `mod.rs:456–478`). Next bite shrinks from that `D`.

---

### 5. Bite 22 evidence is consistent with this, not with a silent too-deep zero-Φ state

README: bite 22, `W = 178.99252`, 53 exact attempts, **min raw Φ = 6.35e-5**, 1 strike, never publishes.

Φ never hit 0. If residual identity were broken (Φ/`max_g` at 0 while `proxy_depth > T`), the `totals.raw <= 0` branch would `Refused` on a fully separated too-deep layout. That is not what happened. The 53 hits are `max_g ≤ 4 µm` with **positive** Φ, i.e. a few micrometres still on the rows (top and/or pairs), depth still strictly above `T`.

---

## Clearance-split family?

**No.** That family was “charge `edge+sag` on the strip top while `proxy_depth` is sag-less,” which **inflates** top residual by one sag (phantom too-strict top). The opposite bug — sag-less Φ vs sag-full proxy — would allow `max_g = 0` with `proxy_depth` a sag above `T`.

Shipped split:

- strip top / proxy / publication gate: `depth_top_inset = sheet_edge_clearance` (sag-less)
- L/R/B / physical sheet top: `physical_edge = edge + sag`

They match on the quantity that decides “too deep vs band.” AABB vs ring vs source vertices are the same outer ring. Centroids are only the cut predicate.

---

## Why 53 events can exist (mechanical)

1. Separator drives every row, **including top**, to `v ≤ 4 µm`.
2. That is the publication **band**, not dual-validity and not `proxy_depth ≤ T`.
3. `proxy_depth − T = v_top ∈ (0, 4 µm]` ⇒ band passes, `if proxy_depth > T { return None }` fires.
4. `exactAttempted` increments in `separate`; `exactCheckpoints` does not.

No red/green vector: the early return is the specified “layout must already sit in the locked strip” gate, not a missed inset.

**Confidence: high** — identity is in the formulas, the sag-split test, and the bite-22 Φ minimum never reaching 0.## Named binder

**Deadline-killed first separation on a constructor-topology jam at `W ≈ 178.99 mm`. Disrupt does not fire at 10 s because the explore clock dies inside the first `separate()`, before pool restore.**

That is **(a) as the basin**, with a precise qualifier that is **not** “disrupt fires but doesn’t help”:

- 21 × 0.1 % centre-cut bites from constructor `D* = 182.976` land at `W ≈ 179.17` in ~0.3 s.
- Bite 22 is the first 0.1 % cut that does not separate (`W_after ≈ 178.99`).
- At 10 s, **every stuck seed has `disruptions: 0` on bite 22**. The fail path in `run_cutclose` only restore+disrupts if the failed separation did **not** stop on `Deadline` / phase-done (`mod.rs` 956–996).

**(c) is a secondary mechanical filter, not Φ = 0 dual-valid refusal.** Stuck bite-22 traces never have `minRawPhi == 0`. Several reach the 4 µm band (`proxyBandReached: true`) and call `publish::attempt`, which returns `None` **before a checkpoint** because `proxy_depth > target` (`publish.rs` 268–270). That is “in-band but still proud of the strip,” the §8 story.

**(b) is refuted.** GLS weights reset on every width change, persist inside a separation, and on pool restore reload the **pooled entry’s** weights.

**(d) is an amplifier, not the shelf.** Eight workers clone one master state per iteration; wall cells did not record winner ordinals (`recordFingerprints: false`). Seed-to-seed diversity *does* matter: seeds 3 and 6 rearrange before bite 22 and cross it in ~1.3 s.

No constructor y-row table exists in-tree. The “179 mm shelf” is **behavioral**, not a measured row height. The 179.07–179.08 incumbents are **compress bite 23** from the bite-21 parent (`179.17 × (1 − 0.0005) ≈ 179.082`), not the failed explore target.

**Confidence: high (~0.9)** on the 10 s mechanism (cell JSON + `run_cutclose` control flow). **Medium** on “row boundary / piece must change rows” as geometry — that is an inference from jam + centre-cut homotopy, not a fixture measurement.

---

## Per-seed bite 22

`wall.json` only has cell summaries. Per-bite fields are in the cell documents `wall.py` wrote to `/var/lib/t3/tmp/overlapics/round1/wall-{3,10,30}s-seed{N}.json`. There is **no `poolRestores` field**; `attempts` is failed-separation / pool-push count. A restore happens only if the loop continues past the deadline check.

### 10 s (gate)

| seed | `widthAfterMm` | pub | `masterIterations` | `strikes` | `disruptions` | `attempts` (pool pushes) | restores (inferred) | `exactAttempts` | `minRawPhi` | `proxyBandReached` | `movedPieces` |
|---:|---:|:--:|---:|---:|---:|---:|---:|---:|---:|:--:|---:|
| 0 | 178.9865000671206 | no | 1290 | 1 | 0 | 1 | **0** | 0 | 1.215e-4 | no | 34 |
| 1 | 178.99140291848428 | no | 797 | 0 | 0 | 1 | **0** | 1 | 1.064e-4 | yes | 35 |
| 2 | 178.98998698110853 | no | 1754 | 0 | 0 | 1 | **0** | 1411 | 3.343e-5 | yes | 35 |
| **3** | 178.96681192483413 | **yes** | **137** | 0 | 0 | 0 | 0 | 3 | **0.0** | yes | 36 |
| 4 | 178.9916412933487 | no | 1072 | 1 | 0 | 1 | **0** | 3 | 6.989e-5 | yes | 35 |
| 5 | 178.98211777198213 | no | 855 | 0 | 0 | 1 | **0** | 0 | 1.599e-4 | no | 37 |
| **6** | 178.96468929578103 | **yes** | **131** | 0 | 0 | 0 | 0 | 1 | **0.0** | yes | 35 |
| 7 | 178.9925150066246 | no | 892 | 0 | 0 | 1 | **0** | 1 | 8.094e-5 | yes | 34 |
| 8 | 178.9925150066246 | no | **922** | **1** | **0** | 1 | **0** | **53** | **6.350e-5** | yes | 35 |

Seed 3 / 6 publications (`wall-10s-seed{3,6}.json`):

- seed 3: `publishedRawDepthMm` 178.9666481914237, `wallSeconds` **1.305**, 137 iterations, 15 433 proposals.
- seed 6: `publishedRawDepthMm` 178.955449846644, `wallSeconds` **1.310**, 131 iterations, 13 908 proposals.

Cell-level `wall.json` 10 s disruptions (25 across nine seeds) are **earlier** explore bites, not bite 22. Examples: seed 5 bite **9** (`disruptions: 1`, then published); seed 2 bite **15** (`disruptions: 1`, then published). Seed 8 10 s: `work.disruptions: 0` for the whole cell.

### Seed 8, 10 s vs 30 s (the cliff)

10 s, `wall-10s-seed8.json` bites 1–24:

- bites 1–21: 1–14 master iterations, all publish; bite 21 `widthAfterMm` 179.1716866933179; search after constructor is 7.688 s of which explore is `loopExploreSeconds` 6.147.
- **bite 22:** `widthAfterMm` 178.9925150066246, 922 iters, 1 strike, 53 exact attempts, `minRawPhi` 6.350e-5, `published: false`, **`disruptions: 0`**.
- bite 23 compress from the **bite-21 parent** (not the failed 178.99 state): `widthAfterMm` 179.08210534663405, 3 iters, **publishes**. This is why the 10/30 s incumbent is 179.08211, not 178.99.
- bite 24 compress toward 178.993, 219 iters, never publishes.

30 s, `wall-30s-seed8.json` bite 22: **3825** iters, **4** strikes, **1** disruption, `attempts: 2`, 233 exact attempts, `minRawPhi` 6.164e-5, still unpublished. Same `widthAfterMm` 178.9925150066246. Compress bite 23 again publishes at 179.08210214351493.

That matches README §7 and `wall.json` seed 8 10/30: `exploreBites: 21`, `bestStrictChildMm` 179.082105…, `finalWidthMm` 178.993….

### 30 s bite 22 (who clears)

| seed | pub | iters | strikes | disrupt | attempts | exactAtt | minRawPhi | band |
|---:|:--:|---:|---:|---:|---:|---:|---:|:--:|
| 0 | yes | 2061 | 2 | 0 | 0 | 1 | 0.0 | yes |
| 1 | no | 5319 | 0 | 0 | 1 | 44 | 3.696e-5 | yes |
| 2 | yes | 7450 | 3 | **1** | 1 | 7074 | 0.0 | yes |
| 3 | yes | 137 | 0 | 0 | 0 | 3 | 0.0 | yes |
| 4 | yes | 3622 | 6 | **2** | 2 | 2103 | 0.0 | yes |
| 5 | yes | 1700 | 2 | 0 | 0 | 1 | 1.581e-5 | yes |
| 6 | yes | 131 | 0 | 0 | 0 | 1 | 0.0 | yes |
| 7 | no | 3906 | 0 | 0 | 1 | 1 | 6.164e-5 | yes |
| 8 | no | 3825 | 4 | **1** | 2 | 233 | 6.164e-5 | yes |

Clearing bite 22 is expensive except for seeds 3/6 (~130 iters). Seed 5 at 10 s is stuck (`exactAttempts: 0`, never in band); at 30 s it publishes after 1700 iters **without** disrupt. Seeds 1/7/8 never leave the 179.08 compress plateau even at 30 s (`wall.json` 30 s `exploreBites: 21` for those three).

---

## When disrupt fires relative to bite 22

Explore loop (`mod.rs` 907–1024):

1. `explore_bite` → `W ← 0.999 W`, centre cut, far-side `t_y` only (`homotopy.rs` 176–188).
2. `reset_weights` (new landscape).
3. `separate()` at **that same `W`** until publish, strike-out, or deadline.
4. On failure: `push_pool`, then **if** `stop == Deadline` **or** explore phase done → **`break` — no disrupt** (964–968).
5. Else: `attempt += 1`, restore pooled poses **and that entry’s weights**, `disrupt()`, loop `separate()` again **at the same `W`** (never grow, never skip).

So disrupt is **after strike-out (or other non-deadline fail) of a separation**, and the next separation is at the **same** bite-22 width.

At 10 s bite 22, every stuck seed matches step 4: one failed `separate()`, then deadline. Pool depth **1** sitting unused; **0 restores**.

At 30 s seed 8: first sep strike-cap (`strikes` 3 of `SeparateLimits::EXPLORE.strikes = 3`), restore from pool depth 1, **one** disrupt (`work.disruptions += 1` in `disrupt.rs` 346), second sep then deadline (`strikes` total 4). Still unpublished; `minRawPhi` stayed ~6.16e-5.

At 30 s seed 4, two strike-outs (`strikes: 6`) and **two** disruptions, then a third separation publishes (`attempts: 2` counts only failures). That is the rare “disrupt fires and the retry eventually publishes” path — **not** the 10 s path.

---

## GLS reset — matches spec / provenance

`energy.rs` 467–475: **`reset_weights` is called on a successful width change, and nowhere else.**

Call sites in `mod.rs`:

- 901: trajectory start.
- 916: every explore bite after the cut (explicitly “`change_strip_width` rebuilds their tracker”).
- 1014: after a dual-valid publication (new `D` / `W`).
- 1036: every compress bite, after reinstalling the exact parent.

Inside a separation, rollback is `restore_keeping_weights` (`mod.rs` 797, 845, 1164–1168) — Sparrow `restore_but_keep_weights`.

Pool restore is **not** a floor reset and **not** “keep current tracker weights”: `PoolEntry::restore_weights` (`mod.rs` 1528–1550) reloads the pooled layout’s own weights (Sol review 17 R2 §5; provenance table “explore failure pool”).

Bite 22’s first (usually only) separation therefore **starts at weight floor**, then GLS runs every master iteration (`gls_update` in `tournament`, `mod.rs` 704–706). Hundreds to thousands of weight updates at the jammed width are available. Reset is not the cliff.

---

## Φ = 0 refused publication at bite 22

**Count among unpublished bite-22 records: 0.**

Unpublished 10 s bite 22 `minRawPhi` values are 3.3e-5 … 1.6e-4, never 0. Unpublished 30 s (seeds 1, 7, 8) likewise ~3.7e-5 … 6.2e-5.

Code path for a true Φ = 0 refuse (`mod.rs` 776–793): band hit → `attempt_publication` → if `raw <= 0` and unpublished → `SeparateStop::Refused`. None of the stuck bite-22 rows look like that.

What *does* happen when `proxyBandReached` is true: `exactAttempts` increment, `publish::attempt` bails at `proxy_depth > state.target_depth_mm` (`publish.rs` 268–270) **without writing a checkpoint**. Seed 8 10 s: 53 `exactAttempts` on bite 22 vs `work.exactCheckpoints: 27` for the **whole cell** (22 publications + a handful of null-`publishedRawDepthMm` rows from other bites, e.g. bite 19’s extra attempts). That is the §8 / README caveat, not Φ = 0.

When bite 22 **does** reach `minRawPhi: 0.0` (seeds 3, 6 at 10 s; 0, 2, 3, 4, 6 at 30 s), it **publishes**. Seed 5 30 s even publishes with `minRawPhi` 1.581e-5 > 0 — publication is `max_g ≤ 4 µm` plus `proxy_depth ≤ target`, not Φ = 0.

**(c) as “stricter than Sparrow’s proxy advance” is real in the spec** (`cutclose-relocate-spec.md` 46–50) **but it is not why bite 22 burns the budget**: the stuck states are not proxy-feasible at `W` either (they still stick out of the strip). Sparrow’s log has no 179 mm cliff because they start from a different, ~31 mm worse LBF packing and reorganize long before this constructor topology jams.

---

## Constructor / 179 mm “shelf”

No piece-diameter / row-height table at 179 mm in the fixture or engine. Measured facts:

- Constructor fingerprint `a791c397…`, `rawSourceDepthMm` 182.976, 61 pieces (`wall-10s-seed8.json`).
- Bite 1 target is exactly `0.999 × 182.976 = 182.793024` (first-bite canary).
- Seed 8’s first 21 widths are the geometric chain `D* × 0.999^k` with `movedPieces` 34 then 35 — same centre-cut partition, 1–14 iterations.
- Seed 3 **breaks that chain** at bites 17–18 (28 and 29 iterations; `movedPieces` 34 → 36; published depth steps off the `0.999` lattice: bite 18 `widthAfter` 179.71028 vs bite 19 `widthBefore` 179.69916). It arrives at bite 22 already ~0.026 mm deeper and with a different fingerprint, then publishes in 137 iterations at `wallSeconds` 1.305.
- Compress 0.05 % from the same 179.17 parent **does** publish (179.082). The jam is specifically the extra ~0.09 mm of a 0.1 % explore bite, not “nothing is legal near 179.”

That is a **constructor-packing clearance jam under 0.1 % centre-cut homotopy**, consistent with (a), but **not** a documented row-boundary measurement. Disrupt exists for large-piece topology change; at 10 s it never gets to fire on this width.

---

## Worker tournament diversity

Wall cells: `"recordFingerprints": false`, `"workers": 8` (`wall-10s-seed8.json` schedule). `contestedIterations: 0` is **unmeasured**, not “uncontested.”

Source (`mod.rs` 614–619): each master iteration **clones the identical pose/row/weight state** into 8 workers; they differ only in `counter_hash(seed, bite, iteration, worker)` permutation and sample stream. Merge is min weighted Φ, ordinal tie.

README §12 / §21 (K=8 fingerprint cell, not the wall): 9/9 iterations contested, **4 distinct winning ordinals**, two-process bit-identical. `stayPutWinners` on seed 8 10 s is 84 803 / 86 022 = **98.6 %** — container-wide samples almost never beat the incumbent pose.

**(d) is therefore structurally true** (eight views of one basin) and can explain why 922–3825 iterations of seed 8 do not find seed 3’s 137-iteration exit, but the **existence** of the 178.99 cliff is the constructor homotopy jam, and the **10 s burn** is the deadline sitting on the first separation.

---

## Candidate scorecard

| candidate | verdict at bite 22 |
|---|---|
| **(a)** constructor shelf ~179 | **Primary basin.** 21 easy 0.1 % bites then a hard width at 178.99. Not a tabulated row height. |
| disrupt at 10 s | **Does not fire** on bite 22. Binder is **deadline-before-fail-path**, not “disrupt useless.” |
| disrupt at 30 s | Fires on seeds 2, 4, 8 (pool depth 1–2). Helps 2 and 4; not 8. |
| **(b)** GLS reset | **Refuted.** Reset on width change matches Sparrow tracker rebuild; bite 22 has a full GLS lifetime. |
| **(c)** dual-valid vs proxy | **Not Φ = 0 refuse** (count **0**). Band-and-proud-of-strip early-out explains empty `exactCheckpoints` for in-band stuck seeds. |
| **(d)** 8 identical clones | **Amplifier.** No per-bite winner stats on the wall. Seed diversity (3, 6) beats worker diversity. |Raw wall documents are still on disk. I’ll pull bite 22 out of them and read Sparrow’s explore/separator strike path next.# CutCloseRelocate fidelity review (adversarial, read-only)

Spec of record: `docs/cutclose-relocate-spec.md` (Grok R2 §6 as amended by Sol R2 §4–§5). I re-read the live path in `relocate.rs`, `energy.rs` Alg.8, `disrupt.rs`, `descent.rs`, `homotopy.rs`, `mod.rs` (`tournament` / `separate` / `run_cutclose`), `publish.rs`, and the wall driver.

**Bottom line:** I did not find a line-level violation of the frozen member/regime that would grant Grok §6.7’s one-repair licence. The closest FAST-blind spots are declared differences or vacuous guards, not a neutered relocate or a live stall-GLS / PGS / jump.

---

## 1. Relocate — **PASS** (declared coordinate difference)

| sub-clause | ruling | anchors |
|---|---|---|
| Colliding pieces only (`incident raw > 0`) | **PASS** | `relocate.rs:707-714`; re-check at turn `descent.rs:399-426` |
| Current pose always in the pool | **PASS** | `relocate.rs:727-731`; commit is unconditional `831-836` |
| 25 focused + 50 container | **PASS** | defaults `relocate.rs:110-112`; loops `744-785`. Wall cannot retune them: `DescentConfig::derive` always uses `RelocateConfig::default()` (`descent.rs:85`); `descent_config` in the driver never writes sample counts (`overlap_ics_benchmark.rs:1340-1362`) |
| Focused = current AABB; container = usable strip at current `T` | **PASS** on intent; **declared deviation** vs Sparrow | Focused box is `geometry.piece_bounds` (vertex AABB) captured once (`737`), then **intersected** with `strip_sample_box(..., state.target_depth_mm, ...)` (`912-920`). Container passes `focused_box: None` (`765-777`) so the domain is the centroid-feasible strip at current `T` (`480-489`). Provenance table already names “they bound a transform translation; we bound a centroid.” Domains are distinct for mixed-61-sized pieces |
| 16 sampled orientations + CD wiggle | **PASS** | Absolute grid `k · 360/16`, not relative to current (`900-906`). Frozen pieces keep `entry_pose.theta_deg`. Wiggle axis is `±θ` about the **transformed centroid** via `compose_proposal` (`411-428`, `617-620`); `draw_axis` uses `0..6` iff `allow_rotation` else `0..4` (`547-555`) — rotation is ~⅓ of draws when enabled |
| 3 unique finalists, `0.05·min_dim`, `1°` | **PASS** | `722-726`, `327-334`. Angle uses `[0,180]` wrap (`380-389`) — this is the provenance “CD/uniqueness in degrees + wrap” claim, and it is real |
| Two-stage axis CD, 1.1 / 0.5, PRE/SND ratios | **PASS** | Coarse `0.25→0.02`, `5°→1°`; fine `0.01→0.001`, `0.5°→0.05°` (`116-129`). Axes ±x, ±y, ±(x,y), ±(x,−y), ±θ (`600-620`). Accept-equal: `cd_accepts` / `order != Greater` (`222-224`, `630-635`). Strict improvement only scales ×1.1; equal takes ×0.5 and re-draws the axis |
| Objective `Clear < Collision{loss}`; Invalid | **PASS** (declared) | `eval_cmp` `204-214`: `raw<=0` is Clear and **two Clears compare equal**; else `weighted` (`w v²` from `incident_totals`, `energy.rs:230-257`). Non-finite → `INVALID` inf (`522-524`). Out-of-strip is a boundary-row **Collision**, not a second predicate — declared |
| Commit the best; **no** `after<before`; **no** `ladder_top` cap | **PASS** | Unconditional install `831-836`. `after < before` exists only in comments. `ladder_top_mm` is derived but unread by relocate (`descent.rs:36-46`, `82-84`); only the FAST tripwire compares against it |

**FAST-blind nits (not FAILs):** `MAX_CD_STEPS = 500` is a hard ceiling (`665`) matching Sparrow’s `n_evals < 1000` **debug_assert**, not a displacement cap. Uniqueness is on pose `tx/ty`, which coincides with centroid space at fixed θ.

---

## 2. GLS Alg.8 — **PASS**

- All pair **and** edge rows, every **master** iteration: `energy.rs:384-426`.
- Active: `w *= 1.2 + 0.8*(v/v_max)` via `1.2 + (2.0-1.2)*share` (`340-342`, `398-415`).
- Inactive: `w *= 0.95`, floor `1.0` (`405`, `state.rs:293`).
- Cap `2^20 = 1_048_576` (`349`, `415`).
- **Workers do not update weights:** `worker_sweep` is Gauss–Seidel only (`descent.rs:385-395`); master does **one** `gls_update` after merge (`mod.rs:704-705`).
- Stall-only integer increment is gone: `PairRow` has `weight: f64` only (`state.rs:275-286`); `guided_update` / integer `penalty` are not on the live path.
- **`v` is our signed-gap `violation_mm`**, not Sparrow’s pole-area proxy (`energy.rs:1-8`, `364-366`). That is the signed field difference, not a second dialect.

`Descent::sweep` still GLS’s for locked-`T` `Engine::run` (one worker). Wall uses `tournament` + master GLS.

---

## 3. Disruption Alg.12 fail path — **PASS** (vacuous cap)

| sub-clause | ruling | anchors |
|---|---|---|
| Fail path only | **PASS** | Sole call: explore fail in `run_cutclose` (`mod.rs:984-993`). Not on stall (`mod.rs:529-533`, `descent.rs:468-469`) |
| Large = CH-area cumulative 75% | **PASS** | `disrupt.rs:40, 87-116` |
| Distinctness 1% **AND** (Sol) | **PASS** | `121-126`. Fallback = any other piece (`269-286`) — swap of poses, not random throws (`288-309`) |
| Interior witness followers, same rigid map | **PASS** | Witness = first positive-area ear-clip cell centroid, stored on `PieceSource` (`decomposition.rs:304-309`, `state.rs:220-224`, `disrupt.rs:128-141`). Containment **after** the swap (`311-343`). `carry` is the new→old rigid map (`184-194`) |
| Follower cap `n` | **PASS on the letter, vacuous in spirit** | `332-335`: stop when `moved.len() >= count`. There are only `n` unique pieces, so a buggy containment test **can still move the whole layout**. `followers_capped` is dead for unique pieces. Spec text was “so a bug cannot move the whole layout”; the cap does not achieve that |
| No pair | **PASS** | `n<2` → idle (`258-260`). `n≥2` always picks two distinct indices |

---

## 4. Eight workers Alg.10 — **PASS** (declared tie-key trim)

- Default `workers: 8` (`mod.rs:1306-1314`). Wall CLI defaults 8 (`overlap_ics_benchmark.rs:977-981`).
- Identical clones **including weights:** `slots.push((self.state.clone(), descent.clone(), ...))` (`mod.rs:649-653`). `IcsState` clone carries `pair_rows.weight` / `edge_rows.weight`.
- Counter streams: `RelocateKey { seed, bite, iteration, worker }` (`descent.rs:261-267`); piece + ordinal in `piece_key` / `draw_sample` (`relocate.rs:151-160, 894-899`); permutation tagged (`943-949`). Iteration is **trajectory-global** (never reset at a bite) — extra uniqueness, still a function of the spec tuple.
- Equal work, no cancel: every worker runs a full `worker_sweep`; join in ordinal order (`mod.rs:662-677`).
- Rank: min `totals.guided` (`Σ w v²`), **stable lower ordinal on ties** (`684-688`). Grok M6 asked for `(weighted, fingerprint, ordinal)`; spec of record says “stable ordinal tie”; provenance names the trim.
- Serial merge after barrier, then **one** master GLS (`699-705`).
- **No clock in sweep/relocate/CD.** `Instant` lives in `Pacer` (`mod.rs:1583-1600`) and the driver.

Work of all eight is charged on `trace.work` (`680-682`). Winner’s `Descent` (including `proposals` / census) overwrites the master — diagnostics, not search.

---

## 5. strip/ball jump, joint PGS, component-Y — **PASS (absent)**

- Jump types/handlers deleted (`descent.rs:459-466`). Vestigial CLI `--jumps/--stalls/--jumpcommit` writes unread fields (`overlap_ics_benchmark.rs:1347-1361`).
- `propose` is a relocate wrapper (`descent.rs:295-318`), not a gradient step.
- `incident_gradient` / ladder live only in `corpus::gradient_probe_step` (`corpus.rs:62-97`).
- No joint-λ / component-Y operator in this tree. Affine `compressed` is corpus/`from_constructor` only (`mod.rs:228-260`); live entry is `from_constructor_at_depth` (`205-225`).
- Publication is Exclusive `r=2.500`, allowance forced 0, not Union (`publish.rs:8-24, 146-151`).

---

## 6–10. Regime — **PASS**

| clause | ruling | anchors |
|---|---|---|
| 6. Start constructor dual-valid, no internal wall cap | **PASS** | Live factory `from_constructor_at_depth` installs constructor poses at `D*` (`mod.rs:205-225`). Driver does not cap construction (`overlap_ics_benchmark.rs:970-972, 959-966`) |
| 7. Explore `W←W·(1−0.001)`, centre cut, far-side `t_y` only | **PASS** | `homotopy.rs:47, 176-188, 158-171`. Advance parent/`D` only on dual-valid pub (`mod.rs:999-1018`). Fail: persist at `W`, pool, Normal(0,0.25) draw, disrupt (`956-997`, `homotopy.rs:230-247`). Never grow `W`; never restore-parent to skip. `Φ=0` + refused pub = `SeparateStop::Refused` (`mod.rs:791-793, 1467-1474`) |
| 8. Compress: restore last dual-valid parent, uniform-Y, TimeBased `0.0005→0.00001` | **PASS** | `mod.rs:1033-1091`; `homotopy.rs:51, 121-139, 191-212`. Failed child does not move parent; loop end reinstalls parent (`1094-1103`) |
| 9. Time 80/20 of **post-constructor remaining**; clock at barriers | **PASS** | Driver: `remaining = wall − started.elapsed()` after constructor (`overlap_ics_benchmark.rs:984-990`). `Pacer`: `explore_deadline = remaining * 0.8`, compress to `total_s` (`mod.rs:1339-1343, 1583-1614`). Deadline read **before** a tournament and **after** join (`812-827`); also at phase boundaries (`959-965`, `1618-1639`). Not inside relocate/CD |
| 10. Publication poses install atomically; next `D` = published raw depth | **PASS** | `install_publication` → `install_poses(..., publication.raw_source_depth_mm)` (`457-471`). Explore/compress success copies `parent_poses` from `publication.poses` and sets `depth_mm` to published raw (`1001-1014`, `1075-1087`). Band 4 µm, 1 µm improvement, giveback refuse remain in `publish.rs` |

Rollback **inside** a separation keeps current weights (`mod.rs:1164-1182`, `735-737`). Width change / new bite from exact parent calls `reset_weights` (`914-916`, `1014`, `1035-1036`).

---

## Provenance table vs code

| claim | code |
|---|---|
| Pool restore puts back **that entry’s** weights | **True** `mod.rs:1528-1550`, call `973-974`. Declared divergence from Sparrow `restore_but_keep_weights` on the **pool** path. Strike rollback still keeps live weights |
| Rollback inside separation keeps current weights | **True** `1169-1182` |
| Pool capacity 64 | **True** `1281`, `1191` |
| CD stored in degrees | **True** `relocate.rs:93-105, 116-126` |
| Finalist angle wrap | **True** `380-389` |
| Sampling counts hardcoded | **True** on the wall path (see §1) |

Sol R2 §5’s phrase “reset weights for the restored pool state” is ambiguous (floor vs restore snapshot weights). The implementation does **not** call `reset_weights()` on pool restore; it restores the pooled `w`. That matches Grok §6.4 persist-inside-a-width better than a floor reset, and is what the provenance table claims. I would **not** call it a spec-of-record FAIL.

---

## Ruling: `incident_gradient`

**Not on the live search path. Not a leftover of stall-only GLS.**

It is the retired **gradient-PGS / numeric-soundness probe**. Only caller: `corpus::gradient_probe_step` (`corpus.rs:76`), which still walks `ladder_top→ladder_bottom` with `after < before` on incident guided energy (`62-70`). Docs in `energy.rs:275-287` state that explicitly.

Live relocate never calls it. Stall-GLS is fully replaced by `gls_update`. Harmless floor residue; do not treat as a member defect.

Related leftovers, also not live: `highest_pressure_piece` / `sweep_order` (`energy.rs:428-457`); `Descent::pressure_piece` has no engine caller.

---

## Ruling: `relocate_eval_budget`

**Off on the wall path. Cannot silently cap CutClose work.**

- Field default in the wall cell is `u64::MAX` (`overlap_ics_benchmark.rs:1005`).
- `Engine::run` (locked-`T` S1/triangle/throughput) stops between sweeps when `sample_evaluations >= budget` (`mod.rs:514-515`). That is the re-denominated quota, not a hidden relocate filter.
- **`run_cutclose` never reads `relocate_eval_budget`.** Even a hostile CLI value cannot truncate explore/compress.

`sample_evaluations < u64::MAX` is effectively “never” on `Engine::run`. Byte-identical-off for the wall member holds.

---

## Silent issues FAST would miss (none are member FAILs)

1. **Focused AABB is a vertex box used as a centroid box**, then clipped to the centroid strip — declared, functionally still a neighbourhood vs container.
2. **Follower cap `n` is a no-op** against “don’t move the whole layout.”
3. **Pool-weight restore vs a literal Sol “reset.”** Declared; consistent with persist-inside-width.
4. **Tournament ties omit pose fingerprint** — matches frozen spec, not Grok M6’s extra key.
5. **`MAX_CD_STEPS` hard cap** vs Sparrow debug_assert.
6. **Vestigial jump CLI / census fields** still exist; unread.
7. **`PublishedBite.wall_seconds` is search-relative** (Pacer start), not request-relative. Driver separately records constructor seconds. Judging 10.000 s from `wall_seconds` alone would be wrong; the driver has both.
8. **`exact_attempts` increments on pose-digest skip** (`mod.rs:776-790`) — diagnostic inflation only.
9. Sequential disrupt blocks can carry a follower twice (source-faithful, documented `disrupt.rs:238-242`).

None of these is the pre-named neutered relocate (container commits are not filtered by `after<before` or `ladder_top`).

---

## What MATCHES (short)

25+50+stay-put; 16 absolute seeds; continuous centroid wiggle iff rotatable; 3 unique with wrap; two-stage CD 1.1/0.5 and the cited ratios; accept-equal; always commit; GLS all rows ×(1.2+0.8 v/vmax) / ×0.95 floor 1 cap 2^20 once on the master; workers cloned with weights; no worker GLS; 8-way barrier + min guided + ordinal; counter keys `(seed,bite,iteration,worker,piece,ordinal)`; Alg.12 swap+witness followers only on explore fail; 0.1% centre cut-close; TimeBased compress from installed parent; 80/20 of remaining wall; dual-valid-only parent advance; `D ←` published raw depth; Exclusive 2.500 / allowance 0; no jump / PGS / component-Y / Union on the live loop; no Xoshiro/`rand::` under `overlap_ics`.

---

## Confidence

**~0.85** on the live member+regime (I read those functions line-by-line). Lower on Sparrow-source micro-details I did not re-open in `/var/lib/t3/tmp/sparrow-bench` this pass (AND distinctness and CD-equal step scaling are taken from the signed reviews + this tree).

I would **not** grant §6.7’s line-level repair rerun on what is in this tree. The round still **fails the numeric gate** (0/9 ≤168.484); that is a closed-member result, not an implementation miss.The 10 s gate is a valid **FAIL (0/9)**. §0 was not rewritten after the numbers: commit `62570a0` froze lines 1–158, and `8ac6916` only appended below the divider.

I do **not** confirm the evidence README’s “no line-level violation.” Grok review 12 Round 2 §6.5, which the quorum spec names as the body of record, froze a 2 % test on the **200-iteration** counter. What shipped resets that counter on **any** raw decrease. That is the silent filter on disrupt. Everything else in the four flags is an extension of the main session, not a clearance-split or leaked-counter bug.

---

## The evidence shows

### Flag 1 — Caveat 4 is **not** a clearance-split defect

**Confirm the main session, with a tighter mechanism.**

Strip-top Φ and `proxy_depth` are the same sag-less convention:

- `proxy_depth = Y_max + sheet_edge_clearance` (`state.rs` `raw_source_depth_mm`)
- top residual = `Y_max − (T − depth_top_inset)` with `depth_top_inset = sheet_edge_clearance` (`broad_phase.rs` `boundary_residuals`)
- so `top_residual = proxy_depth − T` whenever the strip binds (mixed-61, `T ≪ 2700`)

`piece_bounds` is the AABB of the same `ring_points`. After the cut, `target_depth_mm` is set to the new `W` and every row is rebuilt before `separate` runs. There is no inset/`W` mismatch of the old phantom-top family.

The 53 invisible attempts (seed 8, 10 s, bite 22) are the **4 µm band vs a strict `proxy_depth > T` inequality**:

```text
max_g ≤ 4 µm  AND  proxy_depth > T
    ⇒  0 < proxy_depth − T ≤ 4 µm
```

`separate` increments `exactAttempts` as soon as `max_g ≤ 4 µm`. `publish::attempt` returns before `exactCheckpoints` when `proxy_depth > T`. The top row **is** active; it is just inside the band. Widening `T` by 4 µm would be a forbidden rescue.

That is not even the dominant bite-22 failure. At 10 s, seeds 0 and 5 have `exactAttempts = 0` on bite 22 (`minRawPhi` 1.2e-4 and 1.6e-4): they never reached the band. Seed 2 has **1411** exact attempts and still does not publish. Seed 8’s 53 is one seed’s near-miss, not a family.

Φ=0 refused publication at unpublished bite 22: **count 0**. Stuck `minRawPhi` is 3e-5–1.6e-4.

### Flag 2 — Bite 22 is a constructor-topology jam at `W ≈ 178.99`

**Confirm (a) as the basin; refute (b); (c) is not why the budget burns; (d) is an amplifier.**

From the raw cell documents (`/var/lib/t3/tmp/overlapics/round1/wall-{10,30}s-seed*.json`), not the aggregated `wall.json` funnel:

| seed | 10 s bite 22 | iters | strikes | disrupt | exactAtt | min Φ | published |
|---:|:---|---:|---:|---:|---:|---:|:---:|
| 3, 6 | W≈178.97 | 137 / 131 | 0 | 0 | 3 / 1 | **0** | **yes** (~1.31 s) |
| 0, 1, 2, 4, 5, 7, 8 | W≈178.99 | 797–1754 | 0–1 | **0** | 0–1411 | 3e-5–1.6e-4 | no |

Then they cascade: seed 3/6 go 22→30 in a handful of 1–8 iteration bites. The 10 s incumbents at 179.07–179.08 are **compress bite 23** from the bite-21 parent (`179.17 × (1−0.0005) ≈ 179.082`), not the failed explore target.

**(b) GLS:** `reset_weights` runs on every width change (Sparrow tracker rebuild). Bite 22 starts at floor and then GLS’s every master iteration. Not a special bite-22 reset.

**(c) dual-valid vs proxy:** stuck bite 22 never reaches Φ=0. When it does (seeds 3, 6; 30 s seeds 0, 2, 3, 4, 6), it publishes. Seed 5 at 30 s even publishes with `minRawPhi = 1.58e-5 > 0`. The stricter judge is not the cliff.

**(d) 8 clones of one master:** specified Algorithm 10, not a defect. `stayPutWinners` ~98 %. Seed 3 rearranges *before* bite 22 (bites 17–18, 28–29 iterations, published depth leaves the `0.999` lattice) and then crosses in 137 iterations. Worker shuffle at the jammed width does not find that exit in 922–5319 iterations.

Sparrow’s 10 s log has no 179 cliff because their LBF start is ~31 mm worse and they reorganize long before this constructor packing jams under a 0.1 % centre cut.

### Flag 3 — No leaked strike counter. The 200-iteration **predicate** is not the one I froze.

**Extend the main session.**

Control flow is the specified persist-at-`W` loop:

```text
separate()  →  if published: next bite
            →  pool
            →  if Deadline / phase_done: break   // no disrupt
            →  Normal(0,0.25) restore + disrupt
            →  separate() again at the same W, strikes = 0
```

Strikes are a local `let mut strikes = 0` inside `separate`. `BiteRecord.strikes` is a report sum. Wall `attempts_exhausted` is `false` (`max_conseq_failed_attempts = None`). Seed 8 at 30 s is the existence proof of re-entry: **3 + 1 strikes, 1 disruption** (first `separate` struck out, disrupt, second `separate` added one strike, then deadline). If the cap had leaked at 3, the second call would have struck out again and you would expect ≥2 disruptions.

What the wall `"strikes"` field counts: **in-separation plateaus**, not strike-outs. Seed 8 10 s: 1 strike, 0 disruptions ⇒ one 200-iter plateau, **no strike-out**, then `SeparateStop::Deadline`, outer loop skips disrupt.

**The FAST-invisible miss:** Grok review 12 Round 2 §6.5 (the spec of record’s body; Sol §4–§5 did not amend this sentence):

> Strike caps: explore **200 iterations without 2 % raw-Φ improvement vs strike-best → strike**, 3 strikes → stop

Sparrow `separator.rs:102-114` (rev `14f4868f`) does the same inner 0.98: `n_iter_no_improvement` resets only when `loss < min_loss * 0.98`; tiny improvements **pause** the counter; only a true non-improvement increments it.

What shipped (`mod.rs:768-774`):

```rust
if totals.raw < min_raw {
    min_raw = totals.raw;
    snapshot.clone_from(&self.state);
    since_improvement = 0;   // ANY decrease
} else {
    since_improvement += 1;
}
```

The 0.98 is applied only to the **strike count** after a plateau (`mod.rs:800-804`). The provenance table’s “identical / none” for separator strikes is false.

That is why seed 1 at 30 s can run **5319** master iterations with **0 strikes and 0 disruptions**, and seed 7 **3906 / 0 / 0**. A 1e-15 new min at iteration 199 restarts the 200. At the Φ≈6e-5 floor, that starves Algorithm 12. At 10 s, **every stuck bite 22 has `disruptions: 0`**. Disrupt *does* fire on easier widths in the same cells (seed 5 bite 9; 25 disruptions across the nine 10 s cells, none of them on bite 22).

This is not a leaked counter. It is the **wrong improvement predicate** on the counter that licenses the new operator.

### Flag 4 — The 10 s verdict is the gate. The 30 s column does not reopen it.

**Confirm the main session’s reading of §0, with one correction to the “throughput, not basin” diagnosis.**

§0.3 row 3 is the 10 s result: floor green, member as specified on the frozen *counts*, 0/9 ≤168.484. The 3/10/30 cells cannot pass or fail that gate.

The §4.5 exception’s antecedent is only half-true:

- First 0.1 % bite publishes on every seed. True.
- ≥3/9 strict children. True (9/9).
- “Tight band **above** 168.484 (constructor minus a handful of 0.1 % bites).” That describes the **7/9 at 179**, not the round. Two seeds are already at 169.00 / 169.22 at 10 s. Spread is 10 mm, not a tight band just above the bar.
- 30 s: 5/9 go **below** the bar (163.69–165.06). That is the basin existing, not a 10 s throughput miss.

The exception’s only licensed follow-up was “raise separator workers.” Eight are seated. Sol R2 §6 voided that follow-up. The 30 s column does not mint a new one.

Seeds 3 and 6 already **cleared bite 22 at 10 s** (131–137 iterations, 0 disruptions) and still finished at 169.22 / 169.00, with 11 and 12 disruptions on *later* shelves. The 10 s bar is not “if only disrupt had fired at 22.” Crossing 22 and running the specified fail path still does not buy ~105 explore bites inside 7.7 s of search.

---

## Implementation fidelity (what FAST could not see)

Member and regime otherwise match the signed freeze.

| clause | ruling |
|---|---|
| 25 focused + 50 container, stay-put in pool, 16 absolute orientations, 3 unique (0.05 min_dim, 1°, wrap), two-stage CD 1.1/0.5 with cited ratios, accept-equal, commit-best | **PASS.** No `after < before`, no `ladder_top` cap. `containerCommits == containerWinners` (1,762 at 10 s). |
| GLS Alg. 8 on all rows, master only, `1.2+0.8 v/v_max` / `0.95` floor 1, cap 2²⁰ | **PASS.** Stall-only integer increment is gone. `v` is our signed-gap mm, as documented. |
| Disrupt Alg. 12 fail path only, CH 75 %, 1 % AND, interior witness, rigid carry, follower cap `n` | **PASS.** Cap is vacuous (`moved.len() >= n` cannot bind for unique pieces) — nit, not the fail. |
| 8-worker tournament, identical clones including weights, barrier, min guided, ordinal tie, no clock inside sweep | **PASS.** Tie-key trim vs Grok M6 is declared. |
| Jump / joint PGS / component-Y | **ABSENT** on the live path. `from_constructor` affine is corpus-only; wall uses `from_constructor_at_depth`. |
| Explore 0.1 % centre cut, `t_y` only; shrink only on dual-valid; persist+pool+disrupt; never grow `W` | **PASS** as a loop. The 200-iteration predicate is the miss above. |
| Compress from installed parent, TimeBased 0.0005→0.00001, discard failed children | **PASS.** |
| 80/20 of post-constructor remaining; clock at barriers | **PASS.** Measured 80.0 %; max deadline overrun +6.6 ms. |
| Exact-parent install; next `D` = published raw depth | **PASS.** 1,269 publications, 329 with repair, 0 broken parent links. |

**Declared deviations — both accepted.**

- `incident_gradient`: only caller is `corpus::gradient_probe_step`. Not on `relocate` / `descent` / `disrupt` / `run_cutclose`. It is the numeric-soundness probe the floor still requires, not leftover stall-GLS.
- `relocate_eval_budget`: `u64::MAX` on the wall cell; `run_cutclose` never reads it. It only stops `Engine::run` (locked-`T` S1 / triangle / throughput) between sweeps. Byte-identical-off on the gate path holds.

Nits, not §0 material: focused AABB is a vertex box used as a centroid box (declared coordinate difference); funnel `exactAttempted` in `wall.json` counts **bites with ≥1 attempt**, so seed 2’s 1411 attempts collapse to 1 and README §6 “the funnel does not leak” is a bite-count funnel; `wall.py` dropped the per-bite arrays from the committed evidence (they survive under `/var/lib/t3/tmp/overlapics/round1/`).

---

## I recommend

Treat the 2 % inner predicate as the one named repair the failure license allows, and do **not** treat the 30 s column as a new 10 s experiment.

The repair is one comparison in `Engine::separate`: reset `since_improvement` only on a 2 % raw-Φ improvement vs the strike’s best (Sparrow’s inner `loss < min_loss * 0.98`; Grok §6.5’s sentence), keep 200/3 and the outer 0.98 on the strike *count*. Do not retune 200, 3, or 0.98.

I do **not** expect that rerun to pass 3/9 at 10.000 s. Seeds 3 and 6 already had disrupt on later bites and stopped at 169. The 30 s column says the member reaches 163.7–165.1 when it has ~27 s of search; a 10 s cell has ~7.7 s after the constructor. That is the member, not a remaining inset.

The 30 s fact is reportable as the honest state: first-run engine, 5/9 at or better than the old stack’s 30 s numbers, 4/9 still welded to the 179 shelf. It does not change 168.484, does not license workers, and does not license a new homotopy. If the owner wants a 10 s-focused follow-up after the fidelity rerun still fails, that is a new funded experiment.

---

## (A) Line-level defect found — §0 repair + rerun is licensed

**Name:** inner strike predicate in `Engine::separate` (`mod.rs` ~768–774). `since_improvement` resets on any `raw < min_raw`. Grok R2 §6.5 froze “200 iterations without **2 %** raw-Φ improvement vs strike-best.” Sparrow `separator.rs:106-108` is the source of that 2 %. The outer 0.98 on the strike *count* shipped; the inner 0.98 on the 200-counter did not. Provenance “identical / none” is wrong.

**Red:** seed 1, 30 s, shipped binary: bite 22, **5319** master iterations, **0** strikes, **0** disruptions, unpublished, `minRawPhi ≈ 3.7e-5`. Same shape on seed 7 (3906 / 0 / 0) and on every 10 s stuck bite 22 (`disruptions: 0`).

**Green:** after the one-line inner 0.98, the same seed-1 30 s cell records `strikes ≥ 3` and `disruptions ≥ 1` on bite 22 (the 200-counter no longer restarts on 1e-15 mins). Secondary 10 s check: stuck bite-22 rows are no longer uniformly `disruptions: 0` / `attempts: 1`.

**Not in this repair:** publication band, `proxy_depth > T`, worker count, bite size, GLS multipliers, sample counts.

The 10 s FAIL stands until that rerun is scored. The 30 s column stays diagnostic. If the green vector holds and the 10 s quorum is still 0/9, the member closes on a now-faithful FAIL.
