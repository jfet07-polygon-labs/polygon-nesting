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

Evaluate capacity beam candidates on the job-owned rayon pool in bounded
chunks, then replay outcomes serially in source-ordinal order — the exact
compute-then-replay contract `parallel::for_each_chunked_outcome` and
`strict_decoder::replay_admitted_scoring_inputs` already implement and
prove elsewhere in this crate.

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
3. **Cancellation/checkpoint observation ordinals**: the serial loop
   observes checkpoints at exact per-candidate ordinals; the chunked form
   must reproduce those observation ordinals exactly
   (`for_each_chunked_outcome` encodes precisely this contract).
4. **Checkpoint-visible counters** (fanout traces, per-phase timings that
   feed integrity-hash preimages): increments must keep their serial
   pattern — count-bearing steps stay in the serial replay, never in
   worker closures (lesson pinned by the Stage-4 retention-cache work).
5. **Mid-loop resume**: checkpoint resume re-enters the loop at a piece
   index; chunking must not change what a resumed run computes.

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
4. **Stage 3 — chunked dispatch**: evaluate candidates through
   `for_each_chunked_outcome` (chunk size = the serial loop's historical
   checkpoint stride), serial replay owning every effect. Thread-equality,
   golden, vectors after each stage; wall/user benchmarks 5× per row.
5. **Honest exit rule**: if measurement shows the parallel evaluation not
   paying (contention, small per-candidate cost after PR #24's
   reductions), record the numbers and stop — the harness and purity
   split remain independently valuable.
