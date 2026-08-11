# Canonical matrix performance investigation

Working notes and evidence for single-thread-work reduction on the
canonical-matrix workload (Triangle-20, Mixed-61, Shapes-17 across the
2000×2700 / 600×400 / 300×300 sheets, Compact and Short-Side profiles),
following the Issue-21 periodic archive work
(`docs/issue-21-periodic-performance.md`). Hard gate for every change: the
canonical quality golden (`tests/fixtures/canonical-quality-golden.json`)
must stay byte-identical across all 18 rows — request fingerprints,
placement counts, layout fingerprints, and score metrics.

## Baseline

- Machine: Linux x86_64, 16 hardware threads, 62 GiB RAM (same machine for
  every number here; release CLI, project toolchain rustc 1.95.0 via
  `nix develop`); branch base `main` @ `322aa4c` (version 0.1.9).
- Per-row driver identical to `scripts/run-current-canonical-matrix.mjs`:
  adapter output fed to `polygon-nesting run` with
  `diagnosticTraceMode: "full"`.

Single-run sweep of all 18 rows (elapsed seconds):

```text
16.24 mixed-61-2000x2700-short-side     2.12 triangle-20-300x300-short-side
15.80 mixed-61-2000x2700-compact        2.06 triangle-20-300x300-compact
 2.66 shapes-17-2000x2700-short-side    1.95 triangle-20-600x400-short-side
 2.47 shapes-17-600x400-short-side      1.95 triangle-20-2000x2700-short-side
 2.26 shapes-17-600x400-compact         1.88 triangle-20-2000x2700-compact
 2.24 shapes-17-2000x2700-compact       1.86 triangle-20-600x400-compact
 1.59 mixed-61-600x400-short-side       0.40 shapes-17-300x300-short-side
 1.44 mixed-61-600x400-compact          0.39 shapes-17-300x300-compact
 0.31 mixed-61-300x300-short-side       0.31 mixed-61-300x300-compact
```

The two mixed-61-2000×2700 rows dominate (half the whole matrix runtime).
For mixed-61-2000x2700-compact: `real ≈ 16.0 s`, `user ≈ 39.5 s`,
`sys ≈ 5.9 s` — 2.5× effective core use on 16 cores.

### Profile findings

`perf record --call-graph fp -e cpu-clock:u` (frame-pointer scratch build):

- Whole-process view: `score_candidate` 27.6 % cumulative — inside it the
  beam-state canonical keys (`canonical_collision_polygon_key` 12.1 %,
  `canonical_ring_key` 11.3 %, `canonical_point_key` 7.2 %); rayon/
  crossbeam scheduling ≈ 23 % flat; ryu `format64` 6.4 %; allocator
  cluster ≈ 14 %.
- Instrumented counts (bit-repeatable): 4.94 M `canonical_ring_key` calls
  per mixed-61-2000×2700 run, **93.5 % from
  `bottom_left_anchored_canonical_occupied_geometry_key`** re-rendering
  every placed piece's key per scored candidate; ≈ 47 M `canonical_number`
  calls; 8.47 M entry-key `String` clones (≈ 1.36 GB).
- **Per-thread view (the decisive one)**: the coordinator thread holds
  41.6 % of all CPU samples ≈ 15 s ≈ the entire wall time — the wall
  critical path is the serial coordinator, and worker-side CPU (idle
  steal-spin included) inflates flat scheduling shares without being on
  the critical path. Coordinator breakdown: strict state construction
  18.5 %, capacity cold search 17.2 % (retention topology measure 5.2 %,
  `compare_topology_metric` 5.3 %, retain 5.0 %), candidate generation
  12.7 %, periodic portfolio 14.7 %, `measure_canonical_layout_topology_exact`
  6.5 %.

## Stages

### Stage 1 (kept) — memoized anchor-translated parent keys in strict scoring

Per-parent-state memo (exact anchor-translation bit pattern → the parent
pieces' sorted anchored entry keys) threaded through both scoring modes;
each candidate renders only its own piece's key and merges it at its
`partition_point` upper bound, concatenated borrow-only via the new
`canonical_entry_list_key_parts`. Equal keys are identical strings, so the
sorted-multiset concatenation is order-invariant among ties — byte-equal to
the plain probe, pinned by the
`anchored_key_via_parent_memo_matches_plain_probe` differential test.

Memo scope matters: the memo lives one decode-step iteration (the parent
state advances at the bottom of each piece iteration). A briefly-tried
whole-construction scope produced stale parent keys and flipped one
selection tie-break — caught before any push by
`coordinator_vectors::full_job_vectors_match_ts_oracle` (the roomy-n6-c0
canonical geometry hash), which the 18-row golden alone did not trip;
scope is now pinned by that suite plus a code comment.

Measured (mixed-61-2000x2700-compact, 3 runs each, correct scope): user
CPU 39.5 s → 37.0 s (−6 %); wall unchanged on this 16-core machine
(scoring is parallel, off the coordinator critical path). Smaller/loaded
machines convert the CPU cut to wall time directly. Golden identical.

### Stage 2 (reverted) — rayon granularity knobs

Tried `with_min_len(8)` on the per-point legality dispatch and a
32 → 256 scoring chunk size. Both are provably order-preserving, but
measurement showed wall-time regressions (min_len starved workers inside
32-item chunks: sys 5.9 → 8.5 s; chunk 256 alone was flat-to-worse).
Reverted in full. Lesson recorded: the ~23 % flat crossbeam share is
mostly idle-worker steal-spin during serial coordinator phases — CPU-time
noise, not wall-time cost — so scheduling knobs don't pay here.

### Stage 3 (kept) — shared occupied Clipper union in layout topology

`measure_canonical_layout_topology_exact` executed the identical even-odd
occupied Union twice (hull-gap and cavity measurements). The tree — a pure
function of the paths — is now built once and shared via `_from_union`
variants; plain-signature functions remain for other callers, and every
branch runs byte-identical code. Sits directly on the coordinator's
capacity-retention critical path.

Measured (mixed-61-2000x2700-compact, 3 runs): wall 16.0 s → 15.6–15.8 s
(−1.25 % to −2.5 % across runs), cumulative with Stage 1: user −6 %.
Golden identical; `canonical_layout_vectors` and the full workspace
release suite pass.

### Stage 4 (kept) — per-entry topology lookup cache in retention sorts

`compare_topology_metric` re-resolved both entries' memoized topology per
comparison (successor-identity String hash + a deep struct clone per
side). Retention sorts and the depth trace now decorate each pass with
once-cells: the first comparison that needs an entry still calls
`measure` at exactly the same moment — same memoized-measure set, same
checkpoint-visible `topologyMeasurementCount`/`Ms` increments, pinned by
`capacity_search_vectors`' integrity-hash preimages — and later
comparisons borrow the slot. Wall within run noise (median
15.66 s → 15.59 s); kept as a strict per-comparison hash/allocation
removal on the serial retention path.

### Stage 5 (kept) — contact-graph edge lists built once per polygon

`measure_contact_graph` rebuilt both polygons' canonical grid edge lists
on every pair (each polygon's edges constructed n−1 times per topology
call). The all-pairs loop now prebuilds each edge list once; scan order,
short-circuit, and `None` propagation are unchanged. Wall within run
noise (median ≈ 15.6 s); a strict allocation/validation removal on the
serial topology path.

After stages 1–5 the cumulative mixed-61-2000x2700-compact medians are:
wall 16.03 s → ≈ 15.6 s (−2.5 % to −3 %), user CPU 39.5 s → ≈ 37.1 s
(−6 %). The honest reading: the remaining serial wall time is now spread
across candidate generation, strict state construction, and the periodic
portfolio residue, with no single ≥ 10 % local target left below the
structural beam-loop change.

### Stage 6 (kept) — candidate-generation rebuild hoists

Two pure hoists inside `generate_placement_candidates_uncached`:
`all_nfp_index` (never queried by the production `SheetlessNfp` domain) is
now built only inside the one branch that queries it, and the antiparallel
support scan's moving edge list — previously rebuilt for every fixed
edge — is built once per pass. Wall on mixed-61-2000x2700-compact:
median 15.6 s → 15.2 s.

### Stage 7 (kept) — moving-side NFP key parts prepared once per pass

`make_pairwise_nfp_cache_key` rebuilt the moving-polygon digest (the
dominant, forward-plus-reverse coordinate render) per placed piece, hit
or miss. The key builder now delegates through
`prepare_pairwise_nfp_moving_parts` +
`make_pairwise_nfp_cache_key_with_prepared_moving` (identical part
order → byte-identical keys), and both per-placed loops (parallel
precompute pre-pass, main resolve loop) prepare the moving parts once.
Wall: median 15.2 s → 14.6 s.

### Stage 8 (kept) — valid NFP cache hits translate in place

The hit path deep-cloned the cached relative boundary only to read it
once; valid hits now translate through a borrow-based store probe with
bit-identical telemetry accounting (`record_cloning_hit` fires for every
present entry exactly as the cloning `get` did). Wall within run noise;
a strict per-hit polygon-clone removal.

### Stage 9 (kept) — legal-candidate memo placed payload prepared per state

`build_memo_key` re-rendered every placed piece's exact ordered polygon
digest on each candidate-generation call, hit or miss. The memo-key input
now accepts a caller-prepared placed payload (same digest code →
byte-identical keys); the strict decoder prepares it once per piece
iteration, the capacity beam loop once per entry, and every other caller
renders per call as before. Wall median 14.56 s → 14.52 s.

### Stage 10 (kept) — short-circuit per-point NFP interior test

The indexed interior-rejection test collected every bounds-index match
into a Vec (cloning each value) per candidate point before an existential
`.any`. `BoundsIndex::any_match` now scans the identical pre-sorted range
in the identical order with the predicate interleaved — an existence test
over a pure predicate, so the result is unchanged and per-point work is
strictly ≤. Wall median 14.52 s → 14.26 s, user 35.4 s → 34.4 s.

Cumulative after stages 1–10 (mixed-61-2000x2700-compact, 5-run medians):
**wall 16.03 s → 14.26 s (−11.0 %), user CPU 39.5 s → 34.4 s (−12.9 %)** —
all with the quality golden byte-identical across the 18 rows and the
full workspace release suite green at every stage.

## Remaining opportunities (profile-ranked, deferred)

- Candidate-generation memo key, moving half: Stage 9 prepared the
  placed payload once per state; the moving-polygon digest of the memo
  key is still rendered per call (it varies per piece × transform, so
  the win is bounded).
- Scoring input construction: per-candidate `remaining_prepared_pieces`
  Vec clone and the per-candidate deep `TransformedCollisionGeometry`
  clone in `score_candidate_body` (allocator-cluster feeders).
- Candidate-generation per-call rebuilds (remainder after stages 6–10):
  per-boundary `validate_strict_boundary`/`bounds_for_points`/segment and
  `BoundsIndex` rebuilds across the transform loop while the placed set
  is frozen; the remaining collecting `query` call sites (checkpoint
  cadence is tied to collected indices there).
- `gap_regions.rs` duplicate `canonical_ring` (materializes all 2n
  rotations; the shared `canonical_bidirectional_cyclic_key` helper from
  the Issue-21 work is a drop-in) plus ring keys recomputed inside sort
  comparators — not hot for these three fixture families, relevant for
  gap-contained-heavy workloads.
- Structural ceiling: the capacity beam loop is serial per
  entry/transform/candidate; the coordinator is ≈ 94 % busy while 15
  workers average ≈ 4 % each. Any future wall-time step change on large
  mixed workloads needs either less serial work per beam entry or a
  deterministic parallelization of that loop — a design-level change, not
  a local optimization.
