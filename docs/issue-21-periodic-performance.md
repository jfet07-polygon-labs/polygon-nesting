# Issue 21 periodic archive performance investigation

Working notes and evidence for the safe single-thread optimization of the
periodic archive hot paths, with the 64-chord Issue 21 fixture as the primary
performance case. Correctness contract: the regular 64-gon stays a discrete
polygon with exactly 2 retained transform representatives, 2 P1 and 24 P2
`derive_cells` calls, complete runtime/transform/pair/cell coverage, and
byte-for-byte unchanged canonical keys, ordering, provenance, and archive
hashes.

## Baseline (pre-optimization)

- Machine: Linux x86_64, 16 hardware threads, 62 GiB RAM (same machine for
  every number in this document; release build, `codegen-units = 1`,
  `lto = "thin"`, project toolchain rustc 1.95.0 via `nix develop`).
- Revision: branch `fix/periodic-round-family-archive` at
  `d22de76 docs: add Fable performance investigation prompt` (clean worktree).
- Build command (compilation excluded from every timed interval):

  ```sh
  nix develop -c cargo test --release --locked -p polygon-nesting-napi --test job --no-run
  ```

- Timing command per fixture (prebuilt test binary invoked directly, 5 runs
  each, GNU `time -p`):

  ```sh
  time -p ./target/release/deps/job-<hash> issue_21_interchangeable_arc_circle_desktop_request_completes --exact
  time -p ./target/release/deps/job-<hash> issue_21_interchangeable_chord_circle_desktop_request_completes --exact
  ```

### Baseline timings

| fixture | run 1 | run 2 | run 3 | run 4 | run 5 | median |
| --- | --- | --- | --- | --- | --- | --- |
| arc (`repro-2circles.json`) | 0.71 s | 0.70 s | 0.72 s | 0.70 s | 0.72 s | **0.71 s** |
| chord (`G-2circles-lines.json`) | 19.41 s | 19.28 s | 19.34 s | 19.31 s | 19.32 s | **19.32 s** |

### Baseline deterministic work telemetry (release 64-gon characterization)

From `regular_polygon_periodic_work_is_deterministic_and_observer_only`
(release, 64 sides), captured with a temporary local print that was reverted
before any commit:

```text
p1_derive_cells_calls: 2
p2_derive_cells_calls: 24
raw_p2_offsets: 132
nonnegative_p2_offsets: 24
duplicate_candidate_orbits: 0
p2_sheetless_legality_checks: 132
basis_candidates: 1770
lattice_diagnosis_requests: 1770
lattice_diagnosis_computations: 1770
lattice_diagnosis_memo_hits: 0
```

### Baseline profile

`perf record --call-graph fp -e cpu-clock:u` over the chord fixture test
(frame-pointer rebuild in a scratch target dir, itself not benchmarked),
7 987 samples. Cumulative shares of total runtime:

```text
95.4 %  run_intrinsic_periodic_family_portfolio
56.2 %  enumerate_intrinsic_periodic_cells
55.7 %    derive_cells
49.3 %      canonical_cell_key
48.6 %        canonical_cycle
39.2 %  enumerate_intrinsic_periodic_cell_crops
 2.3 %  assess_placement
```

Flat view: `ryu_js::pretty::format64` 21.4 %, BigInt-to-decimal rendering
(`to_radix_le` + `to_str_radix_reversed` + `Display`) 16.8 %, allocator
traffic (`malloc`/`free`/`realloc`/`memmove`) ≈ 21 %, `core::fmt` machinery
≈ 8 %. The workload is dominated by string rendering and allocation, not by
geometry math.

### Dominant cost identified

`canonical_cell_key` is called once per accepted basis candidate
(`basis_candidates = 1770` across the 26 derivations). Each call recomputed,
per quarter-turn, the member identity strings (grid conversion, rotation,
min-shift, `canonical_cycle`) even though those strings depend only on the
members — the basis only contributes the two formatted basis vectors of each
variant. `canonical_cycle` itself materializes all `2n` full rotation strings
of an `n`-token ring (`2n²` BigInt renders per call) and sorts them. For the
64-gon: ~14 000 `canonical_cycle` calls ≈ 124 M BigInt renders per
enumeration, matching the 48.6 % cumulative share.

### First optimization selected

Stage 1: hoist the basis-independent per-quarter-turn member identity strings
out of the per-basis loop in `derive_cells` (compute them lazily once per
`derive_cells` invocation, reuse across all basis candidates). Pure code
motion of a pure function; canonical keys stay byte-for-byte identical, and
every deterministic counter is unchanged. Candidate area 7 of the assignment
("recomputing member-only measurements inside basis loops"), guarded by a
naive test oracle plus the existing TS-dump vector suites.

Further stages (cyclic canonicalization without materializing every rotation,
lattice diagnosis memoization) are evaluated one at a time on re-measured
profiles after each stage lands.

## Stage 1 — hoist basis-independent member turn keys out of the basis loop

Change: `canonical_cell_key` split into `canonical_cell_member_turn_keys`
(member-only, per quarter-turn, computed lazily once per `derive_cells`
invocation) and the per-basis variant assembly. Pure code motion of a pure
function; guarded by a naive verbatim oracle of the pre-change implementation
(`canonical_cell_key_matches_naive_oracle_across_roles_members_and_bases`)
plus the existing TS-dump vector suites.

Verification (release, same machine): characterization
`regular_polygon_periodic_work_is_deterministic_and_observer_only` passes
(4.57 s, down from 33 s for its three enumerations);
`canonical_layout_vectors` 4/4; `job_service`
`compact_archive_completes_for_interchangeable_regular_polygon_copies` passes;
all three `issue_21_*` N-API regressions pass. Deterministic counters
unchanged from baseline (identical dump, including `basis_candidates: 1770`).

### Stage 1 timings

| fixture | run 1 | run 2 | run 3 | run 4 | run 5 | median | vs baseline |
| --- | --- | --- | --- | --- | --- | --- | --- |
| arc | 0.65 s | 0.64 s | 0.64 s | 0.66 s | 0.66 s | **0.65 s** | −8 % |
| chord | 9.87 s | 9.84 s | 9.83 s | 9.84 s | 9.86 s | **9.84 s** | −49 % |

The chord improvement matches the profiled 49.3 % `canonical_cell_key`
cumulative share; the arc fixture has tiny rings and small basis counts, so
the small improvement there is expected.

## Stage 2 — cyclic canonicalization without materializing every rotation

Change: new crate-private
`js_number::canonical_bidirectional_cyclic_key(tokens: &[String]) -> String`.
It selects the `cmp_js_code_units`-smallest semicolon-joined rotation across
both windings by comparing candidates as **virtual joined code-unit
streams** (never token-by-token, so `"1,2"`+`';'` versus `"1,20"`+digits
prefix hazards order exactly as before) and materializes only the winner.
Each coordinate token is rendered exactly once per call site. Adopted by:

- `archive::periodic_cells::canonical_cycle` (BigInt grid tokens): was
  `2n²` BigInt renders + `2n` full string materializations + sort per call;
- `canonical_grid::layout::canonical_ring` (JS float tokens): was `n²`
  renders per winding via the now-removed `canonical_ring_direction`.

Guarded by three naive materialize-everything oracles copied verbatim from
the pre-change implementations:
`canonical_bidirectional_cyclic_key_matches_materializing_oracle`
(token-level: empty/singleton rings, repeated tokens, prefix hazards,
negative/zero and large-BigInt-scale tokens, JS exponent renders,
supplementary-plane UTF-16 code-unit ordering, LCG-generated rings),
`canonical_cycle_matches_materializing_oracle` (GridPoint level) and
`canonical_ring_matches_materializing_oracle` (CanonicalGridPoint level,
including `-0` folding and `1e+21`/`1e-7` renders), plus the existing
`canonical_layout_vectors` and periodic TS-dump vector suites.

### Stage 2 timings

| fixture | run 1 | run 2 | run 3 | run 4 | run 5 | median | vs baseline |
| --- | --- | --- | --- | --- | --- | --- | --- |
| arc | 0.11 s | 0.09 s | 0.10 s | 0.11 s | 0.10 s | **0.10 s** | −86 % |
| chord | 1.84 s | 1.84 s | 1.83 s | 1.85 s | 1.85 s | **1.84 s** | −90 % |

Verification (release, same machine): characterization passes with the
identical counter dump as baseline; `canonical_layout_vectors` 4/4;
`job_service` regression 10.62 s → 2.05 s; all three `issue_21_*` N-API
regressions pass (9.83 s → 1.86 s for the filtered trio). The arc fixture's
large gain comes from `canonical_ring` inside the crop/continuation phase
(39.2 % of the baseline profile), which Stage 1 did not touch.

## Stage 3 — hoist remaining member-only work out of the basis loop

The post-Stage-2 profile (2 314 samples over the 1.9 s chord run) showed the
remaining `derive_cells` cost dominated by three member-only computations
still executed per basis candidate (1 770 times instead of ≤ 26):

```text
26.3 %  far_neighbor_certificate_grid   (O(V²) BigInt max pairwise distance)
12.1 %  measure_base_cell_shape         (translated points, BigInt hull)
11.2 %  polygon_area_grid2              (member doubled-area fold)
```

Change: `far_neighbor_certificate_grid` split into the member-only
`far_neighbor_maximum_distance_squared` plus the cheap per-basis
`far_neighbor_certificate_from_maximum`; `derive_cells` now lazily computes
the far-neighbor maximum, the member doubled area, and the base-cell shape
once per invocation and reuses them across basis candidates (all three are
provably independent of the basis and candidate source; the per-candidate
`BaseShapeRejected` rejection accounting is unchanged because the shared
value is identical for every candidate). Guarded by a verbatim naive oracle
(`far_neighbor_certificate_matches_naive_oracle`) plus the existing suites.

### Stage 3 timings

| fixture | run 1 | run 2 | run 3 | run 4 | run 5 | median | vs baseline |
| --- | --- | --- | --- | --- | --- | --- | --- |
| arc | 0.10 s | 0.10 s | 0.10 s | 0.10 s | 0.10 s | **0.10 s** | −86 % |
| chord | 0.84 s | 0.82 s | 0.84 s | 0.84 s | 0.84 s | **0.84 s** | −95.7 % |

Verification: release characterization passes in 0.86 s (baseline 33 s) with
the identical counter dump as baseline; `canonical_layout_vectors` 4/4;
`job_service` regression 0.93 s; all three `issue_21_*` N-API regressions
pass; full workspace release test suite passes (`cargo test --workspace
--release --locked`, exit 0) and `cargo clippy --workspace --all-targets
--release --locked -- -D warnings` is clean.

### Post-Stage-3 profile and deferred opportunities

`perf record --call-graph fp -e cpu-clock:u` over the chord fixture after
Stage 3 (2 717 samples): `run_intrinsic_periodic_family_portfolio` is down
to 49.4 % of the process; the residual cost is diffuse — `assess_placement`
17.7 % self (crop/lattice/edge-contact legality), canonical layout topology
measurement ≈ 18 %, edge-contact derivation ≈ 7.6 %, `ryu_js::format64`
≈ 7.1 % (NFP cache-key digests and layout identity renders). The member-only
hoists and cyclic-key hotspots no longer register.

Deferred with evidence (each now bounded by the shares above, against
substantially higher blast radius):

- **Prepared sheetless legality context** (assignment area 2): the largest
  remaining single item, but it must reproduce the general validator
  byte-for-byte including error precedence; deferred as a stand-alone
  follow-up with its own differential oracle.
- **Edge-contact enumeration prefilters** (area 3): exact-order-preserving
  direction indexing, ≈ 7.6 % ceiling today.
- **Per-derivation lattice diagnosis memoization** (area 4):
  `diagnose_lattice` no longer appears among the dominant remaining costs
  for this fixture (its duplicate-diagnosis savings are bounded by the
  edge-contact validation share); the memo-hit telemetry counters stay in
  place for the fixture that would justify it.
- **NFP construction/caching changes** (areas 1 and 6): `format64` digest
  cost ≈ 7 %; switching `LinearEdgeMerge` in or restructuring cache
  identities requires the global parity evidence the assignment describes
  and is not justified by the current profile alone.

## Telemetry observer concern — resolution

The opt-in `IntrinsicPeriodicWorkTelemetry` counters increment between
finite-deadline checks, so an enabled-telemetry run extremely close to a
finite deadline could theoretically admit one fewer later candidate than a
disabled run. Resolution adopted (assignment outcome 1): telemetry stays
strictly test-only. Production always passes `telemetry: None`
(`periodic_family.rs` portfolio driver), and the focused characterization
opts in only under `maximum_runtime_ms = ∞`, where the deadline comparison
is unreachable. No production finite-deadline observer-neutrality claim is
made or implied; the deterministic-equality guarantees the characterization
proves (identical keys, coverage, and counters across repeated runs, with
and without telemetry) hold under the no-deadline seam only.

## Result summary

| fixture | baseline median | final median | speedup |
| --- | --- | --- | --- |
| arc (`repro-2circles.json`) | 0.71 s | 0.10 s | 7.1× |
| chord (`G-2circles-lines.json`) | 19.32 s | 0.84 s | 23× |

The release 64-gon characterization counter dump is byte-identical at every
stage (2 retained representatives, 2 P1 / 24 P2 derivations, 132 raw P2
offsets, 24 nonnegative offsets, 0 duplicate orbits, 1 770 basis candidates
and lattice diagnoses, 0 memo hits), and all canonical-key, layout,
ordering, provenance, and coverage suites pass unchanged — no vector,
snapshot, or expected output was regenerated.

## Files changed

- `crates/polygon-nesting-core/src/archive/periodic_cells.rs` — Stage 1
  (`canonical_cell_member_turn_keys` split + lazy reuse in `derive_cells`),
  Stage 2 (`canonical_cycle` on the shared virtual-rotation helper), Stage 3
  (far-neighbor maximum + member doubled area + base-cell shape hoists);
  naive-oracle unit tests for each.
- `crates/polygon-nesting-core/src/js_number/mod.rs` — the
  `canonical_bidirectional_cyclic_key` helper and its token-level oracle
  battery.
- `crates/polygon-nesting-core/src/canonical_grid/layout.rs` —
  `canonical_ring` on the shared helper (`canonical_ring_direction`
  removed); ring-level oracle test.
- `docs/issue-21-periodic-performance.md` — this evidence document.
