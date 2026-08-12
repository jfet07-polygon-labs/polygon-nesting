# Deterministic parallelization of the capacity beam evaluation — design

Follow-up to the canonical-matrix work (`docs/canonical-matrix-performance.md`,
PR #24): after ten local, byte-identical work reductions, the wall-clock
ceiling on large mixed workloads is structural. The per-thread profile of
mixed-61-2000×2700 shows the coordinator thread holding ≈ 42 % of all CPU
samples ≈ the entire wall time (15 rayon workers average ≈ 4 % each), and
the capacity cold search — whose beam loop evaluates candidates strictly
serially per entry × transform (`capacity/search.rs` beam loop) — is the
largest block on that critical path (≈ 17 % of coordinator time) that
parallelism can remove without inventing new algorithmic structure: the
strict decoder already evaluates its candidates on the pool and replays
outcomes in deterministic order.

## Target

Evaluate the cap-admitted prefix of capacity beam candidates on the job-owned
rayon pool, then replay outcomes serially in source-ordinal order. This uses
the same pure-compute/ordered-replay principle already proved by the strict
decoder without adding candidate-level checkpoint observations that the
serial capacity loop never had.

## Invariants that must be preserved bit-for-bit

1. **Outputs**: every emitted byte — placements, scores, canonical keys,
   checkpoint `integrityHash` preimages — identical to the serial loop for
   every thread count (including 1) and every repeat. Gate:
   `tests/thread_equality.rs` (Job-level canonical semantic bytes at
   1/2/4/8 workers, mixed-61 subsets of 2/4/8/20 pieces), the 18-row
   canonical quality golden, `capacity_search_vectors` (checkpoint
   preimages), `coordinator_vectors` (TS oracle).
2. **Cache access**: the geometry cache is `&mut` on the coordinator; no
   worker may touch it. Pattern: the same precompute-then-publish pre-pass
   used for pairwise NFPs (pure compute on workers, serial publication in
   first-encounter order), then the serial replay performs the real
   resolver calls as warm hits.
3. **Cancellation/checkpoint observation ordinals**: the existing capacity
   checkpoint before candidate generation stays at the same site. Worker-side
   candidate evaluation is pure and does not add, move, or remove control
   observations.
4. **Checkpoint-visible counters** (fanout traces, per-phase timings that
   feed integrity-hash preimages): increments must keep their serial
   pattern — count-bearing steps stay in the serial replay, never in
   worker closures (lesson pinned by the Stage-4 retention-cache work).
5. **Mid-loop resume**: checkpoint resume re-enters the loop at a piece
   index; admitted-prefix dispatch must not change what a resumed run computes.

## Plan of record

1. **Stage 0 — harness first**: extend `thread_equality.rs` coverage to a
   capacity-heavy configuration if the current fixtures under-exercise the
   cold-search path (verify by instrumentation before assuming); record
   the baseline matrix timings on this branch.
2. **Stage 1 — purity split**: refactor `evaluate_candidate` (and the
   contact-score step) into a pure compute half (no cache, no counters,
   no control) plus a serial effects half; prove by types (`Fn` bounds) —
   behavior-neutral commit, all suites green.
3. **Stage 2 — cache pre-pass**: extend the NFP precompute pre-pass so the
   pure half finds only warm hits (measure first: it may already cover
   everything the evaluation path resolves).
4. **Stage 3 — admitted-prefix dispatch**: derive the deterministic prefix
   allowed by both evaluation caps, evaluate that whole prefix through the job
   pool, and keep every observable effect in the serial replay. Thread-equality,
   golden, vectors after each stage; wall/user benchmarks 5× per row.
5. **Honest exit rule**: if measurement shows the parallel evaluation not
   paying (contention, small per-candidate cost after PR #24's
   reductions), record the numbers and stop — the harness and purity
   split remain independently valuable.

## Delivered (updates)

- **Candidate evaluation dispatch** (`73514c6`): pure triple on the pool,
  serial replay with every effect at unchanged sites/ordinals. Gates all
  green (thread-equality 6/6, hash preimages 3/3, golden 18/18). Wall
  median ~14.4 → 14.17 s.
- **Survivor topology precompute** (this commit): per-depth pure topology
  values computed on the pool and seeded; measure() consumes seeds on
  memo misses with count/clock sites unchanged. Wall median
  14.17 → 13.16 s. **Cumulative on this branch: ≈ −8.6 % wall** vs the
  post-#24 baseline.
- Review-confirmed diagnostic trade: wall-clock `contactMeasurementMs` /
  `topologyMeasurementMs` now record replay bookkeeping (the injected
  deterministic clock — the parity gate — sees identical sequences).
- Remaining from the plan: successor-construction pure parts via the same
  replay pattern; early-abort waste on erroring prefixes (nit).

## Pre-successor-key clean-machine numbers (load < 2, 5×/3× runs)

mixed-61-2000x2700-compact, after survivor-topology precompute and before the
successor-key memoization reported below:

| pool workers | wall (median) | user |
| --- | --- | --- |
| 15 (default = available−1) | **13.16 s** | 36.2 s |
| 12 (`MIN_PLANE_IRREGULAR_NATIVE_THREADS=12`) | **12.88 s** | 31.7 s |
| 16 | 13.29 s | 37.7 s |

- At that snapshot, cumulative wall at the default was post-#24 ≈ 14.4 s →
  **13.16 s (−8.6 %)**;
  campaign total 16.03 s → 13.16 s (**−17.9 %**).
- On this 16-CPU machine the scouted "coordinator occupies a pool worker, so
  available−1 wastes a core" hypothesis is **refuted by measurement**: 16
  workers is worse than 15, and 12 beats both (less idle steal-spin, ample
  parallelism). The wide-host default therefore stays untouched, while the
  production-shape measurements below justify using every CPU on hosts with
  at most 8 CPUs. The existing `MIN_PLANE_IRREGULAR_NATIVE_THREADS` knob still
  allows workload-specific tuning.

## Production-shape measurements (pinned CPUs, quiet machine)

| shape | workers | wall (median) |
| --- | --- | --- |
| 2 CPUs pinned | 1 (old default) | 18.80 s |
| 2 CPUs pinned | 2 | **15.96 s (−15 %)** |
| 8 CPUs pinned (production container) | 7 (old default) | 12.99 s |
| 8 CPUs pinned | **8 (new default)** | **12.78 s (−1.6 %)** |
| 8 CPUs pinned | 6 | 13.09 s |
| 16 CPUs free | 15 (default, kept) | 13.05 s |
| 16 CPUs free | 12 | 12.88 s |
| 16 CPUs free | 16 | 13.29 s |

The automatic default now uses every CPU on hosts up to 8 cores (the
coordinator executes on the pool; the one-CPU reservation only starved
small hosts) and keeps `available − 1` above that. Also landed: the
successor-build anchored keys reuse the per-parent memo per entry
(16-core clean median 13.16 → 13.05 s).
