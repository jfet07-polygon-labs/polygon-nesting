# Grok review 9 — the overlap engine design (2026-08-22, two rounds)

Consultation for the overlap-tolerant continuous engine (the owner's directive:
Sparrow-class at 10 s without copying). Round 1 = independent design against
the six-question brief; round 2 = convergence after reading Sol's full reply.
`grok --reasoning-effort xhigh`, full-tree read access at 43f190b. Verbatim.

# ROUND 1

# Consultation: overlap-tolerant continuous engine — prototype round

Grok, independent, at HEAD `43f190b` evidence. This is a design verdict, not a rewrite of the campaign. I agree the family is the right generation bet. I disagree with several parts of the funded draft, including the round-1 “no gate”, the 175.388 success bar, constructor-as-search-init, and using the kernel as a penalty. Those would spend the month on a prettier version of work we already failed.

The one-sentence stance: **build a locked-strip then shrink member of the Imamichi/Egeblad ICS family, with PGS as the local solver and jumps as the search, judged by the integer disc kernel; do not start from the constructor’s legal layout; kill the month in days if a random throw into 168 mm cannot legalize.**

---

## 0. What the evidence actually licenses

Three facts, already measured, decide the architecture.

1. **The millimetre gap is reachability, not legality.** Gate A: Sparrow 150.16451 is contract-legal and disc-legal at \(r=2.500\); the miter join is 100% of the multi-millimetre refusal. Kernel round: exact integer disc, 8× cheaper envelope half, pair 38·39 accepted at \(r=2.500\). Matched gate: 48/48 bit-identical searches, 0/12 at equal work, 0.0000 mm. Skip-pile census: 13 867 frontiers, 0.80% released, join-tax class, **0.000 mm of depth**; 58.62% bulk overlap; the whole pile beats published by at most +0.042 mm, and those two rows are *miter-legal*. Putting a round authority on confirmation, or on the proxy, does not move 10 s.

2. **The current engine is an island hopper on a 2.5° lattice.** Seed spread 8.6 mm at equal work (`calibrated-plan` §9). 40 M→120 M buys +5.96 mm. Record 155.264 is hours, 7 204 arms, parents already 159–164. 57/61 of Sparrow’s poses are off the 2.5° lattice *and* off 1.0°. `crot` is +3.721 mm worse at 10 s. Constructor saturates 1.4 s @ ~180 mm, then 0.000 mm. Substrate 3.775 M vs Sparrow 3.742 M evals/s — raw speed is not the gap.

3. **We already “allow overlap” and it is not this family.** `general_relaxed` permits surrogate overlap around *legal* parents, on miter-offset triangulations, then confirms. The skip pile’s bulk-overlap row is clamp-lowering residue, not a designed infeasible trajectory in a shrinking strip. `global_legalize` is PGS for *micron* residues with frozen \(\theta\) and miter envelopes; its own comment says large residues “walk back out of the basin”. Sol 11 was right to refuse “more PGS/PBD” *as a new operator in that regime*. The ICS family is the opposite regime: millimetre overlaps, strip as a decision variable, continuous \(\theta\), jumps when PGS stalls.

Sparrow is a fast member of this family. The literature members we are allowed to design from, without opening `jagua-rs` or Sparrow sources: **Imamichi–Yagiura–Nagamochi 2009** (overlap-penalty NLP + ILS + shrink), **Egeblad–Nielsen–Odgaard 2007** (overlap neighborhood + GLS + shrink), **Bennell–Oliveira 2008/2009** (taxonomy: iterative compaction and separation). Ours is an ICS engine. It is not a Sparrow port, not a jagua adapter, and not mode-34 with a round envelope.

Chinese wall, stated as a build rule: `jagua-rs` stays an unused optional dependency; `search/kernel/jagua.rs` and `general_hazard.rs` are not compiled into this prototype; the Sparrow pose fixture is a **calibration oracle** (would we accept it? is \(\Phi=0\) on it?) and never a search hint.

---

## 1. Architecture of the minimal prototype

### 1.1 What “smallest honest test” means

The smallest version that can *fail the paradigm* on mixed-61, not the smallest version that produces a curve.

**Round-1 engine, in one block:**

- State = 61 continuous SE(2) poses over **source rings** (not miter envelopes, not catalog cells).
- Random overlapping throw into a **locked strip** \(W=168.484\) (the wall-ref).
- Penalty \(\Phi\) on signed clearance deficit vs \(2r=5.0\).
- PGS / separation-vector descent, continuous \(\theta\), piece-swap/jump when stuck.
- When \(\max g_{ij}\) is small, snap → kernel Exclusive at \(r=2.500\) → contract validator → local repair.
- Trivial shrink: if that publishes, \(W \leftarrow W-1\) mm and continue. The shrink schedule is not the thing under test.
- Constructor runs first, **only** as the anytime incumbent. Its poses are not the ICS state.

If that cannot publish a contract-valid ∧ kernel-exclusive-valid layout at \(W\le 168.484\) from a throw, the family is not a 10 s bet on this geometry. Shrink-from-180, L-BFGS, GJK/EPA, overlap-area, and catalog-only rotation are all ways to *avoid* that test.

### 1.2 Representation

```text
Piece i:  (x_i, y_i, θ_i) ∈ ℝ² × S¹     // f64, source-ring frame
Sheet:    short axis = 2000 mm, long axis = W (decision variable)
Clearance: r = 2.500 mm,  2r = 5.000 mm   // contract disc, not 2.502
```

Precompute per piece, once, immutable:

- source rings (already on the request);
- convex hull (`geometry/convex.rs`);
- a **deterministic ear-clip triangulation of the source ring** (≤512 cells/piece, same cap the relaxed lane already uses). Mixed-61 is 11 distinct shapes, 52 convex + 9 non-convex stars, **290 source vertices total** (kernel README). That number is the reason this prototype is cheap;
- f64 AABB in the local frame, for incremental transform.

SoA layout arrays: `x[n], y[n], theta[n], sin[n], cos[n], aabb[n]`. Piece ordinal is the iteration order. No `HashMap` in the hot path.

**Not** “continuous SE(2) over source rings *or* catalog cells”. Catalog cells are the 57/61 barrier. Sample the constructor’s angle catalog **only to draw the initial \(\theta\)**, then add \(\mathcal{U}[-5^\circ,5^\circ]\). Search is continuous \(\theta\).

Mirrors: include as a jump (flip + re-place), not as a continuous coordinate.

### 1.3 Overlap measure — this is the whole prototype

The kernel answers **exact separation \(\ge 2r\)** after a 1 µm snap. It does **not** answer penetration depth on overlapping pieces (`pair_admissible` at \(2r=1\) returns false and `critical_two_r_micron` returns `None`). Using the kernel as \(\Phi\) is a category error: you would be finite-differencing a boolean on a grid. I reject that option.

I also reject overlap *area* (Clipper, vanishing gradient near contact, Egeblad’s slower branch) and GJK/EPA (need convex decompositions and a robust EPA; overkill at 290 vertices).

**Round-1 \(\Phi\): squared clearance deficit on source rings, incremental.**

Signed gap of pair \((i,j)\), material frames:

\[
d_{ij} =
\begin{cases}
+\text{min boundary distance} & \text{if interiors disjoint}\\
-\delta_{ij} & \text{if interiors overlap}
\end{cases}
\]

Penetration \(\delta_{ij}\) for round 1:

- **convex–convex (52 pieces):** SAT minimum-translation depth. `validation/sat.rs` already implements this and has **zero production callers**. Promote it into the prototype; do not rewrite it.
- **any pair involving a star:** max SAT depth over triangle-cell pairs (underestimate of true concave MTV — see risk §6). Containment: point-in-polygon on source rings, same clause the kernel already treats separately.

Constraint residual and penalty (Imamichi, not Sparrow):

\[
g_{ij} = 2r - d_{ij},\quad
\phi_{ij} = \bigl(\max(0, g_{ij})\bigr)^{2}
\]

\[
g_{i}^{\partial} = 5.0 - \mathrm{dist}(\text{material } i,\ \partial\text{sheet}(W)),\quad
\phi_{i}^{\partial} = \bigl(\max(0, g_{i}^{\partial})\bigr)^{2}
\]

\[
\Phi = \sum_{i<j} \phi_{ij} + \sum_i \phi_{i}^{\partial}
\]

Sheet at depth \(W\) is the rectangle the contract actually measures: material must sit in \([5, 1995]\times[5, W-5]\).

Broad phase: f64 AABB grown by \(r\). On mixed-61 the integer kernel already box-certifies 95.34% of pairs at \(2r\); the continuous analogue will be in that band. Narrow phase only on the rest.

**Incremental is mandatory.** A moved piece updates 60 pair rows, not 1830. Full \(\Phi\) rescore is a debug assertion, not the loop. Sol’s production-roadmap “one moved piece should never cause an \(O(n^{2})\) score” applies here even more than it did to m22.

Throughput kill, **before the search exists** (round 0, hours):

| quantity | kill if |
|---|---|
| full \(\Phi\) (cold, 1830 pairs) | \(> 200\,\mu\mathrm{s}\) |
| incremental \(\Phi\) delta (one piece) | \(> 20\,\mu\mathrm{s}\) |
| SAT/segment pair | cannot sustain \(\ge 1\mathrm{M}\) narrow evals/s single-thread |

At 290 vertices these numbers are easy if we stay off Clipper. If round 0 needs Clipper boolean or a rebuilt offset per pose, stop; that is not a 10 s engine.

### 1.4 Move scheme

Three layers. Round 1 includes all three. Missing the third is how PGS becomes `global_legalize` again.

**Layer A — local solver (every sweep).** Projected Gauss-Seidel on the active pair set, piece-ordinal order, both endpoints move:

\[
\Delta t_i \mathrel{+}= \tfrac12 \max(0, g_{ij})\, n_{ij},\quad
\Delta t_j \mathrel{+}= -\tfrac12 \max(0, g_{ij})\, n_{ij}
\]

\(n_{ij}\) is the SAT axis or the closest-feature unit. Re-measure after each pair (true PGS, not a frozen Jacobian). After a translation sweep, one rotation sweep: for each overlapping piece,

\[
a_{\theta} = n\cdot J(p-c)
\]

(Sol review 5’s coefficient; \(J\) is 90° rotation, \(c\) the piece origin the request already uses). Step \(\Delta\theta = \mathrm{clip}(\alpha a_{\theta}, \pm 5^\circ)\), line-search \(\alpha\) on that piece’s incident \(\phi\). **No finite-difference \(\Phi\)** in round 1 (that is \(2n\) extra evals for a worse normal). Snap nothing during the sweep.

**Layer B — jumps (when layer A stalls).** Stall = \(\Phi\) drop \(< 10^{-4}\,\mathrm{mm}^{2}\) over 20 sweeps and \(\max g_{ij} > 0.05\,\mathrm{mm}\). Then, in order:

1. swap the two worst-incident pieces (exchange poses, keep \(\theta\));
2. relocate the worst piece uniformly in the free AABB of the strip (continuous \(\theta\) redraw);
3. mirror-flip the worst piece.

This is Imamichi’s ILS neighborhood, not Sparrow’s.

**Layer C — strip.** Locked \(W=168.484\) until the first legal publish; then \(W \leftarrow W-1.0\) mm. After 3 failed shrinks, \(\delta \leftarrow \delta/2\), floor 0.125 mm. After 8 failed jumps at a \(W\), **restart**: new throw at the best published \(W\) (or 168.484 if none). Do not grow \(W\) to rescue a throw.

**Not in round 1:** L-BFGS/quasi-Newton, NFP, raster grids, coordinate descent over all 183 DOF as a joint NLP, contact-block, mode-34, `crot`.

### 1.5 Module boundaries

New, feature-gated, **not** wired into `run_portfolio`:

```text
crates/polygon-nesting-core/src/search/overlap_ics/     # feature = "overlap-ics"
  mod.rs        // run_overlap_ics(pieces, settings, budget) -> PublishedIncumbent
  state.rs      // SoA poses, AABBs, active pair set, strip W
  overlap.rs    // g_ij, Φ, SAT/cells, containment, incremental delta
  pgs.rs        // layer A
  jump.rs       // layer B
  shrink.rs     // layer C
  publish.rs    // snap + kernel Exclusive r=2.500 + contract + repair
  work.rs       // integer work units, no Instant::now in the loop

examples/overlap_ics_benchmark.rs                       # 3/10/30 table, 9 seeds
```

`overlap-ics` implies `round-envelope-kernel`. It does **not** imply `jagua-experimental`. A default build is byte-identical to HEAD (four pinned gates, whole-document). The example is the only from-request driver in round 1.

---

## 2. Time-budget architecture at 10 s

### 2.1 Constructor hand-off — I disagree with starting ICS from it

The constructor is a **legal** layout at ~180 mm in 0.65–1.4 s. ICS exists because legal layouts at 180 are the island. Initializing ICS from that pose set and shrinking is mode-0 plus `global_legalize` with a bigger cap. We have the negative.

Use the constructor as the **anytime floor only**:

| t | what runs | what may publish |
|---|---|---|
| 0–0.65 s | `construct_short_side_first`, **hard-capped** at first exact-valid complete (quality-frontier: 0.535 s @ 231 mm; do not spend 1.4 s chasing 180 vs 182) | constructor, dual-gate HEAD **or** kernel Exclusive — see §2.3 |
| 0.65–10 s | ICS throws into \(W\in\{168.484, 160.0\}\) | ICS legalizations; constructor remains incumbent until beaten |

Two independent incumbents. ICS never mutates constructor poses. If ICS publishes nothing, the 10 s number is the constructor’s ~180 and the paradigm test **fails**, rather than silently reporting “we still have 175 from m34 we didn’t run”.

Do **not** run v3/`plancal`/m34 beside ICS in round 1. Mixing stacks makes every millimetre unattributable. The old stack’s numbers stay the *baseline table*, not a lane.

### 2.2 How many cycles fit

Work unit, integer, published in `work.rs`:

```text
1 unit  = 1 incremental piece-eval (AABB + incident narrow + Φ delta)
20 units = 1 PGS sweep over all currently-active pairs (charged exactly, not estimated)
500 units = 1 jump (swap or relocate, including the following PGS to stall or 50 sweeps)
```

Round-0 must confirm incremental eval \(\lesssim 20\,\mu\mathrm{s}\). Then 8 s of ICS is \(\gtrsim 4\times 10^{5}\) piece-evals single-thread, \(\gtrsim 3\times 10^{6}\) at 8 workers. A stall-to-jump cycle is tens of sweeps (hundreds of evals). **Hundreds of jump cycles and tens of shrinks fit.** If someone quotes “maybe 5 shrink-repair cycles”, they have put Clipper or a full rescore in the loop; that is a design bug, not a budget fact.

Round 1 is **single-thread** plus the constructor. The paradigm test is not “beat 8-worker wall on 1 core”. Round 2 is 8 workers with the existing replay identity: worker seed = `(request_seed, epoch, worker_ordinal)`, serial merge by ordinal, completion order unobservable (`next-generation-engine-plan.md` §replay). Do not invent a new parallel story.

Anytime curve (3/10/30) is reported from wall **and** from a calibrated work cap. Gates read the work cap. Interpolation of staircase depths is forbidden (round-envelope gate’s false 8/12).

Suggested ICS work caps, to be *measured* on a quiet box in round 1 and then frozen like `plancal`:

| nominal | constructor cap | ICS work cap | role |
|---|---|---|---|
| 3 s | first legal or 0.65 s | remaining, likely small | honest constructor-dominated curve |
| 10 s | 0.65 s | cap at p95 wall \(\le 9.2\) s on the quiet box | **the metric** |
| 30 s | 0.65 s | 3× the 10 s cap | transfer / diminishing returns |

3 s may be *worse* than today’s 179.690 if ICS has not yet published. Report that. Do not starve 10 s to defend 3 s.

### 2.3 When to stop and how to legalize

Attempt publication only when **all** of:

- \(\max_i g_{ij} \le 0.05\,\mathrm{mm}\) (proxy-near-legal at current \(W\));
- raw source depth of the *snapped* pose set would beat the incumbent by \(\ge 0.001\,\mathrm{mm}\);
- this fingerprint has not already failed publication.

**Publication pipeline** (`publish.rs`), in this order:

1. Snap translations with `to_grid_mm` (1 µm, ties-away-from-zero, already in tree). Leave \(\theta\) in f64; the existing transform path (`sin_cos` → canonical rings) is what the kernel consumes. Do **not** snap \(\theta\) to 2.5°. Fingerprint \(\theta\) at \(10^{-6}\) deg, the key scale `general_relaxed` already uses for identity, not for search.
2. Build `GridSet::of` on the snapped rings.
3. **Kernel Exclusive** at \(r=2.500\,\mathrm{mm}\) (`two_r = 5000`). Not Union. Not 2.502. Union re-admits the 1 µm miter leak; 2.502 refuses Sparrow’s two radius-caused pairs. This is a **declared experimental publication policy**, not a silent change to HEAD’s composite.
4. **Untouched** `validate_publication` (material 5.0/5.0 on source rings).
5. If either refuses: **publication repair** — PGS on the *snapped* poses, kernel+contract as the boolean, SAT/closest-feature as the direction, snap after every round, displacement cap 2 mm, 80 rounds, \(\theta\) frozen in this repair (so we do not fight the kernel’s canonicalization). This is the one place existing micro-legalization *ideas* are allowed; **do not call** `general_micro_legalization::{micro,global}_legalize` (miter envelopes, `polygons_overlap_exact`, frozen-θ against the wrong geometry).
6. If repair succeeds and both gates pass: replace the public incumbent. Record `inflation_mm = legalized_raw_depth - W_at_Φ≈0`.
7. If repair fails: count a failed attempt, mark the fingerprint, continue ICS. **Not a kill.**

HEAD production path (`PolygonSet::offset`, miter composite, `rek` default Off) is not compiled into this example. The four pinned gates stay on an unarmed default binary.

**Allowance.** Searching and publishing at \(r=2.500\) matches the public 5.0 mm contract. Comparing raw source depth to HEAD’s 168.484/175.388 is valid because those records are already raw-source (`raw_source_long_axis_depth_mm`). Do not mix envelope-basis depths (kernel README §7). Do not quietly drop 0.002 to “make Sparrow fit” and then quote 150 as a 10 s win of search.

---

## 3. The gate, pre-committed — replace the draft

The draft (round 1 = no gate; round 2 = beat plan 175.388 on ≥6/9; round 3 = beat wall 168.484 median; kill on round-3 fail or “publication rate < 100%”) would pass a run that is the constructor plus noise, would call 6/9 vs 175 a success when the *wall* arm of the old engine is already 168.484, and would kill on failed *attempts* that are the definition of this family.

### Round 0 — hours, no search loop

Overlap-measure microbench on mixed-61, numbers in §1.3. **Kill the program here** if incremental \(\Phi\) cannot hit the table. Also: \(\Phi=0\) on the committed Sparrow 10 s fixture at \(r=2.500\) (calibration; we already know the kernel accepts; this checks the *penalty* agrees). If \(\Phi>10^{-2}\) on that fixture, the measure is wrong.

### Round 1 — infrastructure + first curve. **Has a gate.**

Mixed-61, seeds 0–8, from-request, single-thread ICS + capped constructor, work cap frozen after a quiet-box calibration.

**Paradigm pass** (all required):

1. At least **3/9** seeds publish a layout with `contractValid ∧ kernelExclusiveValid(r=2.500)` at raw depth **\(\le 168.484\)**.
2. That layout is **not** a constructor fingerprint and not within 0.5 mm RMS pose of the constructor (otherwise we did not leave the island).
3. Every published incumbent in the run is dual-valid. Failed publication *attempts* are reported, not kills. Target: attempted legalization success rate **≥ 50%** among \(\max g\le 0.05\) states; if \(<20\%\), the proxy is lying — yellow flag, fix \(\Phi\), do not proceed to round 2.
4. Median **legalization inflation** \(\le 8\,\mathrm{mm}\). If \(>8\,\mathrm{mm}\), the family is eating the strip — this *is* the wasted-month mode; **kill**.
5. Hygiene, not success: the anytime floor is at least the constructor’s own depth. If we *lose* the constructor incumbent, the driver is broken.

Beating 175.388 is **not** a round-1 success. The plan arm is the old engine leaving wall on the table. A new engine that lands at 174 has not tested ICS.

No transfer in round 1 except a 2-hour **triangle-20 canary**: locked strip at that fixture’s 10 s mark (70.742). All triangles, SAT is exact MTV. If this canary cannot legalize, mixed-61 numbers are not to be trusted.

### Round 2 — 8 workers, 10 s *work-capped* (plancal-style), mixed-61 seeds 0–8.

**Pass:** median raw depth **\(\le 168.484\)** (beat the *wall* arm, reproducibly), and **≥6/9** seeds \(\le 168.484\). Same dual-validity 100% on published. Inflation median \(\le 5\,\mathrm{mm}\).

Transfer: shapes-17 and triangle-20 must not regress vs their current 10 s plan marks (200.349 / 70.742) by more than 1 mm median. A mixed-61-only win is overfit.

**Kill the program if round 2 fails.** That is “is this family real at 10 s on this box”. Do not spend round 3 hoping the shrink schedule saves a separator that cannot legalize into 168.

### Round 3 — toward Sparrow-class, still not 150 as a kill.

**Pass:** mixed-61 10 s work-capped median **\(\le 160\)**, at least **one** seed \(\le 155\), transfer holds (shapes-17 and triangle-20 medians \(\le\) their round-2 numbers).

**Do not kill for missing 150.165.** That is the campaign after a working prototype. A 160@10s median from a 168.484 wall-ref is the generation-gap confirmation; 150 is then an engineering problem (workers, jump set, \(\Phi\) fidelity on the 9 stars), not a family question.

Sparrow 150.165 remains the *calibration* ceiling, not the round-3 kill. If round 3 median is in (160, 165], that is a yellow continue-or-not for the owner, not an automatic kill.

### What I will not accept as a gate

- Best-seed quotes (8.6 mm spread).
- Linear interpolation of anytime staircases.
- Envelope-basis millimetres.
- Union-kernel publications.
- Any arm that ran m34/`crot`/v3 in the same process.

---

## 4. Determinism and honesty

### 4.1 Achievable contract

| claim | honest? |
|---|---|
| Same binary, same x86_64 box, same work cap, same seed → bit-identical document | **yes**, and this is the gate contract |
| Cross-platform bit-identity of the ICS loop | **no**. `sin_cos`, SAT `hypot`, FD-free but still f64 PGS. Do not write it down. |
| Cross-platform published depth agree to 1 µm | **aspirational**, not a round-1 gate. Kernel Exclusive *given identical GridSets* is integer and platform-identical; getting identical GridSets from f64 poses is not. |
| Wall-clock 10 s → same millimetres next Tuesday | **no**. That is why `plancal` exists. ICS gates are work caps. |

Pin, or the gates are theatre:

- `libm` for `sin`/`cos`/`hypot` (already in tree; `sat.rs` already learned this the hard way: 20/269 ULP residuals vs TS).
- Piece-ordinal PGS order, not spatial-hash order.
- RNG = existing request-seed PRNG, one stream per worker ordinal; no `thread_rng`.
- Work units from §2.2, charged in the loop, **no `Instant`**.
- Worker merge by ordinal (round 2+).
- `KernelMode` is **request-scoped** in this example (do not reuse the process-global atomic as the production path; Sol 13 already forbade promoting that).

Replay identity includes: feature set, worker count, work cap, `r=2.500`, libm, toolchain. Same as the campaign’s lesson that a digest is not a certificate: the document fingerprint is placements + raw depth + both validity flags.

### 4.2 Self-deception modes in this family (pre-commit the counters)

| failure | how it looks | counter that must be in every evidence JSON |
|---|---|---|
| **\(\Phi\) tourism** | overlap drops 80 mm → 4 mm at \(W=152\), never publishes | `phi`, `max_g`, `W`, `published_raw_depth` on one timeline. Metric is published depth. |
| **Legalization inflation** | \(\Phi\approx 0\) at 152, snap+repair emits 171 | `inflation_mm` per successful publish; median in the gate |
| **Proxy lie** | `max_g≤0.05` but kernel refuses 80% | `legalization_attempted` / `legalization_accepted` |
| **Constructor cosplay** | 174 mm that is the constructor topology nudged | RMS pose vs constructor; fingerprint inequality |
| **Cherry seeds** | seed 2 is 159, median 174, quote 159 | always 9 seeds, median **and** count-below |
| **Allowance games** | search at 2.500, compare to HEAD as if composite radius matched | declare \(r\); compare raw source depth |
| **Staircase interpolation** | already falsified 8/12 on the kernel gate | step-function at the work cap only |
| **jagua “just for the quadtree”** | copying Sparrow’s collision backend | feature set forbids `jagua-experimental` on this example |
| **Wall/work scramble** | 16 M work finishing faster than 8 M under load, booked as a win | work-capped cells, one run per cell (see coordinator-v4 work table) |

If an evidence file cannot name inflation, attempt/accept, and constructor-fingerprint distance, it is not a gate result.

---

## 5. Reuse map

### Consume

| asset | how |
|---|---|
| `validation/round_envelope.rs` | **final envelope judge**, Exclusive, \(r=2.500\). `GridSet`, box gap, `pair_admissible`, `boundary_admissible`. Never as \(\Phi\). |
| `validation/general_polygon.rs` `validate_publication` | **untouched** material 5.0/5.0 |
| `validation/sat.rs` | penalty MTV for convex–convex; first production caller |
| `geometry/convex.rs` | hulls |
| `geometry/predicates.rs` | orientation / containment sign |
| `canonical_grid::to_grid_mm` | publication snap |
| `construct_short_side_first` | anytime floor, time-capped; not ICS init |
| plan/anytime harness pattern (`plancal`, 3/10/30 table, quiet-box work calibration, four pinned gates as non-regression) | **measurement**, not search |
| Sparrow `fixture/sparrow-10s-x86-poses.json` | \(\Phi=0\) calibration + “would publish?”; **not** a seed |
| request-seed PRNG / replay-ordinal merge | determinism |

A small, **independent** ear-clip of source rings is allowed. Do not import `triangulate_ring` from `general_relaxed.rs` (that triangulates *miter-offset* collision rings).

### Do not touch

- `search/portfolio.rs` and every coordinator key (`v3`, `race`, `lanes`, `lanedebit`, `m34*`, `crot`, `sparserot`, `replan`, `planprobe`).
- `general_relaxed.rs` (27 k lines, 2.5° lattice, miter surrogates).
- `compression_schedule.rs` / mode 34.
- `general_micro_legalization.rs` and `contact_block` / `se2_certificate` (wrong geometry, wrong residue scale, frozen \(\theta\)).
- `general_hazard.rs`, `search/kernel/jagua.rs`, the `jagua-rs` crate.
- `PolygonSet::offset` production path; `KernelMode::Union`.
- NFP/IFP service, free-material Clipper, skip-pile dump.

Round 1 may *call* `construct_short_side_first` and the two validators. It may not *link* the relaxed lane.

---

## 6. Biggest risk, and the cheapest falsifier

**Biggest risk: legalization inflation.** ICS will happily drive \(\Phi\to 0\) at a Sparrow-class \(W\). Snap to 1 µm + exact disc + 9 concave stars reopens pairs. Repair then inflates \(W\) by millimetres and you republish inside the 168–180 island the old engine already owns. That is the documented behaviour of `global_legalize` on large residues, and it is the standard ICS failure on non-convex nesting. A month of shrink schedules, jump heuristics, and 8-worker plumbing will not be distinguishable from progress if this is happening, because \(\Phi\) and even “proxy-feasible at 152 mm” will look great.

**Earliest cheap measurement — half a day, no shrink, no constructor, no workers.** Call it the **inflation probe**. Pre-commit the reading before running it (the skip-pile round did this correctly).

Four cells, mixed-61, single-thread, 2 s cap each:

| cell | setup | pass | kill |
|---|---|---|---|
| S0 | Sparrow 10 s fixture, unmodified | \(\Phi=0\), kernel Exclusive accepts at \(r=2.500\) (already known for the kernel; this checks \(\Phi\)) | \(\Phi>10^{-2}\) |
| S1 | Sparrow poses + \(\mathcal{U}[-0.5,0.5]\,\mathrm{mm}\) and \(\mathcal{U}[-2^\circ,2^\circ]\) per piece, strip locked at 150.165 | PGS returns to kernel-legal at \(\le 150.165\) | cannot stay in a known basin |
| S2 | same, \(\mathcal{U}[-2,2]\,\mathrm{mm}\) / \(\pm 10^\circ\) | legal at \(\le 152\) | separator too weak for ICS jumps’ wreckage |
| T | **random overlapping throw** of all 61 into **locked \(W=168.484\)**, 8 jumps allowed | publishes dual-valid at \(\le 168.484\), inflation \(\le 8\,\mathrm{mm}\) | **this is the wasted-month detector** |

If T fails, stop. Do not build `shrink.rs`, workers, or a 3/10/30 driver. The family cannot beat wall-ref from a throw on this geometry with this \(\Phi\); everything after is choreography.

Optional same-afternoon canary: cell T on triangle-20 at \(W=70.742\). Cheaper, exact SAT. Fail there ⇒ implementation bug, not geometry.

**Second risk, cheaper still:** round-0 throughput. If \(\Phi\) is a Clipper offset, 10 s is thousands of evals, not millions, and we already know from the constructor census that exact geometry at that rate does not search.

**Third, not the biggest:** copying Sparrow by “borrowing” jagua’s quadtree or compact-and-separate control flow. Process rule, not a measurement: the example’s `Cargo.toml` features must not include `jagua-experimental`.

---

## Disagreements, listed so Sol can mark them

1. **Do not init ICS from the constructor.** Anytime floor only. (Draft implied a hand-off of the 1.4 s output as the search state.)
2. **Do not use the kernel as the overlap measure.** Judge only. SAT + closest-feature on source rings for \(\Phi\).
3. **Round 1 has a gate**, and it is *paradigm* (3/9 seeds dual-valid \(\le 168.484\) from a throw, inflation \(\le 8\), not constructor-cosplay), not “first curve, no gate”.
4. **Beating 175.388 is hygiene, not success.** Round 2 must beat **168.484 median, work-capped**. 6/9 vs the plan arm is a bar the old wall arm already clears.
5. **“Publication rate 100%” applies to published incumbents, not attempts.** Kill on inflation and on proxy-lie rates, not on refused legalizations.
6. **Do not kill round 3 for missing 150.** Kill at round 2 if 168.484 median is not beaten. Round 3 pass is 160 median / one seed \(\le 155\).
7. **Rotation is continuous from round 1**, catalog used only to draw the throw. Continuous-around-catalog-seeds as the *search* reintroduces 57/61.
8. **Publish Exclusive at \(r=2.500\), not Union, not 2.502.** Declared experimental policy. HEAD miter path untouched.
9. **PGS is the local solver, not the search.** Without jumps + locked-then-shrink strip, this is `global_legalize` and Sol 11 already closed it.
10. **Single-thread in round 1.** 8-worker 10 s is round 2. Do not ask a paradigm prototype to beat an 8-worker wall on one core and then read the failure as “ICS does not work”.

If we converge on T-before-the-loop, Exclusive@2.500, constructor-as-floor, and kill-at-round-2-vs-wall, I will treat the rest (swap vs relocate order, 0.05 mm vs 0.02 mm publication trigger, \(\delta=1\) vs 2 mm) as round-1 knobs, not consultation blocks.


# ROUND 2 — convergence (after reading Sol's reply)

## 1. Named disagreements

### 1.1 Triangle nine-point Minkowski vs SAT + closest-feature — **KNOB** (I yield Round 1 to Sol)

Same family: unoffset source, convex cells, signed clearance deficit, max violation per pair, not area, not GJK/EPA, not the kernel as \(\Phi\).

On two triangles the nine-point Minkowski difference and SAT are the same query. Closest-feature on **raw nonconvex rings** misses containment; Sol is right and I had already patched that with point-in-polygon. `sat.rs` also returns `None` for disjoint/touch, so it is not a signed field.

**Round 1 primitive is Sol’s:** deterministic ear-clip of source outer rings (holes rejected, not filled; do not import `general_relaxed`’s clipper), nine-point signed distance, \(v_{ij}=\max_{a,b}[c_{\rm pair}-s_{ab}]_+\), \(c_{\rm pair}=5.0\) on mixed-61.

**Named fallback, not a consultation block:** if Gate 0 force-correlation misses, the first amendment is hull-as-one-cell SAT for the 52 convex pieces (max-triangle penetration underestimates MTV; that is Sol’s own deepest-triangle risk). Do not kill the family until that amendment is measured.

### 1.2 Start state — **DISAGREE** on the target, **AGREE** on the injector and the floor

| piece | mark |
|---|---|
| Constructor as anytime exact incumbent / fallback | **AGREE.** Random throw with no fallback was the weaker half of my draft. |
| Affine centroid compression, rigid shapes, distributed overlap | **AGREE.** Better than a uniform SE(2) soup. This is how you enter a designed infeasible state. |
| First \(T = D^\*-0.10(D^\*-L)\) | **DISAGREE.** That is a 5–7 mm nudge of the constructor. The retained-infeasible experiment already compressed a feasible parent, dropped raw overlap \(1596\to 975\), and produced **zero** feasible states. A milder shock with a nicer field is that experiment again. |
| ICS state = constructor poses | **DISAGREE** with treating that as the search topology we are testing. It is the island. The shocked **copy** is the ICS state; the constructor fingerprint is the floor, never a “child”. |

**Minimal fix:** first locked strip is **\(W = T = 168.484\)** mm. Bisect the affine factor onto that depth. Do not grow \(T\) above 168.484 to rescue a failed T. Homotopy bisection \((T+D^\*)/2\) is legal **after** T passes, on later deeper targets, retaining the infeasible state as Sol specified.

I yield the random throw.

### 1.3 Gate 0 (10 k-state, 3–5 days) vs inflation probe S0/S1/S2/T — **AGREE** on content, **DISAGREE** on blocking shape

Sol’s kill criteria are the right ones: no proxy-feasible / exact-invalid outside the 4 µm band, no containment false-feasible, force correlation 95 % / 80 %, \(\ge 100\) k piece-proposals in eight seconds.

They are also **my S0/S1/S2/T**, written as a geometry battery:

| mine | Sol |
|---|---|
| S0 \(\Phi=0\) on the Sparrow 10 s fixture | calibration of the measure (test, not a seed) |
| S1/S2 basin return | force-correlation |
| T legalize into 168.484 | 2 s affine-shock repair test, **if retargeted** |
| round-0 throughput | 100 k / 8 s |

**Disagree:** 10,000 states and 3–5 days must not block T. Implementing `decomposition`/`contact`/`publish` **is** Gate 0; T plus a few hundred deterministic shocked states plus the throughput projection is the kill; the 10 k battery is the first **heavy** round-boundary confirmation of Gate 0, not a second research week.

If T at 168.484 fails, stop. Do not build `homotopy.rs` epochs, workers, or a 3/10/30 driver.

### 1.4 Gauss–Seidel + guided penalties, no jumps vs PGS + jumps — **AGREE** on sequencing, **KNOB** on the local solver

**Yield the local solver:** one-piece Gauss–Seidel, max incident guided energy, stable ID tie-break, SE(2) metric with \(R_i\), backtracking ladder, strict guided decrease, incremental row updates. That is cleaner than pair-PGS on both endpoints, and it is not contact-block (exact rejection does not shorten a step).

**Yield GLS** \(u_{ij}=v_{ij}/(1+p_{ij})\) as the first landscape change. Better than stall-then-swap.

**Agree no jumps until T passes.** Jumps on a field that cannot legalize a locked 168.484 shock would hide the failure, as Sol says, and as `global_legalize` already did on large residues.

**Disagree that Round 1 stays jump-free if T is only a 10 % residual.** Under the retargeted T, this mostly dissolves: if the field legalizes into 168.484 without jumps, the paradigm test does not need them; the one worst-pressure relocation stays the **single** predeclared Round-2 addition. If T fails, jumps are not licensed.

### 1.5 Round-2 gate \(\le 175.388\) + 6/9 paired + \(\ge 1\) mm vs beat **168.484** median — **DISAGREE. I will not yield.**

175.388 is the reproducible plan arm leaving wall on the table. The old wall arm is already 168.484. A new engine that beats 175.388 by 1 mm on 6/9 has not tested the family.

Adopt Sol’s **methodology** (contemporaneous interleaved control, nine **distinct** seeds, paired wins, p95 wall, child rate separate from fallback). Replace the number.

| round | pass | kill |
|---|---|---|
| **0 / T** | dual-valid child at \(\le 168.484\), repair \(\le 16\) µm, no jumps, 2 solver seconds, on a predeclared shocked constructor copy (not a 3-seed median theatre) | T fails |
| **1** | 10 s work-capped, **\(\ge 3/9\)** distinct seeds publish a **strict continuous child** at \(\le 168.484\), every published incumbent dual-valid, inflation = repair \(\le 16\) µm | child rate \(< 3/9\), or any invalid publish |
| **2** | median \(\le 168.484\), **\(\ge 6/9\)** \(\le 168.484\), paired median vs contemporaneous plan \(\ge 1\) mm (hygiene; implied by the median), p95 from-request \(\le 10\) s, \(\ge 6/9\) strict children. One relocation allowed since Round 1. | **kill the family** |
| **3** | median \(\le 160\), \(\ge 1\) seed \(\le 155\), transfer holds on shapes-17 / triangle-20 | yellow for the owner if in \((160,165]\); do **not** kill for missing 150.165 |

Beating 175.388 is implied by beating 168.484. It is not a success bar.

### 1.6 Repair cap 16 µm + publication pipeline vs inflation — **AGREE. This answers the accounting half.**

Sol’s pipeline is the better publication contract. I yield pose-snapping of \(t_x,t_y\):

1. Continuous \(f64\) pose, no 1 µm snap of the pose.
2. Transform source rings; **`GridSet::of` is the sole 1 µm canonicalization**.
3. Search allowance **0**.
4. Pure round predicates at the requested contract radius: `pair_admissible` / `boundary_admissible` with \(r=2.500\), `two_r=5000`. **Not** `KernelMode::Union`, **not** 2.502, **not** the process-global atomic. Request-scoped function calls.
5. Untouched `validate_placements_against_contract` last.
6. Repair only inside \(\varepsilon_{\rm grid}=4\) µm, freeze \(\theta\), cap **16 µm/piece**, else **discard the checkpoint**. `best_exact` unchanged.

That closes the lie I named: \(\Phi\approx 0\) at 152, repair emits 171, booked as a find. 16 µm is quantization. My 2 mm / 80-round repair with an 8 mm median kill was still `global_legalize`.

**It does not close the search half by itself.** A trajectory can still drive guided energy to zero at a \(T\) that cannot be legalized inside 16 µm; then you honestly keep the constructor floor at \(\sim 180\) and spend the month on a field that never converts. That is why T must be locked at 168.484 **and** why proxy energy must never be the anytime column.

---

## 2. The four convergence blocks

| block | Sol | amendment |
|---|---|---|
| **T-before-the-loop** | Partial. Gate 0 + 2 s shock exist; the shock is \(0.10(D_0-L)\approx 5\)–\(7\) mm and the pass is half of that. | **Retarget T to locked \(W=168.484\).** Affine-compress the constructor copy onto that strip. No jumps. Repair \(\le 16\) µm. Dual-valid child or kill. No homotopy rescue of T. |
| **Exclusive@2.500** | **Yes.** Pure predicates at contract radius, not Union, not a process-global arm. Mixed-61’s 5.0 mm is exactly representable. | Name it: \(r=2.500\), `two_r=5000`, request-scoped. Do not silently round a non-integer contract outward. |
| **constructor-as-floor** | Partial. He uses it as floor **and** as ICS init. Child-rate vs fallback-rate is the right distinction. | Two incumbents. Constructor poses → affine **copy** = ICS state. Constructor fingerprint is never a child. RMS-vs-constructor cosplay test **withdrawn** (a legalized 168 from a shocked constructor *will* look like that topology). |
| **kill-at-round-2-vs-wall** | **No.** He kills at Round 3 vs 168.484; Round 2 is 175.388. | Round 2 **is** vs 168.484 median, work-capped, 6/9. Fail → stop the family. Round 3 is 160 / one seed \(\le 155\), not 150 as a kill. |

If those four amendments are in, I treat swap-vs-relocate order, 8 vs 24 epochs, 0.25 µm vs 8 µm ladder rungs, and hull-SAT-as-fallback as Round-1 knobs.

---

## 3. Two-tier test discipline

This family’s three self-deceptions are **legalization inflation**, **proxy-energy-as-progress**, and **f64 nondeterminism**. A FAST tier that does not catch all three is not a FAST tier; it is a way to discover the lie at the round gate.

### FAST — every iteration, minutes

**Single feature combo:**

```text
--features round-envelope-kernel,continuous-overlap
```

Never `jagua-experimental`. Never the campaign combo (`jagua-experimental,compression-schedule,...`). The example’s `required-features` are exactly those two.

**Compile-only default build (must stay green when the feature is red):**

```text
cargo check -p polygon-nesting-core
cargo check -p polygon-nesting-cli
cargo test  -p polygon-nesting-core the_shipping_defaults_are_v3_plus_three_and_v3_is_off --offline
```

Plus one new cfg test: default features do not expose `optimize_continuous` / `search::continuous_overlap`. `cargo test --no-run -p polygon-nesting-core` on default features is the compile-only gate. **Do not** run the four pinned engine gates here.

**Unit tests** (`cargo test -p polygon-nesting-core --features round-envelope-kernel,continuous-overlap continuous_overlap`, seconds):

| test | catches |
|---|---|
| Nine-point signed distance on two triangles: disjoint / touch / penetrate / vertex-in-triangle | wrong primitive |
| Containment false-feasible = 0 | signed-segment lie |
| Incremental row \(\Phi\) == full rescore | tracker drift |
| Holes: explicit reject, no fill | silent topology change |
| **S0 (pinned number: 0):** Sparrow 10 s fixture scores \(\Phi = 0\) (\(\lvert\Phi\rvert<10^{-6}\)) at \(c_{\rm pair}=5.0\); Exclusive `two_r=5000` accepts; contract validator accepts | proxy lie on a known-legal layout |
| Publication: a 0.5 mm exact deficit is **discarded**; `best_exact` unchanged | inflation accounting |
| Any accepted publish with per-piece repair \(>16\) µm fails the test | inflation |
| Reporter schema: anytime column is exact-valid raw source depth; `phi` / `T` / guided energy are not quality | proxy-as-progress |
| 10 k piece-proposals, seed 0, twice, bit-identical poses | f64 / order / FMA drift |
| One invalid publication fails hard | invariant |

S0 is a **correctness test of the specified measure**, not a tuning surface. Constants stay frozen. If \(\Phi\neq 0\), the geometry is wrong; do not retune against the fixture.

**Smoke canary** (one command, \(\le 2\) minutes wall):

```text
overlap_ics / continuous_overlap benchmark
  fixture = triangle-20
  seed    = 0
  strip   = 70.742          # that fixture’s 10 s plan mark
  work    = 25_000 piece-proposals   # ~2 s if 12.5 k/s holds
```

**Pinned numbers:** `invalid_publications = 0`; constructor floor present; `repair_um_max ≤ 16` on any accepted child. Not required to beat 70.742 every iteration (that is mechanism). Required: if it emits a child, giveback is quantization-scale.

Triangle-20 is all triangles: the nine-point query is exact MTV. Fail here ⇒ implementation bug, not mixed-61 concavity.

### HEAVY — round boundaries only

- Gate 0 full: \(\ge 10\,000\) deterministic states (constructor × 1 %/3 %/10 % residual **and** the 168.484 shock), force correlation 95/80, zero false-feasible outside 4 µm, projected \(\ge 100\) k proposals / 8 s.
- Nine **distinct** seeds, mixed-61 exact-clearance, 3/10/30 work-capped + p95 wall. Contemporaneous interleaved plan **and** wall controls. No historical-only 175.388.
- Dual-validity 100 % on published; child rate; repair displacement; depth giveback; time-to-first-strict-child.
- Determinism: 3 seeds × 2 replays, same binary/toolchain/libm/work cap → bit-identical poses, checkpoint sequence, publications. Cross-platform `sin/cos` identity is not a claim.
- Transfer: shapes-17 (200.349) and triangle-20 (70.742) — no validity failure; Round 2+ paired median regression \(>1\) mm is a fail.
- **Default-build** four pinned regression gates as whole documents (mode-20 206.869 / `8a773738…` and the three mode-22 records), plus the ordinary core / protocol / canonical-quality / canonical-grid vector suites. Feature-off binary only.

### What must NEVER be deferred to HEAVY

These are cheap, and they are exactly the three self-deceptions. Deferring them is how a round gate becomes a surprise.

1. **Default binary isolation** — compile-only + shipping-defaults test + cfg gate. One leaked `jagua-experimental` or a default-path digest change is a stopped iteration, not a Round-2 finding.
2. **\(\Phi=0\) on the Sparrow fixture (pinned 0)** — proxy lie.
3. **Containment false-feasible = 0** — the named hole in segment distance.
4. **Repair cap on every accepted publish (16 µm)** — legalization inflation. A child that moved 0.5 mm is a FAST fail, not “we’ll look at inflation at the gate”.
5. **Invalid publication = fail** — invariant, not a rate.
6. **Anytime column is exact raw depth** — proxy-energy-as-progress.
7. **Incremental \(\Phi\) == full rescore** — otherwise GLS weights a fiction.
8. **Short-trajectory bit-identical replay (10 k proposals, seed 0, twice)** — f64 nondeterminism. If this flakes, 9-seed gates are theatre.
9. **Chinese wall in the graph:** prototype `required-features` must not include `jagua-experimental`; fixture is not a seed and not a parameter source.

Throughput at 100 k/8 s **may** wait for Gate 0 heavy if the FAST microbench already shows incremental row updates in the right band (order \(10^4\) proposals/s on triangle-20). False-feasible, inflation, and replay may not wait.

---

## 4. Converged spec for the implementing agent

Feature-gated module, example-only driver, default build byte-identical to HEAD.

```text
search/continuous_overlap/          # feature = "continuous-overlap"
    mod.rs          optimize_continuous()
    state.rs        poses, contact matrix, exact incumbent, constructor floor
    decomposition.rs  deterministic source-ring triangulation; holes = error
    contact.rs      nine-point Minkowski signed distance + witnesses
    broad_phase.rs  f64 piece/cell AABBs
    energy.rs       raw and guided penalty, incremental row deltas
    descent.rs      one-piece SE(2) Gauss–Seidel, backtracking ladder
    homotopy.rs     strip targets and affine shocks
    publish.rs      Exclusive r=2.500 + 16 µm repair + contract
    diagnostics.rs  work and anytime trace (exact depth only)

examples/continuous_overlap_benchmark.rs
```

`continuous-overlap` implies `round-envelope-kernel`. It does not imply `jagua-experimental`. Not an `ExplorationKernel`. Not wired into `run_portfolio` / `GeneralRelaxedOutcome.result` until Round 2 passes.

**State.** `Pose { tx_mm, ty_mm, theta_rad, mirrored }` continuous \(f64\). \(\theta\) accumulates over the circle; no 2.5° catalogue; mirrors frozen in Round 1. Cache transformed triangles + AABBs. A proposal updates one piece and its \(n-1\) contact rows.

**Objective.** \(E=\sum_{i<j} \tfrac12 w_{ij} v_{ij}^2 + \tfrac12\sum_{i,k} w_{ik} v_{ik}^2\), \(v_{ij}=\max_{a,b}[5.0-s_{ab}]_+\) on mixed-61. Target depth \(D\) is a hard boundary, not \(E+\lambda D\). Raw and guided stored separately. Guided: \(u_{ij}=v_{ij}/(1+p_{ij})\), increment lex-first max-utility contact, \(w=1+p\).

**Moves (Round 1).** One Gauss–Seidel trajectory as Sol specified. No swaps, teleports, archive, or mirror flips. Exact geometry is not a move-acceptance predicate.

**Start.** Fast constructor (`construct_short_side_first` via `InitialLayoutProvider`) produces `best_exact` (floor). ICS state is an **affine-compressed copy** of those poses, centroids scaled along the long axis onto **locked \(T=168.484\)**. Shapes rigid. Two incumbents; ICS never mutates the floor except through `publish.rs`.

**Homotopy (after T).** Descend at fixed \(T\). On successful child, set \(D^*\) to raw exact depth and take the next 10 %-residual target **below** \(D^*\). On epoch-limit failure, \(T\leftarrow(T+D^*)/2\), keep the infeasible state. Eight work epochs in the 10 s slice (constructor \(\le 1.4\) s measured, preprocess \(\le 0.2\) s, 8.0 s search, 0.4 s headroom). 3 s = 1–2 epochs of the same trajectory; 30 s continues it. Work-capped for gates; clock read only between batches in wall mode.

**Publish.** Continuous pose → proxy inside 4 µm band → `GridSet::of` → Exclusive predicates at \(r=2.500\) (`two_r=5000`) → bounded repair \(\le 16\) µm/piece, \(\theta\) frozen → `validate_placements_against_contract` → `best_exact`. Allowance 0. No pose rounding. Discard otherwise. Deadline returns `best_exact`.

**T (before the loop, hours after contact/repair exist).** One predeclared shocked constructor copy at \(W=168.484\), no jumps, \(\le 2\) solver seconds. Pass: dual-valid child at \(\le 168.484\), repair \(\le 16\) µm. Fail: stop.

**Work currency.** Piece-proposals, triangle-pair queries, AABB rejects, accepted moves, weight updates, exact checkpoints, repair corrections. Kill if cannot project \(\ge 100\) k complete piece-proposals into eight seconds after incremental rows.

**Determinism.** Same request, seed, binary, x86, toolchain, libm, feature set, worker count, work quota → bit-identical poses, checkpoints, publications. Serial in Round 1. If later parallel: contact-row partitions only, predetermined slots, reduce in pair-ID order, no completion-order observation. Pin triangulation, ties, FMA, seed, constructor config, \(T\) rule, ladder, weights, repair cap, work unit. Replay/gates: no `Instant` in the trajectory.

**Reuse.** `GeneralFastPiece` / settings / placements, constructor, `PolygonSet`, pure `GridSet` + pair/boundary/critical-radius, untouched contract validator, raw-source depth, `plancal` **methodology** (not its numeric file), 3/10/30 trace format.

**Do not use.** Coordinator, archive, modes 20–34, `LaneSearch`, `general_relaxed` and its triangulation, catalogues, miter offsets, Clipper overlap area, NFP in the move loop, `GridSet` as broad phase, Jagua / Sparrow optimizer, exact rejection in line search, global kernel arm, portfolio miter lower bound, `general_micro_legalization`.

**Chinese wall.** `jagua-rs` unused. Sparrow pose fixture = S0 correctness test + post-gate holdout. Not a seed, not a parameter source. Freeze constants before using it as diagnosis.

**Telemetry (every evidence JSON).** Exact-valid raw source depth as the only anytime quality; `phi`, `max_v`, \(T\), guided energy on the same timeline as diagnostics; proxy-near attempts, round failures, contract failures, repair µm, depth giveback, time to first strict child, constructor fingerprint inequality, child-or-fallback flag. If a file cannot name those, it is not a gate result.

**Gates.** As the table in §1.5. Nine distinct seeds. Failed *attempts* are not kills; invalid *outputs* are. Constructor fallback is not a mechanism success.

### Remaining disagreement I refuse to yield

**The first locked strip, and therefore the Round-2 kill, is wall-ref 168.484 — not a 10 %-residual constructor shock and not plan 175.388.**

Sol’s pipeline, field, and publication discipline are the engine I would hand over tomorrow. A Round-2 that “beats 175.388 by 1 mm on 6/9” is the old plan arm with a new name. If T cannot legalize a shocked constructor into 168.484 inside 16 µm without jumps, the family is not a 10 s bet on this geometry, and no epoch schedule will make it one.
