# Fable 5 Assignment: Safely Reduce Single-Thread Polygon Nesting Runtime

You are working in the `polygon-nesting` Rust repository. Your task is to investigate and implement safe single-thread performance improvements for the periodic archive and related geometry hot paths, with the 64-chord Issue 21 fixture as the primary performance case.

This is a correctness-first optimization task. The current behavior fixes a real production failure. Do not trade away any part of that fix to improve a benchmark.

## Operating instructions

1. Read this entire assignment before changing code.
2. Inspect the current branch, current diff, relevant git history, and current tests. Do not assume line numbers or commit hashes below are still exact.
3. Work only in the supplied isolated worktree and on its current branch. Do not switch branches or modify another checkout.
4. Do not modify the configurator repository or configurator behavior. The work belongs in this Rust engine.
5. Use test-driven development for behavior changes. Add or tighten a test, verify that it fails for the intended reason, implement the smallest correct change, and verify it passes.
6. Profile and measure before choosing an optimization. Do not implement every idea in this document merely because it is listed.
7. Make one independently measurable optimization at a time. Verify correctness and structural counters before stacking another change.
8. Do not commit, push, open a pull request, publish a package, or edit an external issue unless the user explicitly authorizes that action in the session executing this prompt.
9. Never add credentials, access tokens, private URLs, or other secrets to the repository or reports.
10. When you have enough evidence to act, act. Do not stop for unnecessary confirmation, but stop and report if a correctness invariant cannot be preserved or a required test exposes an unresolved defect.

## Repository and correctness history

Issue 21 involved repeated round pieces sharing an `interchangeabilityKey`. The engine previously failed to settle a complete periodic archive for production desktop requests. The correctness fix is represented in history by the commit titled:

```text
fix: complete periodic round-family archives
```

The optimization branch also contains a commit titled:

```text
perf: add periodic work telemetry
```

The branch may have been rebased, so locate these commits by subject rather than relying on a fixed SHA.

The engine must support the declared interchangeability semantics for all of these representations:

- Native circles.
- Full circles represented by valid connected arc chains.
- Regular polygons represented by line chords.
- Regular polygons whose vertices were snapped to the canonical 0.001 mm grid.

Do not silently weaken, replace, or ignore the declared interchangeability key. Do not special-case the production fixture by identity, file name, vertex count, or coordinate values.

## Exact production fixtures and regression paths

The primary fixtures are:

```text
tests/fixtures/issue-21/repro-2circles.json
tests/fixtures/issue-21/G-2circles-lines.json
tests/fixtures/issue-21/A-exact-production.json
```

They are exercised end to end through the N-API desktop compatibility path in:

```text
crates/polygon-nesting-napi/tests/job.rs
```

Important test names are:

```text
issue_21_interchangeable_arc_circle_desktop_request_completes
issue_21_interchangeable_chord_circle_desktop_request_completes
issue_21_exact_production_desktop_request_completes
```

The core `Job` path also has this relevant regression in:

```text
crates/polygon-nesting-core/tests/job_service.rs
```

```text
compact_archive_completes_for_interchangeable_regular_polygon_copies
```

Do not replace these end-to-end tests with synthetic-only tests. Synthetic focused tests are useful for proving a local invariant, but the exact desktop fixtures remain required.

## Non-negotiable 64-gon semantics

The 64-chord fixture is a regular polygon, not a mathematical circle.

A valid native circle or validated full-circle arc chain has continuous rotational symmetry. A regular 64-gon made from line chords has discrete rotational symmetry of order 64. Its fundamental rotational period is 5.625 degrees. Orthogonal and half-edge-aligned orientations are genuinely distinct in the nesting problem.

The current verified contract for the release 64-gon characterization is:

- Exactly 2 retained transform representatives.
- Exactly 2 P1 `derive_cells` calls.
- Exactly 24 P2 `derive_cells` calls.
- Complete runtime coverage.
- Complete transform coverage.
- Complete pair coverage.
- Complete cell coverage.

These values are correctness constraints for this task. They are not profiling noise that may be reduced by dropping valid work.

Do not:

- Treat the regular 64-gon as continuously symmetric.
- Collapse the two retained transform representatives into one.
- Remove any of the 24 required P2 derivations.
- Apply arbitrary 5.625-degree finite-sheet quotienting.
- Use approximate rotational symmetry as proof for canonical pruning.
- Expand the existing safe orbit reduction beyond exact canonical-grid quarter-turn equivalence unless you first provide a rigorous proof, focused differential tests, and end-to-end evidence that finite-sheet behavior is unchanged.

The existing safe reduction relies on both transformed polygons proving exact canonical-grid quarter-turn symmetry. Preserve that boundary.

## Archive invariants that must remain unchanged

An optimization is rejected if it changes any of the following unless the user explicitly approves a separate behavior change:

- Canonical cell identities and ordered canonical cell keys.
- Collision-layout identities.
- Archive hashes.
- Candidate emission order.
- Member order.
- Transform order.
- Basis order.
- Source keys.
- Source kinds.
- Basis provenance.
- Source-survival participation and audit behavior.
- Rejection behavior.
- Continuation identities or continuation ordering.
- Final fixture placements or finite-sheet legality.
- Family selection and archive settlement.
- Transform, pair, cell, family, runtime, continuation, or source-survival coverage.
- JavaScript code-unit comparison semantics used by canonical identity code.

Hash maps and hash sets may be used for exact lookup. Their iteration order must never define public output, candidate order, provenance order, or canonical serialization. A hash must not replace exact equality unless collisions are resolved by comparing the full exact value.

Do not regenerate canonical vector expectations, snapshots, fixture outputs, or archive hashes merely to make a changed implementation pass. Any unexpected identity change must be investigated as a correctness failure.

## No time-dependent correctness shortcuts

Wall-clock measurements are benchmark evidence only.

Do not:

- Add elapsed-time thresholds to CI tests.
- Make tests pass or fail according to machine speed.
- Use a deadline to enumerate only part of a correctness-required candidate space.
- Use wall-clock-dependent partial enumeration as an optimization.
- Reduce production runtime budgets to hide expensive work.
- Mark incomplete work as complete.

The production coverage contract must remain intact under finite runtime budgets. Deterministic structural counters are preferred for regression assertions.

## Existing deterministic work telemetry

Read the current implementation in:

```text
crates/polygon-nesting-core/src/archive/periodic_cells.rs
crates/polygon-nesting-core/src/archive/periodic_family.rs
crates/polygon-nesting-core/tests/periodic_vectors.rs
```

The branch contains an opt-in `IntrinsicPeriodicWorkTelemetry` with integer counters including:

- `p1_derive_cells_calls`
- `p2_derive_cells_calls`
- `raw_p2_offsets`
- `nonnegative_p2_offsets`
- `duplicate_candidate_orbits`
- `p2_sheetless_legality_checks`
- `basis_candidates`
- `lattice_diagnosis_requests`
- `lattice_diagnosis_computations`
- `lattice_diagnosis_memo_hits`

Production uses `telemetry: None`. Focused tests opt in. Do not expose these internal counters through protocol schemas or general execution diagnostics without explicit approval.

The focused characterization test is:

```text
regular_polygon_periodic_work_is_deterministic_and_observer_only
```

It uses a small regular polygon in debug mode and the required 64-gon in release mode. It verifies that telemetry does not change ordered canonical keys or coverage and that repeated telemetry runs are deterministic.

The known pre-optimization release 64-gon structural baseline is:

- Retained transform representatives: 2.
- P1 derivations: 2.
- P2 derivations: 24.
- Every admitted raw P2 offset receives a sheetless legality check.
- Duplicate candidate orbits: 0 for this primary fixture.
- Lattice diagnosis computations equal requests.
- Lattice diagnosis memo hits: 0.

Re-run the characterization and record the actual current values before optimizing. If current code differs from this description, investigate git history and tests before proceeding.

### Telemetry observer concern

The current optional counters perform small increments inside loops that also contain finite wall-clock checks. Counting occurs after the relevant offset admission deadline check, and focused tests prove deterministic output under a no-deadline seam. However, enabled telemetry still performs additional work between later deadline checks. A run extremely close to a finite deadline could theoretically admit one fewer later candidate with telemetry enabled than with telemetry disabled.

Do not ignore this concern. Choose one of these defensible outcomes:

1. Keep telemetry strictly test-only and document why no finite-deadline production observer claim is being made.
2. Refactor counting to unconditional local owned counters with deferred optional publication after correctness decisions, then prove enabled and disabled runs remain equivalent.
3. Develop another design that demonstrably cannot influence finite-deadline admission.

Do not add a wall-clock-sensitive test that pretends to prove observer neutrality. Use deterministic control seams or a design-level proof backed by focused tests.

## Required investigation workflow

### Phase 1: Establish a fresh baseline

1. Confirm the worktree is clean and inspect the branch diff from `origin/main`.
2. Read the current implementation and relevant history before proposing changes.
3. Build release test binaries before timing so compilation is excluded from measured intervals.
4. Run the arc and 64-chord exact fixtures multiple times on the same machine.
5. Record all elapsed times and the median. Do not report only the fastest run.
6. Capture deterministic work telemetry for the same revision.
7. Use profiling appropriate for the available platform. Attribute cost to functions or stages rather than guessing from source shape.
8. Write a short baseline note before implementation that identifies the dominant measured costs and the first optimization selected.

The previous branch had historical measurements around 0.7 to 0.9 seconds for the arc fixture and around 20 to 23 seconds for the chord fixture. Those are context only. They are not a valid current baseline after rebases or code changes.

A prior post-rebase release N-API `--no-run` build was killed with exit code 137 while compiling. Do not claim a fresh post-0.1.7 baseline unless you successfully build and measure it. If memory pressure blocks release verification, report the exact command and failure, reduce build parallelism if appropriate, and continue only with evidence you can obtain without weakening tests.

### Phase 2: Select one optimization

Rank possible work by measured payoff, correctness risk, and testability. Prefer removing repeated exact work over changing the valid search space. State why the selected change should affect the observed profile or a deterministic structural counter.

### Phase 3: Implement with differential evidence

For each optimization stage:

1. Add or tighten focused tests first.
2. Verify the new test fails for the intended reason where the change adds behavior or a new structural guarantee.
3. Implement the smallest change.
4. Run focused identity, ordering, provenance, coverage, and end-to-end tests.
5. Compare deterministic counters before and after.
6. Rebuild before timing if code changed.
7. Repeat the same-machine release benchmark and report every run plus the median.
8. Keep the stage only if it provides a real structural or measured improvement without correctness changes.

Do not stack several speculative changes and then infer which one helped.

## Candidate optimization areas

These are investigation targets, not mandatory implementation tasks. Current profiling and differential evidence decide the order.

### 1. Convex NFP construction

Relevant files:

```text
crates/polygon-nesting-core/src/nfp_ifp/boundary_core.rs
crates/polygon-nesting-core/src/caches/nfp_cache_key.rs
crates/polygon-nesting-core/src/caches/store.rs
```

Production currently selects `NfpConstructionAlgorithm::VertexPairHull`. For two 64-vertex convex polygons, that path can create 4,096 pairwise Minkowski points per cache miss and then compute a convex hull.

The codebase already contains `NfpConstructionAlgorithm::LinearEdgeMerge`, which should usually behave linearly in the combined edge count for valid convex inputs. It is not automatically safe to make it the production default.

Before switching or routing inputs to it:

- Verify its input preconditions from current code.
- Differentially compare both algorithms across existing vectors and generated convex polygons.
- Include reversed winding, repeated or collinear vertices, snapped coordinates, translated inputs, different digit widths, and degenerate inputs accepted by current validation.
- Compare canonicalized boundaries, legality decisions, cache behavior, and all downstream archive identities.
- Preserve algorithm identity in cache keys. Never read a cached boundary produced by one algorithm under the identity of another.
- Retain a correct fallback if linear edge merge cannot prove its preconditions.

This area may have high payoff, but it also has broad blast radius. Do not change the production default based only on asymptotic complexity.

### 2. Prepared sheetless legality context

Relevant files:

```text
crates/polygon-nesting-core/src/archive/periodic_cells.rs
crates/polygon-nesting-core/src/validation/placement.rs
crates/polygon-nesting-core/src/validation/spatial_index.rs
```

Periodic enumeration calls sheetless legality repeatedly. The general path may repeatedly clone placements into new `Arc` values, translate polygons, compute bounds, prepare edge vectors, and validate overlap.

Investigate a periodic-archive-specific prepared context that can safely retain immutable geometry-derived data such as:

- Translated or origin-normalized polygons.
- Bounds.
- Prepared edge vectors.
- Other exact broad-phase data already proven independent of candidate provenance.

Do not assume the existing persistent spatial index is a free win. Inspect its exact `Arc` identity requirements and update costs. A new index that changes candidate order or legality semantics is unacceptable.

Differentially compare every sheetless result against the existing implementation for representative P1, P2, legal, touching, overlapping, arc, chord, mirrored, and translated cases.

### 3. Edge-contact enumeration

Relevant code is in `periodic_cells.rs`, including `derive_edge_contact_basis_candidates` and its helpers.

A 64 by 64 member pair can require 4,096 edge comparisons. Investigate exact pre-indexing or prefilters based on properties such as:

- Normalized direction.
- Parallelism class.
- Orientation sign.
- Edge-length feasibility.
- Bounding interval overlap.

Any prefilter must be mathematically exact for all accepted coordinates. Preserve the existing source and candidate order. If a lookup groups edges, emit results in the exact order the current nested scan would have produced. Hash iteration order is forbidden for emission.

Build a naive test oracle from the current implementation before replacing the scan. Compare complete ordered candidates, source keys, source points, source kinds, and provenance.

### 4. Per-derivation lattice diagnosis memoization

Relevant functions in `periodic_cells.rs` include `diagnose_lattice`, `derive_edge_contact_basis_candidates`, and final cell construction in `derive_cells`.

`diagnose_lattice` constructs and validates a 3 by 3 neighborhood. The same exact canonical basis may be diagnosed more than once through different source paths.

A safe memo should:

- Be scoped to one `derive_cells` invocation.
- Use an exact structured key equivalent to `(GridPoint, GridPoint)`.
- Reuse only the pure diagnostic result.
- Be shared by the diagnosis sites that can revisit the same basis.
- Preserve duplicate output candidates, source keys, provenance, ordering, rejection behavior, and source-survival participation.
- Never use formatted strings as the lookup key when exact grid points are available.

Add a focused fixture where multiple source kinds reach the same canonical basis. Compare the full ordered cells and provenance against an uncached oracle. Require nonzero memo hits in that focused fixture. Do not require memo hits in the primary 64-gon if its actual source graph has no duplicate diagnosis requests.

Also inspect member-only values currently recomputed inside per-basis loops, including total doubled member area and base-cell shape. Hoist only values that are provably independent of the basis and candidate source.

### 5. Cyclic canonicalization without materializing every rotation

Relevant functions include:

```text
canonical_cycle in crates/polygon-nesting-core/src/archive/periodic_cells.rs
canonical_ring and canonical_ring_direction in crates/polygon-nesting-core/src/canonical_grid/layout.rs
```

Current code materializes many complete forward and reverse cyclic strings. A 64-vertex ring can produce 128 full rotation strings.

A possible crate-private helper is:

```rust
pub(crate) fn canonical_bidirectional_cyclic_key(tokens: &[String]) -> String
```

The helper must compare complete virtual semicolon-joined byte streams and materialize only the winner. Preserve current JavaScript code-unit ordering exactly.

Token-by-token comparison is not sufficient. Prefix hazards such as `1,2` versus `1,20` can order differently once separators and following bytes are considered.

Create a test-only naive oracle equivalent to the current implementation. Differentially test:

- Empty and singleton rings.
- Both windings and every cyclic origin.
- Repeated tokens.
- Negative and zero coordinates.
- Different digit widths.
- Prefix hazards.
- Large `BigInt` coordinates.
- Folded negative zero if a call site can produce it.
- Deterministically generated rings.

Render each coordinate token once per call site and serialize only the winning candidate. Existing canonical layout and periodic vector expectations must remain byte-for-byte unchanged.

### 6. NFP cache hit overhead and prepared identities

Inspect:

```text
crates/polygon-nesting-core/src/caches/nfp_cache_key.rs
crates/polygon-nesting-core/src/caches/store.rs
```

A cache hit may still perform input validation, string-heavy polygon digest construction, full key construction, cached-output cloning, output validation, translation, and boundary canonicalization.

Investigate computing exact geometry identities once for prepared immutable geometry and reusing them. Hashing is useful only when it avoids repeated formatting and serialization. Formatting the same large string and then hashing it is not a meaningful optimization.

Preserve exact cache partitioning, algorithm identity, tolerance-sensitive inputs, and output validation guarantees. Do not introduce a hash-only identity without collision checking.

### 7. Structured internal lookup keys and allocation cleanup

Prefer exact structured keys for internal lookup, for example:

```rust
#[derive(Hash, Eq, PartialEq)]
struct BasisKey {
    v1: GridPoint,
    v2: GridPoint,
}
```

Potential cleanup targets include:

- Unnecessary boundary clones.
- Repeated `BigInt` formatting.
- Formatting strings inside sorting comparators.
- Cloning full cell collections before deduplication.
- Recomputing member-only measurements inside basis loops.

Treat these as profile-guided changes. Avoid broad refactors that make correctness review harder or add abstractions with no measured benefit.

### 8. Pair-orbit handling

The current primary 64-gon telemetry reports zero duplicate candidate orbits, so orbit deduplication is not expected to be the dominant optimization for that fixture.

If another measured workload shows duplicate exact quarter-turn orbits, an optimization may prepare per-transform-pair exact invariants once and skip duplicate representatives before expensive legality checks. Preserve sorted offset order, nonnegative-bounds admission semantics, and exact quarter-turn proof requirements.

Do not add arbitrary-angle orbit handling. Do not optimize this path merely to make a counter nonzero.

## Required tests and verification

All Cargo commands in this repository should run through the Nix development environment unless the current environment already provides the exact project toolchain. Prefer the documented form:

```sh
nix develop -c cargo ...
```

Start with focused checks. Adapt exact filters only after confirming current test names.

```sh
nix develop -c cargo fmt --check
nix develop -c cargo test --locked -p polygon-nesting-core --test periodic_vectors regular_polygon_periodic_work_is_deterministic_and_observer_only -- --exact
nix develop -c cargo test --locked -p polygon-nesting-core --test periodic_vectors
```

Run the release 64-gon structural characterization:

```sh
nix develop -c cargo test --release --locked -p polygon-nesting-core --test periodic_vectors regular_polygon_periodic_work_is_deterministic_and_observer_only -- --exact
```

Run canonical identity tests affected by the implementation, including the canonical layout vectors if cyclic or layout identity code changes:

```sh
nix develop -c cargo test --release --locked -p polygon-nesting-core --test canonical_layout_vectors
```

Run the core `Job` regression:

```sh
nix develop -c cargo test --release --locked -p polygon-nesting-core --test job_service compact_archive_completes_for_interchangeable_regular_polygon_copies -- --exact
```

Run the exact Issue 21 N-API regressions:

```sh
nix develop -c cargo test --release --locked -p polygon-nesting-napi --test job issue_21_
```

Run any additional focused suites covering the implementation area. Before completion, run repository-wide release validation if machine resources permit:

```sh
nix develop -c cargo clippy --workspace --all-targets --release --locked -- -D warnings
nix develop -c cargo test --workspace --release --locked
```

If a repository-wide command cannot complete because of resource limits, report the exact command, exit code, and last relevant output. Never describe an unrun or killed command as passing.

Run IDE or language-server diagnostics on every modified Rust file before reporting completion. Run `git diff --check` and inspect the complete diff from the branch base.

## Benchmark procedure

Build first:

```sh
nix develop -c cargo test --release --locked -p polygon-nesting-napi --test job --no-run
```

Then measure the already-built focused test binary if practical. If invoking through Cargo, confirm no compilation occurs inside the timed interval.

Measure both cases at least five times on the same machine and under comparable load:

```sh
/usr/bin/time -p nix develop -c cargo test --release --locked -p polygon-nesting-napi --test job issue_21_interchangeable_arc_circle_desktop_request_completes -- --exact
/usr/bin/time -p nix develop -c cargo test --release --locked -p polygon-nesting-napi --test job issue_21_interchangeable_chord_circle_desktop_request_completes -- --exact
```

Report:

- Machine and build mode.
- Exact revision.
- Exact commands.
- Every arc timing.
- Arc median.
- Every chord timing.
- Chord median.
- Relevant deterministic counters before and after.
- Profile evidence linking the change to the reduced cost.
- Any variance or resource limitation.

Never add these elapsed-time values as test assertions.

## Acceptance criteria

The work is acceptable only if all of these are true:

1. The regular 64-gon remains a discrete polygon and retains exactly two transform representatives.
2. The focused release characterization still performs exactly 2 P1 and 24 P2 derivations.
3. Transform, pair, cell, family, runtime, continuation, and source-survival coverage remain complete where required by existing tests and fixtures.
4. Canonical keys, collision-layout identities, archive hashes, candidate ordering, provenance, source keys, source kinds, continuation identities, and fixture placements remain unchanged.
5. Arc-circle, chord-circle, exact production, and core `Job` Issue 21 regressions pass.
6. The selected optimization reduces a measured dominant cost or a deterministic structural counter without removing correctness-required work.
7. Release timing is reported as evidence only, with no wall-clock CI gate.
8. No configurator code or behavior changes.
9. No canonical vectors or expected outputs are regenerated to hide differences.
10. No public protocol change is introduced for internal telemetry.
11. The telemetry observer concern is either resolved by design and tests or clearly retained and reported without an unsupported neutrality claim.
12. The final diff contains only justified optimization, test, and narrowly related documentation changes.

## Expected final report

Return a concise but complete engineering report with these sections:

### Summary

- What was measured.
- What optimization was selected.
- Why it was selected.
- What changed.

### Correctness evidence

- Exact tests run and outcomes.
- Confirmation of 2 retained representatives, 2 P1 derivations, and 24 P2 derivations.
- Confirmation that identities, ordering, provenance, coverage, and finite-sheet behavior remained unchanged.
- Red-green evidence for new tests.

### Performance evidence

- Baseline profile and counters.
- Post-change profile and counters.
- All timing samples and medians for arc and chord fixtures.
- Explanation of why the result is attributable to the change.

### Files changed

- Each modified file and its responsibility.

### Limitations and remaining opportunities

- Any verification blocked by machine resources.
- Any unresolved telemetry observer concern.
- Any promising optimization intentionally deferred, with evidence for deferral.

### Repository state

- Current branch and revision.
- Whether the worktree is clean.
- Explicit confirmation that you did not commit, push, publish, or alter external systems unless separately authorized.

Optimize the implementation, not the correctness contract.