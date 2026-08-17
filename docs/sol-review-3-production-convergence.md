# Sol adversarial review 3: production convergence toward 160/150 mm in 10 s

Same-session continuation of the go-to-production consultation (docs/production-roadmap-sol-consultation.md), model gpt-5.6-sol at max reasoning effort, read-only sandbox, reviewing HEAD 4c35855 on 2026-08-17. Verbatim final message follows.

Bottom line: the landed performance work is real, but the convergence claim is not yet earned. The important update is that m22’s proxy substrate is now essentially Sparrow-speed; the remaining gap is basin generation, exact-heavy deep operators, and orchestration—not the relaxed candidate loop.

I inspected HEAD `4c35855`, the ledger tail, implementation, certification drivers/artifacts, and replayed the existing release artifact read-only. No files were changed.

## Measured correction to the premise

My m22 profiling replay counted 13,576,116 candidate queries and 122,018 effective piece moves. Combining those unchanged counters with the paired unprofiled 3.597-second median gives:

| Metric | Current m22 | Sparrow |
|---|---:|---:|
| Candidate evaluations/s | 3.775M | 3.742M |
| Approx. ns/evaluation | 265ns | 270ns |
| Evaluations/effective move | 111 | 263 |
| Effective moves/s | 33.9K | 14.2K |

The definitions claim direct correspondence in [profiling.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/profiling.rs:40), although I would still audit Sparrow’s external “move” definition before marketing the comparison. The 3.597-second wall result is pinned in the [hot-path ledger](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1955).

In the profiled run:

- `pairCollide`: 29.5% of aggregate leaf CPU.
- `pairPressure`: 23.1%.
- Exact overlap queries: 9.3%.
- Publication validation: 0.4%, 16 attempts.

Therefore:

- Moving publication validation is no longer a meaningful m22 optimization.
- `pairPressure` is worth optimizing, but cannot explain a 10–100× product gap.
- The old “substrate is 3–4 orders too slow” statement is now false for m22.
- Throughput is sufficient to execute roughly 38M m22 evaluations or 339K effective moves in ten seconds. Quality per unit work is the current problem.

## Findings, ranked by severity

### 1. Critical: there is still no evidenced 160-in-10 path from scratch

The record and production lines have not converged. PR5 proves that a deep mode result can become the engine result; it does not prove discovery of that deep basin.

The current from-scratch pin is 164.037568 after 24 more arms, and the finer orientation rungs were not causal even for that 0.001mm change [summary.json](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/summary.json:355). Earlier evidence explicitly says the global solver primarily deepens an already-deep basin and moved the from-scratch line only to 164.096 [next-generation-engine-plan.md](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1300).

Worse, the proven from-scratch lineage required:

- A mode-20-created basin substantially worse than the public incumbent.
- Multiple alternation waves.
- Sibling retention.
- Crossover; the pure locks did not cross 168 without it [ledger](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:914).
- Later compression/global legalization.

No current single-process run reproduces that causal chain remotely near ten seconds. Existing-mode scheduling alone cannot honestly be called a credible 160-in-10 plan.

Also, 150-in-10 requires a new quality mechanism: the absolute record itself is still 159.07876. PR7 cannot pass the Sparrow 150.165 envelope by orchestration alone.

### 2. Critical: PR5 is safe publication logic but wrong as the sole anytime state model

The safety semantics are good:

- It requires complete cardinality.
- Compares raw depth.
- Keeps ties on legacy.
- Re-runs the composite validator.
- Invalid, incomplete, or deeper diagnostics cannot replace the result.

That logic is in [adopt_published_layout](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3636).

The production fragility is that adoption only retains an immediately better public result. Mode 20’s 206.869 candidate correctly loses to the 179.756 protected incumbent—but the documented from-scratch lineage begins from precisely this class of worse, structurally different constructor basin. If PR7 feeds only `GeneralRelaxedOutcome.result` forward, it destroys the only evidenced route to 164.

The coordinator needs two separate objects:

- `PublishedIncumbent`: always dual-gate valid and best raw depth.
- `SearchArchive`: typed exact-valid or deliberately infeasible parents retained for future expected value and topology diversity, even when presently deeper.

Additional PR5 fragilities:

- It reconstructs candidates from a diagnostics DTO rather than receiving a typed operator outcome [published_mode_placements](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3593). A future mode can still be forgotten by failing to populate that field correctly.
- Every adoption rejection silently returns legacy; production telemetry cannot distinguish incomplete, invalid, envelope-only rejection, or non-improvement.
- The claimed “single exit” is not literal: direct successful returns remain at [general_relaxed.rs:3283](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3283). They precede persistent-mode dispatch today, so this is not a current lost-result bug, but the asserted invariant is fragile.
- The adopted gate is not merely “real-request contract validity.” `validate_and_measure_placements` also requires the stricter search envelope and can reject a raw-contract-valid layout [general_fast.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:3476). Requiring both flags is conservative and consistent with current policy, but the distinction should be visible in rejection telemetry.

### 3. High: the literal certification claim is false under the engine comparator

The certification driver defines improvement as:

```python
published < RAW - 1e-12
```

at [certify_full.py:54](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/drivers/certify_full.py:54).

At depth 159, `1e-12` is about 35 `f64` ULPs. The retained certificate contains five exact-valid, contract-valid publications at `159.07876040364792`, exactly one ULP below the declared `159.07876040364795`, while reporting `below:false`. Four reproduce the incumbent fingerprint; one has a distinct fingerprint [cert.json:399](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/cert.json:399).

PR5’s actual comparator is strict raw `<` via the `>=` rejection at [general_relaxed.rs:3656](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3656). Consequently, the certificate and engine do not implement the same improvement policy.

Resolution:

- If one ULP is intentionally a tie, use the exact scoped ULP comparator everywhere.
- If publication is strict raw depth, advance the pin by one ULP and recertify.
- Do not use a decimal absolute epsilon.

More generally, “certified fixpoint” should be renamed “enumerated 40-arm campaign fixpoint.” The battery covered the listed modes and constants, took 428.8 seconds [cert.json](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/cert.json:99), and says nothing about unenumerated angles, centers, budgets, operators, or instances. The legality/replay evidence is strong; the unqualified optimality language is not.

### 4. High: the current kernel seam is not the production Jagua seam

The code is fairly honest about this limitation, but the “type-system property” language overclaims.

Problems:

- `ExplorationKernel` itself contains both proxy and exact methods [kernel/mod.rs:182](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/kernel/mod.rs:182). Current exact call sites name `LEGACY`, but the type system does not prevent future generic code from calling `K::exact_pair_overlaps`.
- `LaneSearch` fixes `K::Shape = OrientedSurrogate`, so `JaguaKernel<Shape = JaguaShape>` cannot be substituted into it [general_relaxed.rs:3033](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3033).
- `PosedShape` supports only translation of a pre-oriented shape. Continuous-angle operators require prebuilding every angle variant.
- The Jagua skeleton creates a new `Layout`, container, placed item, and moving scratch per pair question [jagua.rs:274](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/kernel/jagua.rs:274). That is a parity scaffold, not a nanosecond candidate engine.
- Pair collision and pair pressure are separate calls, forcing duplicate traversal.
- `KernelProbes` are backend-defined “own terms,” so a Jagua “one SAT” charge is not economically comparable with the legacy collider’s SAT count. Portfolio work quotas cannot be backend-neutral under that definition.

PR6 needs a seam revision around a lane-owned dynamic layout:

```text
query_moved_into(piece, pose, cutoff, scratch)
    -> Pruned | Complete<MovedRowDelta>
```

The proxy trait should contain no exact methods. Exact collision construction and publication should remain separate named legacy services or require a private publication-authority token.

### 5. High: the row tracker is not merely numerically noisy; it is structurally wrong

The audit found 534 structural disagreements in 121,463 accepted m22 moves—0.44%—plus widespread magnitude-order differences [ledger](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1595).

The ledger lists two suspects:

- Revert installs a row through `tracked_piece_score` without remeasurement.
- Candidate structure comes from the hazard index while full scoring uses the surrogate collider.

It has not causally attributed the 534 disagreements between them. Until that is done:

- Do not inherit trackers across sweeps.
- Do not make Jagua’s dynamic rows authoritative.
- On reject/revert, restore an immutable snapshot of the old row.
- Define one canonical `measure_row(lower_id, higher_id, poses)` used by both full and incremental scoring. A hazard may shortlist; it may not silently define different row membership.

This is a correctness dependency for PR6, but not a direct performance prize: full rescoring is now only 0.70% of m22 leaf time [ledger](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1668). Selling the oracle work as a speedup would be another overclaim.

### 6. High: the orientation mechanism is causal on one basin, not general or scale-free

The record attribution is convincing. The generality language is not.

The implementation is a nine-element discrete ladder, not continuous optimization [general_persistent_vacancy.rs:267](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_persistent_vacancy.rs:267). Its “scale-free” test justifies the floor using a hard-coded 100mm radius [general_persistent_vacancy.rs:11562](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_persistent_vacancy.rs:11562). Angular locality and pose-grid expressibility both depend on the actual piece radius.

For arbitrary DXF, derive angular rungs from displacement rungs and each piece’s effective radius, approximately `δθ = δx/r`, then quantize to the angle grid. A 0.0032° floor is tiny for one part and enormous for another.

Other limits:

- Each variant builds exact collision geometry before candidate search [general_persistent_vacancy.rs:1472](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_persistent_vacancy.rs:1472).
- Mode 32 produced no record publications; mode 33 required joint endpoint ejection.
- The finer rungs did not cause the from-scratch improvement.
- The total record gain was 0.003877mm, irrelevant to the 4mm from-scratch gap.
- Accepted rungs piling onto the new floor means the search is truncated at a boundary, not that the correct general floor has been found.

I would retain m33 as a targeted tail repair, not a default production phase.

### 7. Medium: the hypot decision is appropriately conservative, but its stated blast radius is wrong

Default-off is correct. Five agreeing streams are correlated Mixed-61 streams and do not cover scales, near-ties, topology, workers, platforms, or arbitrary DXF.

The claim that these calls “cannot reach a published placement” is false causally: they alter ranking, accepted moves, and therefore which placement reaches publication. The source comment itself correctly acknowledges this trajectory risk [general_relaxed.rs:2270](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:2270).

For the new proxy profile, I would test something stronger than naive hypot replacement:

- Compare squared distance to squared reach first.
- Compute `sqrt` only for actually overlapping pole pairs whose magnitude is needed.
- Skip pressure entirely when collision count/weighted feasibility already decides the candidate.
- Measure winner/runner-up score margins in ULPs; whole-stream equality is a weak near-tie test.
- Prove overflow/underflow safety over admitted coordinate bounds.

## The shortest plausible ten-second portfolio

This is the schedule I would build after PR6. It is not achievable with current operator costs.

| Budget | Work |
|---|---|
| 0–1.9s | Protected mode-0 result and shared preprocessing |
| 1.9–4.0s | One or two fast mode-20-derived basin constructors; retain them even if deeper |
| 4.0–6.8s | m22 work quanta across the best structurally distinct archive states |
| 6.8–7.4s | One mode-23 crossover only if two plateau basins exist |
| 7.4–9.6s | Fused short compression → m31 legalization; m22 micro-descent after successful publication |
| 9.6–10.0s | Publication queue drain, validation, serialization |

Mechanism calls:

- **m20:** Required in mechanism, but the current 26.562-second implementation cannot run. Its worse immediate depth must not disqualify its basin.
- **m22:** Core production descent. It already has sufficient throughput.
- **m23:** Conditional but currently evidence-required: crossover was causal in crossing the from-scratch 169.x locks.
- **m26:** Only one or two state-derived rungs. Never the full certification ladder.
- **m31:** Production-worthy only as the legalizer for a compressed/perturbed frontier. Standalone m30/m31 probes on clean fixpoints are diagnostics.
- **m28/29/32:** Cut from production.
- **m33:** Cut from the initial 160 path. Invoke only when a post-legalization residue is one or two translation-inseparable components involving a depth-setting endpoint.
- **m30:** Replay/residue diagnostic only.

Current economics explain the rewrite requirement:

- m20 is 26.562s. Even making its 57.5% `vacancyProxyRank` phase free leaves about 11.3s by Amdahl’s law. It needs at least roughly 13× overall to occupy a two-second slice.
- `vacancyProxyRank` itself rasterizes the whole strip, allocates three buffers, and performs cell-by-cell point-in-polygon scans against active collisions [general_persistent_vacancy.rs:5474](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_persistent_vacancy.rs:5474). Replace it with reusable/incremental occupancy counts and a bit-grid flood fill; do not merely tune the current loops.
- Current m26 certificate arms range from 15.2 to 97.8 seconds [cert.log](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/cert.log:72). After subtracting the roughly 2.8-second process floor, that is still approximately 12–95 seconds of operator work. A 0.5–1.0-second rung requires roughly 12–190×.
- m31 is near the repeated-process floor—2.9s versus 2.8s for a replay [cert.log](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/cert.log:14). Its problem is input-state suitability, not primary runtime.
- m33 ranges from near-floor for a small residue to roughly two seconds of incremental work for broad residues [cert.log](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/cert.log:19). It can fit only when tightly triggered.

The iteration target should therefore be:

- Preserve at least ~3.5–4M proxy candidates/s and ≥25K effective moves/s.
- Stay around ≤150 candidate queries/effective move for m22-like work.
- Make exact finalist rows a tiny survival fraction, not 10–40% of proposals.
- Keep all optimizer-internal exact geometry below roughly 5% of the ten-second budget.
- Measure quality as depth-versus-work, not total moves.

## Priority under the performance mandate

| Order | Work | Effort | Call |
|---:|---|:---:|---|
| 0 | Fix certification comparator; add time-to-quality trace | S | Immediate evidence correctness |
| 1 | Row-ownership oracle | M | First implementation slice because PR6 otherwise inherits an invalid state model; direct speed benefit is negligible |
| 2 | PR6 seam v2 and deep-operator port | L | Largest effect; start with m20 basin generation and fused m26→m31, then targeted m33 |
| 3 | m20 `vacancyProxyRank` redesign | M | Treat as part of PR6; incremental occupancy/bit-grid, reusable buffers, scale-derived resolution |
| 4 | PR7 full coordinator | L | Thin trace harness now; full typed archive/coordinator after one ported operator has real economics |
| 5 | `pairPressure` pole loop | M | Likely a 10–15% m22 wall win, not the convergence breakthrough |
| 6 | Hypot corpus validation | S/M | Useful ~0.4–0.5s possibility, but lowest causal value for reaching 160 |

PR6’s gate must measure descendant quality under a fixed downstream work budget. Immediate constructor depth is an invalid quality proxy—the ledger repeatedly proves that worse constructors create better eventual basins.

## What I would cut

- Full m26 ladders, seed grids, controls, and certification arms from production.
- Modes 28, 29, 32, and standalone 30 from the anytime portfolio.
- Unconditional m33 and further fixed-angle floor tuning before scale-derived rungs.
- Sweep-rescore elimination as a performance project; it is a correctness task worth under 1% today.
- The current Jagua pair-layout skeleton as a production target.
- Any requirement that the new proxy trajectory remain bit-identical to legacy. Protected legacy must remain bit-identical; a new profile needs per-seed determinism, exact-valid publication, and quality/time gates.
- Any 160-in-10 claim based on a pinned parent, or 150-in-10 claim while the absolute record remains above 159.
- “Arbitrary DXF” without qualification: relaxed search still explicitly falls back for hole topology [general_relaxed.rs:3173](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3173).

## First measurement next session

Build one event-driven, one-process, from-request-only **quality frontier trace** before doing more optimization.

Every exact-valid candidate—not only a new public incumbent—should record:

```text
elapsed_monotonic
work_ordinal
operator / seed / parent fingerprint
raw depth
public-incumbent or archived-basin
candidate queries / effective moves
proxy survivors
exact finalist rows / Clipper builds
publication attempts
```

Plot both:

- Best published depth versus time.
- Structurally diverse archived basins versus time, with their eventual descendant depth under a fixed downstream work budget.

That is the missing measurement. Current evidence gives whole-arm totals from selected parents, not time-to-first-value, marginal Δmm/ms, or the cost of the actual from-scratch ancestry inside one process. Until that curve exists, a ten-second portfolio allocation is informed engineering—but still storytelling.
