# Production roadmap - Sol (Codex max) consultation, 2026-08-17

Context: owner directive that records and production converge (Sparrow-class:
157.971mm@3s / 150.165mm@10s measured same-machine, ~270ns/eval, evidence in
docs/experiments/sparrow-mixed61/). Full consultation output below.

## 1. Profile first: where the time is

A read-only ablation using the existing release artifacts was directionally revealing, though it should be repeated from a provenance-clean native build:

- Constructor only: about **1.20 s**, 520,141 reported exact evaluations across four parallel order variants.
- Adding standard relaxed search: about **+1.10 s**, including 3.14M candidates, 21.38M SAT tests, and 12,142 accepted moves.
- Enabling the three coupled arms: about **+0.37 s** in this shallow run.
- Dynamic Jagua mode: about **2.25 s total**, versus about **2.30 s** for rollback. The collision backend reduced SAT work but did not materially reduce wall time.
- The Jagua adapter itself handled 1M complete queries in about **1.006 s single-threaded**, after 30.6 ms preprocessing. That is already approximately 1 µs per complete query before wrapper improvements.
- Standard relaxation performed only three full publication validations in that run. Therefore publication validation is not the primary mode-0 cost, though deep modes invoke exact machinery much more frequently.

Sparrow’s 270 ns number is per reported evaluation, not necessarily equivalent to one of our complete pose scores. The benchmarks must pin both units: primitive/neighbor evaluation and effective piece move.

### Ranked cost centers

| Rank | Cost center | Evidence and action |
|---|---|---|
| 1 | Exact geometry inside search | The constructor materializes a translated `PolygonSet`, evaluates every fixed piece into a temporary `Vec<bool>`, and then rebuilds the collision polygon again for the currently best candidate in [general_fast.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:825) and [publication_confirmed_candidate](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:1792). Deep repair is worse: [construction_confirm_row](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_persistent_vacancy.rs:6184) and [exact_vacancy_child](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_persistent_vacancy.rs:7015) build Clipper collision geometry and scan active pieces for finalist rows. This is the likely campaign-scale killer. |
| 2 | Whole-layout rebuild/rescore around small moves | Coupled scoring reconstructs the hazard state and scores the full layout in [coupled_auditor_score](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:7118). Each round clones the master state and weights for every worker, and each sweep starts by preparing the index and scoring again. [score_state_dynamic](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:10853) still visits all pairs using f64 pressure. One moved piece should never cause an O(n²) score. |
| 3 | Jagua adapter and pressure wrapper | A query creates a fresh `BasicHazardCollector`, creates an owned collision-ID `Vec`, then sorts and deduplicates it in [general_hazard.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_hazard.rs:478). Search then transforms again for bounds and again per colliding neighbor for pressure. [precise_pose_bounds](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_hazard.rs:811) transforms and canonical-snaps every source vertex for every candidate. |
| 4 | Per-candidate and per-round allocation | Hot structures include collision-pair `Vec`s, sorting, `BTreeSet`/`BTreeMap`, contour/edge materialization, cloned states and weights, and linear `.find()` calls while updating a moved row. [update_score_after_move](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:12559) rescans and resorts the global collision list after every accepted move. |
| 5 | Publication validation and diagnostics | [validate_and_measure_placements](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:3280) rebuilds every collision polygon, performs all pair tests, and invokes the independent validator. Its semantics should remain untouched, but invocations should be scarce. Diagnostics formatting is secondary; the larger problem is that the “diagnostics” block actually launches three coupled arms and modes 22–29. |

### Profiling PR

Use fixed streams and batch-level timing rather than timers inside nanosecond loops:

- Constructor spans: proposal generation, transform, offset, AABB rejection, Clipper intersection, confirmation, allocation count.
- Relaxed spans: boundary bounds, Jagua query, pressure, f64 ambiguity confirmation, row merge, index update/rebuild, full rescore.
- Persistent spans: proposal generation, proxy ranking, `build_collision`, exact pair rows, complete validation.
- Run matrices for rollback/Jagua, diagnostics off/on, one coupled arm/all arms, and each deep operator.
- Sample with `perf`/flamegraphs; use heap profiling separately. Per-thread counters should be aggregated at barriers, not updated through hot atomics.
- Define counters precisely: `candidate_queries`, `neighbor_tests`, `effective_piece_moves`, `accepted_moves`, `full_rescores`, and `publication_attempts`.

Immediate cheap wins:

- Reuse the Jagua collector and caller-owned collision-ID scratch.
- Return a bitset or stable dense IDs without allocating/sorting.
- Compute the transform once and return its search bounds with the query.
- Replace pair `BTreeMap`s with dense triangular arrays and generation-stamped bitsets, preserving stable iteration order.
- Cache contours, edges, bounds, expanded area, and discrete canonical collision polygons by `(geometry fingerprint, mirror, canonical angle, contract settings)`.
- For continuous angles, do not create an unbounded cache: the proxy must avoid Clipper entirely.
- Short-circuit the constructor’s all-fixed collision scan after preprocessing has established that later calls cannot introduce a different geometry error.
- Compile the diagnostics operator block completely out of the production route.

## 2. Two-tier evaluation

### Search tier

I would implement an internal, statically dispatched `ExplorationKernel`; the exact Jagua pin already exists in [Cargo.toml](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/Cargo.toml:8), but the current hot path still uses a concrete adapter.

Its fundamental operation should resemble:

```rust
query_moved_into(
    piece_id,
    pose,
    score_cutoff,
    &mut lane_scratch,
) -> Pruned | Complete<MovedRowDelta>
```

The inner search state should contain:

- SoA pose arrays: translation, normalized f64 angle, cached sin/cos, mirror.
- Lane-local Jagua hazard handles.
- Dense triangular `PairState` storage.
- A collision adjacency bitset and per-piece incident loss.
- One reusable moved-row scratch buffer.
- One reusable collector and reported-neighbor bitset.
- A sparse undo record containing the previous pose, boundary entry, and moved pair row.

Candidate evaluation then becomes:

1. Search AABB/boundary calculation from the already transformed proxy or four transformed local-AABB corners.
2. AABB negative rejection and Jagua fail-fast poles for a lower-bound cutoff.
3. Complete Jagua query only when the candidate can still beat the current score.
4. Incremental loss: subtract the old moved row and add the candidate row.
5. Optional pressure only for reported neighbors and only when count/weighted feasibility cannot distinguish finalists.
6. On acceptance, update one hazard and install one moved row.

No full layout rescore, index rebuild, full-state clone, source transform, Clipper offset, or heap allocation belongs in this path.

### Jagua versus our own proxy

Use pinned, unmodified Jagua 0.7.2 first, behind the trait already specified in [the kernel plan](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:122).

Why:

- Its quadtree, continuous transforms, dynamic hazard lifecycle, and fail-fast poles already match the workload.
- The measured adapter is near 1 µs per complete query before removing allocation and duplicate transforms.
- Writing another correct dynamic concave-polygon spatial index would delay the moved-row work, which is currently the larger blocker.

Circle covers or poles alone are not a complete collision oracle. They are useful as:

- A cheap positive lower bound for pruning already-bad candidates.
- A low-cost ranking signal among collision-count ties.
- A source of one or two directed separation proposals per effective move.

If Jagua fails the existing conversion, contact, topology, memory, or update gates, retain the trait and substitute an f32 SoA BVH/quadtree. Do not fork Jagua or weaken validation merely to pass a gate.

### Role of exact and analytic primitives

Exact primitives should reduce candidate count, not become candidate scoring:

- Run analytic closest-feature or segment work once for the current conflicting piece and its small neighbor set to generate a directed proposal. That can replace tens of blind samples.
- Use robust f64 orientation/segment predicates only in the derived ambiguity band after the f32 query. The existing robust sign path is suitable at [predicates.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/geometry/predicates.rs:95).
- Reserve rational/BigInt fallback for rare exact sign or tie cases. The code already records that BigInt allocation dominated these predicates in [canonical_grid/math.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/canonical_grid/math.rs:135).
- Never use rational values as continuous loss magnitudes or run analytic all-segment distance against all neighbors per candidate.

### Publication tier

A candidate enters a deterministic publication queue only if it:

- Is proxy-feasible.
- Improves raw, unrounded depth under the scoped ULP comparison.
- Has not already failed under the same placement fingerprint and pinned contract.

The coordinator then invokes the untouched contractual and independent validators. Both `contractValid` and `exactValid` must be true before replacing the public incumbent. The independent validator remains authoritative at [validation/general_polygon.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:77).

Validate candidates in raw-depth/fingerprint order and stop when one publishes; do not validate every lane.

### Kernel gates

In addition to the existing 250K agreement stream and 3× rollback requirement:

- Zero heap allocations after lane warm-up.
- No Clipper/source-ring call in the candidate-search stack.
- No unconfirmed false negative outside the derived ambiguity band.
- Complete moved-row query: initial target ≤1 µs p50 and ≤3 µs p95 single-core.
- At least 4M complete queries/s and 10K effective moves/s across eight workers initially; converge on or exceed Sparrow’s 14.2K moves/s.
- Stable query/update/state fingerprints across three same-platform runs.
- Sampled full-rescore agreement after commits in shadow/debug builds.
- Exact publication rejection rate tracked; a high rate fails the proxy, rather than being tolerated as validator traffic.

## 3. Anytime mode

The production entry should own a `PublishedIncumbent` from the moment the first layout passes both validators. Search workers may hold infeasible proxy states, but timeout can return only that published incumbent.

A sensible initial budget schedule is:

- Preprocess once and share immutable shape/kernel caches.
- Spend roughly 15% obtaining constructor incumbents from domain-separated salts.
- Spend 45% on infeasible descent from the best few basins.
- Spend 20% on short clamped depth ladders.
- Spend 15% on collision-component repair, including the new per-component beam repair.
- Reserve approximately 5%, or a measured publication p99 if larger, for validation and serialization.

Those are generic portfolio defaults, not Mixed-61 constants. After every operator has received a warm-up quantum, later quanta can be allocated by deterministic improvement-per-work-credit, with an exploration floor and stable tie-breaking.

Important mechanics:

- Derive seeds from `(root seed, operator kind, lane ordinal, generation)`; no shared RNG.
- Workers consume fixed query/update quanta and see a snapshot of the incumbent.
- Merge results only at deterministic ordinal barriers, never completion order.
- Check the deadline only between quanta. Do not put wall-clock branches inside the optimizer.
- Keep a bounded publication queue and always retain the last exact-valid incumbent.
- Emit a transcript containing completed work ordinal, seeds, fingerprints, publication attempts, and raw depth.

There is an unavoidable contract issue: a hard wall-clock deadline and bit-identical output cannot both be guaranteed under arbitrary scheduler jitter. I would expose both:

- Production `TimeBudget`: deterministic ordering and barriers, returning the best published completed prefix.
- Exact replay `WorkBudget`: use the recorded completed-work ordinal for bit-for-bit reproduction.

If identical seed-plus-budget must always mean identical output, the public budget must be deterministic work credits calibrated to a time class, with wall time treated as a soft target. I would not claim strict versions of both simultaneously.

### Adopting modes 22–29

The review finding is correct. The modes are called inside [run_coupled_dynamic_separator_experiment](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3016), starting at [the persistent dispatch](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3123), while the outer function ultimately returns the old `protected` result at [general_relaxed.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:2931).

The safe adoption path is:

- Refactor each operator to return an internal `OperatorOutcome { diagnostics, candidate }`.
- Keep the existing diagnostics caller able to discard the typed candidate, preserving legacy behavior.
- Have the anytime scheduler consume the candidate directly; never reconstruct it from serialized diagnostic placements or rounded depth.
- Convert `UnvalidatedCandidate` to `PublishedCandidate` only through the dual validator.
- Update `GeneralRelaxedOutcome.result` only from a `PublishedCandidate` that beats `protected` by raw depth.
- Preserve mode 0 byte-for-byte until the new profile passes rollout gates.
- Production should run the promoted operator, not control+treatment+boundary arms sequentially.

This makes the 164.058 mechanism a production operator rather than a hidden diagnostic.

## 4. Smallest safe migration sequence

Effort scale: S = 2–4 engineering days, M = 1–2 weeks, L = 3–5 weeks including tests and review.

| PR | Change | Effort | Gate |
|---|---|---:|---|
| 1 | Fixed-stream profiling, phase counters, allocator measurements, precise candidate/move semantics | S | No outcome changes; diagnostics-off overhead below 2% |
| 2 | Hot-loop hygiene: scratch reuse, dense IDs, cached contours/bounds/expanded areas, constructor short-circuit, removal of duplicate collision reconstruction | M | Legacy fingerprints, [canonical quality golden](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/canonical-quality-golden.md:5), and [collision-builder vectors](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/tests/collision_builder_vectors.rs:3) unchanged |
| 3 | Real static `ExplorationKernel` boundary; pinned Jagua `query_into`, reusable collector, fail-fast cutoff, single transform | M | Existing deterministic/agreement/throughput/memory gates at [plan line 124](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:124) |
| 4 | Dense `PairState`, moved-row delta, incident score, one-hazard commit, delta rollback; eliminate per-sweep rebuild/full rescore | L | Shadow full rescoring agrees after every accepted move; ≥10K effective moves/s aggregate |
| 5 | Typed operator/publication outcome and modes 22–29 adoption into `GeneralRelaxedOutcome.result`, feature-gated | M | Both validators required; raw-depth comparison; legacy mode result unchanged |
| 6 | Port constructor and persistent vacancy/replacement/beam operators off Clipper finalist rows and onto the shared proxy kernel | L | Every returned layout exact-valid; deterministic fingerprints; quality non-regression on a topology-stratified DXF corpus |
| 7 | Single-process anytime coordinator, deterministic quanta, constructor/descent/ladder/component-repair portfolio, shadow rollout | L | Exact-valid at every budget; one-process ≤164.058 within a seconds-class budget; continue until the retained 157.97073@3s and 150.16547@10s Sparrow envelope is met or beaten |

The current plan’s old quality thresholds and calibration around [line 137](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:137) are no longer sufficient as a ship criterion. The retained x86 evidence at [the Sparrow addendum](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparrow-mixed61/README.md:57) should define the competitive envelope. These remain benchmark gates, never optimizer constants.

One limitation must be stated plainly: the current kernel plan preserves a one-outer-ring relaxation limitation. “Arbitrary supported simple-region DXF” is achievable in this sequence. Truly arbitrary pieces containing holes or multiple material regions require a separate L-sized grouped-hazard topology implementation; until then those requests must use the protected fallback and cannot honestly carry the same seconds SLA.

## What I would refuse to do

- Copy Sparrow’s optimizer, move schedule, population logic, or operators.
- Put f32, a tolerance, or Jagua in the publication authority.
- Run Clipper, the full validator, BigInt predicates, or all-neighbor f64 confirmation per candidate.
- Hardcode Mixed-61 depths, piece IDs, component membership, ladder rungs, or seed outcomes.
- Cache unbounded continuous-angle collision polygons or round angles to make caching convenient.
- Run control, treatment, boundary projection, and diagnostics arms in production.
- Use completion-order reductions or wall-clock checks inside workers.
- Claim 270 ns parity using incomparable counter definitions.
- “Fix” throughput by adding threads before eliminating allocations and O(n²) rescoring.
- Flatten holes or multiple regions into filled outer polygons.
- Make Jagua default before its agreement, topology, determinism, memory, and quality gates pass.

The decisive work is PRs 3–4. PR 5 makes the record mechanisms real engine outcomes; PRs 6–7 turn them into a Sparrow-style anytime product.
