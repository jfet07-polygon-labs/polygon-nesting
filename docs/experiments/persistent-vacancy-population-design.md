# Persistent vacancy population experiment

## Decision boundary

This is a diagnostic-only Mixed-61 experiment behind a mode argument and a required frozen-parent fixture path, after the retained zero-valued retired-terminal argument. Mode `0` disables it and accepts no parent path. Modes `1` and `2` use the reviewed two-hardest scheduler with comparator and contact-signature retention respectively. Modes `3` and `4` use the stateless rotating scheduler defined below with those same two retention policies respectively. Mode `6` applies dual-objective retention to mode `3` without carryover, and mode `5` adds the bounded elite carryover to that mode-`6` policy. Historical mode `7` is retired and unavailable. Modes `8` and `9` form the bounded macro-successor control and treatment described below. Nonzero modes must not change the protected constructor, relaxed search, coupled separator, public profiles, or returned result. A diagnostic candidate is reported separately until every promotion gate passes.

The frozen request SHA-256 is `dfd2ceecf02efe3475e3344dfefbfb2a2a5bd8a673008b449f5689507c933ba1`. The required retained boundary-projection placement fingerprint is `b9335a72cdcdd8df29be21450818f4ab1766ea1ea0b16765ad3998942a2ea6c5`, with reported depth `168.625 mm` and independently rebuilt source depth `168.361 mm`. The canonical placements live in `tests/fixtures/mixed-61/persistent-vacancy-parent-b9335a72.json`; they are input to the vacancy lifecycle rather than regenerated from the platform-sensitive continuous separator trajectory. The fixture's raw SHA-256, request hash, effective sheet and clearance settings, placement count, exact placement fingerprint, full canonical validation, independent source validation, and grid depth are all checked before `attempted` becomes true. This prevents sub-grid placement edits or geometry-setting overrides from reusing the same rounded placement fingerprint while changing the successor trajectory. A requested nonzero mode that does not reach `attempted: true` makes both the core wrapper and benchmark process fail instead of emitting a successful-looking no-op result. The experiment searches a fixed `165.000 mm` strip.

## Non-duplication map

The following mechanisms are closed and must not be repeated:

- preserving more constructor orders;
- a complete-layout retained-infeasible restart pool;
- pair templates, pair shadows, and pairwise NFP terminal beams;
- frontier-only reconstruction followed by the existing greedy transition;
- independent retries of the precompression vacancy children;
- target-native crossover followed by the unchanged separator;
- exact-area finalist reranking;
- one-pair terminal repair and simultaneous complete-layout projection.

The first causal variable is topology-diverse retention while an exact-valid vacancy migrates across several insertion/ejection transitions. No previous experiment retained several active-set/contact alternatives across such transitions. The follow-up screen separately tests the selected-piece scheduler without changing retention policy.

## State and invariant

Each state contains one placement slot per requested piece, an active bitset, source-rebuilt expanded collision polygons for active pieces, exact canonical metrics, and the last transition. Inactive placement slots are inert storage only. Every successor uses the same immutable frozen baseline pose for that piece as its current-pose hint; an ejected pose never affects a later successor. Semantic state identity contains only canonical active geometry, ordered inactive stable IDs, and the last transition. The ordered transition trace is recorded outside the state for diagnostics and never participates in deduplication, ordering, seeding, or any other behavior.

The ordered active IDs are derived from the active bitset in canonical input-piece order. Before an initial partial or a layer winner is accepted, the implementation asserts:

- active-bit count equals active-ID count;
- active IDs are unique and exactly equal the IDs in the filtered placements;
- active collisions exist exactly for active IDs and no inactive collision exists;
- every active collision fits the `165.000 mm` expanded-collision rectangle using `collision_sheet_inset_mm`;
- the incremental exact row proof contains no positive active-pair intersection.

A full active-state audit then filters pieces and placements by the same active IDs, reapplies target-depth settings, asserts the filtered sets again, runs canonical validation on that filtered view, and runs source-geometry publication validation on the filtered `GeneralPlacement` view. This audit runs for the initializer and the comparator-best state after every completed layer, for at most 41 partial audits per arm. Every complete child additionally enters the ordinary full 61-piece canonical and independent publication gates.

## Initial partial

Reconstruct the retained `168.361 mm` endpoint from its diagnostic placements and rebuild every expanded collision polygon from source. Compute the canonical target rectangle using the same collision inset as publication. Deactivate exactly those pieces for which `PolygonSet::fits_rect` is false. Order the inactive IDs by descending positive canonical-grid boundary overflow, then stable ID. Do not remove any additional “blockers”: removing boundary offenders from an exact-valid full layout cannot create a pair overlap.

Run the authoritative filtered active-state audit. Any validator disagreement, a nonpositive overflow, fewer than half the pieces active, more than 32 inactive pieces, or a frozen identity mismatch rejects the arm. Diagnostics pin the exact inactive ID order and its SHA-256 before the result is interpreted. The earlier 24-piece observation used source-material frontier bounds and is background evidence only; this initializer makes no assumption that expanded-collision containment yields the same count.

## Transition

The fixed mode-independent seed domain is `0x5650_4f50_3030_3031`. Control and treatment never mix their mode, label, retention ordinal, or population position into a seed. For each parent compute `parentSeedKey` from its semantic identity as defined below, then use `transitionSeed = derive_seed(BASE ^ parentSeedKey, layer, 0)`. For selected-piece ordinal `s` and input piece index `p`, call the existing generator with `angleSeed = derive_seed(transitionSeed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN, s, p)`. For orientation ordinal `o`, call the existing position generator with `positionSeed = derive_seed(transitionSeed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN, s * 12 + o, p)`. Use `diversitySeed = derive_seed(transitionSeed ^ CONFLICT_RUIN_DIVERSITY_SEED_DOMAIN, s, p)` for every shortlist key. A physical parent shared by both arms therefore receives identical streams even if retention gives it a different ordinal.

Behavior compares the semantic identity tuple directly. Its diagnostic digest and `parentSeedKey` use SHA-256 over ASCII domain `persistent-vacancy-state-v1\0`, followed by a big-endian `u32` active count. For each active piece in canonical input-piece order, encode its stable ID as a big-endian `u32` UTF-8 byte length and bytes, followed by its signed canonical angle key as big-endian `i64`, mirrored as one byte, and signed canonical-grid `x` and `y` as big-endian `i64`. Then encode the big-endian `u32` inactive count and the same length-framed stable IDs in canonical input-piece order. Encode last transition with byte `0` when absent or byte `1`, the inserted length-framed ID, a big-endian `u32` ejected count, and the sorted length-framed ejected IDs. `parentSeedKey` is the first eight digest bytes interpreted as a big-endian `u64`; the full digest is diagnostic only and never substitutes for direct tuple equality.

For every retained parent, select at most two inactive pieces by the following fixed key:

1. descending exact canonical doubled expanded-collision area;
2. descending exact canonical convex-hull deficit, defined as hull doubled area minus material doubled area;
3. descending minimum canonical collision-bounds side;
4. ascending stable ID.

For a selected piece, reuse the existing `conflict_ruin_orientations` ordering exactly, except that its “current” pose is the immutable frozen baseline pose: baseline orientation, orthogonal orientations, source-edge alignments, then seeded continuous orientations, deduplicated by angle key and mirror and truncated to 12. Reuse the existing `conflict_ruin_positions` category order with that same immutable hint: baseline position, sheet supports, active-piece AABB supports in input order, 16 focused samples, and 16 global samples. Round-robin the categories, deduplicate by placement key, and truncate to 32 positions per orientation.

For each proposal, `proxyLoss` is exactly the sum of `JaguaHazardIndex::collision_pressure(pieceIndex, hazard_pose(placement), fixedPieceId)` over the active colliding IDs returned by `query_unplaced`, in returned canonical ID order. `diversityKey` is exactly `conflict_ruin_diversity_key(placement, diversitySeed)`. Within each orientation stream, sort proposals by `(proxyLoss, diversityKey, placementKey)` and retain two. Merge those at most 24 candidates by `(proxyLoss, orientationOrdinal, diversityKey, placementKey)`, deduplicate by placement key preserving first occurrence, and retain eight complete exact finalist rows for the selected piece.

Exact-score each finalist against every active collision. A zero-overlap row inserts the piece directly. A positive-overlap row may create an ejection child by removing every positive-area blocker before activating the piece, provided the sorted blocker set has one or two members and the resulting inactive count is at most 32. The child carries `(insertedPiece, sortedEjectedPieces)` as its last transition. Reject an immediate reversal exactly when the new inserted piece was in the parent’s ejected set and the new ejected set contains the parent’s inserted piece.

The parent is exact-valid. Removing blockers cannot create overlap, and the complete row proves the inserted collision has zero positive intersection with every remaining active piece. Boundary fit is checked before the row. Thus the child is exact-valid by induction; positive overlap discovers the blocker set but is never retained. The first experiment does not relocate active pieces, run a separator inside a partial, or add another terminal repair.

## Population and objective

Both arms retain at most eight states for at most 40 completed transition layers and expand every retained parent with the identical piece, orientation, position, finalist, ejection, quota, and audit schedule.

Deduplicate children by state identity, then sort by:

1. lower total inactive exact doubled expanded-collision area;
2. lexicographically lower descending sequence of the inactive piece-difficulty keys defined above;
3. fewer inactive pieces;
4. lower exact doubled material area ejected by the current edge;
5. fewer pieces ejected by the current edge;
6. lower active material frontier on the canonical grid;
7. canonical state and last-transition keys.

There is no unbudgeted next-layer lookahead.

The retention key compares the structural tuple `(orderedActiveIds, sortedEdges)` directly; hashing never controls behavior. An edge `(smallerId, largerId, axis)` exists when canonical expanded-collision bounds have zero gap on exactly one axis and strictly positive projected overlap on the other; corner-only contact is excluded and `axis` is byte `0` for `x` and `1` for `y`. IDs are UTF-8 and edges sort by the two IDs then axis.

Diagnostics additionally hash that tuple as SHA-256 over ASCII domain `persistent-vacancy-contact-v1\0`. Encode the active-ID count as big-endian `u32`, then each ID as big-endian `u32` byte length followed by UTF-8 bytes. Encode the edge count the same way, then each edge as two length-framed IDs followed by its one-byte axis. No native-width values or separators enter the stream.

The control takes the first eight comparator-sorted children. Treatment scans the same sorted list and first retains the first representative of each new contact signature until eight slots are full, then fills remaining slots from the comparator order. If more than eight signatures exist, only the first eight in comparator order are retained. The arms have equal width and identical per-parent schedules; after retention chooses different parents, later generated geometry may naturally differ, and all work remains separately reported.

A complete state beats an incomplete state. Between complete states, lower independently measured canonical-grid depth wins, then the placement fingerprint. “Control matches treatment” means identical normalized layer identities, work counters, validation outcomes, and terminal state. A treatment promotion requires a complete state that the control does not reach, or a strictly lower independent grid depth if both complete.

## Hard quotas

Each arm has its own nontransferable ceilings. The global ceilings fund the new modes `8` and `9`, which add at most one bounded macro-parent expansion per layer to the original eight-parent schedule. Modes `1` through `6` retain their smaller realized work. The declared worst case is:

- layers: `40`;
- parent expansions: `40 * (8 + 1) = 360`;
- selected-piece slots: `360 * 2 = 720`;
- orientation streams and collision builds: `720 * 12 = 8,640`;
- canonical source-feature visits across both mirror traversals: `720 * 2 * 512 = 737,280`;
- pre-deduplication position-source attempts per orientation: `1 + 8 + 61 * 8 + 16 + 16 = 529`;
- total pre-deduplication position-source attempts: `8,640 * 529 = 4,570,560`;
- returned positions, hazard queries, and placement attempts: `8,640 * 32 = 276,480`;
- proxy-pressure visits: `276,480 * 61 = 16,865,280`;
- exact finalist rows and finalist collision builds: `720 * 8 = 5,760`;
- initializer collision builds: `61`;
- experimental collision builds: `61 + 8,640 + 5,760 = 14,461`;
- initializer exact pair rows: `61 * 60 / 2 = 1,830`;
- finalist pair visits: `5,760 * 60 = 345,600`;
- experimental pair visits: `1,830 + 345,600 = 347,430`;
- partial dual audits: `41`;
- complete dual publication audits: `64`;
- total dual audits: `41 + 64 = 105`;
- validator collision builds per dual audit: `2 * 61 = 122`;
- validator collision builds: `105 * 122 = 12,810`;
- validator pair visits per dual audit: `2 * 1,830 = 3,660`;
- validator pair visits: `105 * 3,660 = 384,300`;
- aggregate collision-build ceiling: `14,461 + 12,810 = 27,271`;
- aggregate pair-visit ceiling: `347,430 + 384,300 = 731,730`.

Reject an input piece whose canonical source representation exceeds 512 features. Check every rebuilt expanded collision immediately and reject the arm if it exceeds 512 vertices. The resulting transformed-collision vertex ceiling is `27,271 * 512 = 13,962,752`. AABB-disjoint pair visits still consume the logical pair-visit ceiling but skip Clipper. Before each pair operation, charge its actual two input vertex counts; the conservative aggregate Clipper input ceiling is `2 * 512 * 731,730 = 749,291,520` vertices. This is a monotonic work counter, not a simultaneous-memory allowance.

Clipper output vertices are unknowable before an intersection. Charge them immediately after the operation and before committing any candidate, audit, layer, or publication state, against a 4,000,000-vertex ceiling. If the charge exceeds the ceiling, discard the uncommitted result and incomplete layer, mark that arm cap-exhausted, and prohibit experimental publication. Work already performed remains in diagnostics. The runtime and RSS gates remain mandatory; the large deterministic counter ceilings do not excuse a slow or memory-heavy run.

Complete children enter the dual publication audit in deterministic comparator order. The sixty-fifth complete child exhausts the declared complete-audit quota before either validator runs; it is not retained, the incomplete layer is discarded, and the arm becomes cap-exhausted. Partial audits are likewise preflighted before their first validator. A dual audit commits its semantic outcome only after both validators finish within every remaining build, pair, input-vertex, output-vertex, and invocation ceiling.

Before retaining a layer, charge the actual capacities of placements, active bits, collision regions/rings/points, IDs, signatures, and out-of-state diagnostic transition records. The existing `retainedPeakBytes` remains the state-population peak so the pre-selector experiment stays semantically comparable. Add `selectorDiagnosticPeakBytes` for the ordered per-parent rows and `totalRetainedPeakBytes` for the simultaneous state-plus-diagnostic peak. The 64 MiB cap and every promotion decision use `totalRetainedPeakBytes`; `retainedPeakBytes` alone is no longer the complete memory gate. Charge selector rows before committing the layer diagnostics so a cap failure cannot retain an unaccounted row. More than 64 MiB of experiment-owned live or peak retained allocation rejects the arm atomically. Initialization, generator construction, scoring, partial audits, complete publication audits, and retention have separate counters in each arm. No unused quota is transferred between arms.

## Focused test contract

Before any benchmark result is interpreted, focused unit and integration tests must prove:

- filtered validation accepts exactly the active IDs and rejects missing, duplicate, extra, inactive, out-of-boundary, and positive-overlap entries in both validators;
- changing every stored inactive or ejected pose leaves successor proposals, state identities, and seeds byte-identical because only immutable baseline hints are behavioral;
- direct insertion, one-blocker ejection, two-blocker ejection, more-than-two-blocker rejection, inactive-count rejection, and the exact immediate-reversal predicate have fixed examples;
- physically identical states reached through different transition traces deduplicate, while distinct last transitions remain distinct only where the reversal rule makes their futures different;
- semantic identity encoding and `parentSeedKey` have a golden byte stream and digest, and the same shared state receives identical control/treatment streams regardless of population ordinal;
- contact tuples and diagnostic digests are invariant under storage/insertion-order permutation, ignore corner-only contact, distinguish `x` from `y`, and make treatment reservation deterministic when more than eight signatures exist;
- ordered per-parent selector diagnostics reproduce the parent identity, inactive order, slots, and seeds; shared-parent rows match across modes `1`/`2` and `3`/`4`, inactive storage permutation is inert, one inactive piece yields only slot zero, slot one skips slot zero, and an unchanged `n`-piece inactive set exercises every non-hard entry within `n` layers;
- complete-audit ordering is comparator-stable and the sixty-fifth complete child exhausts the arm before validation;
- every pre-operation ceiling and the post-operation Clipper output ceiling fails atomically without committing a candidate, layer, audit, or publication state;
- the aggregate quota formulas equal the declared constants, mirror-enabled orientation generation charges both source-feature traversals, and phase counters sum to the aggregate counters;
- modes `8` and `9` build and audit the same combined macro pool, mode `8` preserves mode `3`'s population history, only mode `9` admits novel macro identities, retained-novel fingerprints are truthful, a complete macro child is audited in both arms but ignored by the control, and raw macro/combined memory failure is atomic;
- the stable mode-zero compatibility projection reproduces SHA-256 `f51f8d4e767c4828430af4f154616b9c73aa237f1cbfbf0cc3e04d6cadfe85d0`;
- one fixed concave source piece completes an insertion/ejection lifecycle and passes both source and canonical validation.

## Reproducibility and gates

The canonical command prefix is:

```text
nix develop -c cargo run --release --locked -p polygon-nesting-core \
  --features jagua-experimental --example general_request_benchmark -- \
  tests/fixtures/mixed-61/mixed61-request.json \
  1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 \
  24 8 40 10 10 5 0 0.005 0.001 1 6 0 0 0 structured \
  0 10 1 0 0 0 0 MODE \
  tests/fixtures/mixed-61/persistent-vacancy-parent-b9335a72.json
```

Run four cold processes after one unmeasured release build in the order control, treatment, treatment, control, with `MODE=1,2,2,1`. The canonical identity is Apple M4 Max, `aarch64-apple-darwin`, eight requested and actual threads, rustc `1.95.0 (59807616e 2026-04-14)`, LLVM `22.1.2`, locked dependencies, and no `RUSTFLAGS`. Every output records the engine commit, dirty status, relevant-source-tree hash, executable hash, full rustc identity, machine, thread counts, request hash, frozen-parent fixture hash, frozen parent fingerprint, mode, seed domain, and exact command. The two outputs for each arm must match those identities. Cross-platform comparisons start from this identical frozen parent; the upstream mode-zero separator output remains platform-bound and is not a portability oracle.

The follow-up scheduler screen uses the same single unmeasured release build for every arm. Run `MODE=1,3,3,1` to isolate the scheduler under comparator retention, then `MODE=2,4,4,2` to isolate it under contact-signature retention. Every invocation uses the canonical command above with only `MODE` changed. Both replays of each mode must be byte-identical after the declared treatment/control normalization. Before comparing a new scheduler pair, its contemporaneous modes `1` and `2` must also match the normalized hashes and semantic evidence pinned by `persistent-vacancy-initial-evidence.json`; an implementation change that perturbs the old arms invalidates the screen instead of silently establishing a new control. All eight processes must report the same executable, relevant-source-tree, request, parent, toolchain, machine, and thread identities.

Mode `0` must remain structurally compatible with the protected output. Normalize it with this exact projection:

```sh
jq -S '
  walk(
    if type == "object" then
      with_entries(select((.key | test("(?:^elapsedMs$|ElapsedMs$)")) | not))
    else . end
  )
  | .pairConstructorProbe = null
  | .rustcVersion = null
  | del(
      .quota.precompressionFrontierVacancyMode,
      .quota.exactPairTerminalMode,
      .quota.persistentVacancyMode,
      .relaxedDiagnostics.coupledDynamicSeparator.precompressionFrontierVacancy,
      .relaxedDiagnostics.coupledDynamicSeparator.persistentVacancyPopulation,
      .engineCommit,
      .engineWorktreeDirty,
      .engineWorktreeStatus,
      .executableSha256,
      .relevantSourceTreeSha256
    )
' INPUT.json
```

The projected mode-`0` JSON must hash exactly to `f51f8d4e767c4828430af4f154616b9c73aa237f1cbfbf0cc3e04d6cadfe85d0` with SHA-256. Mode `0` omits `persistentVacancyParentFixture` entirely, preserving its pre-fixture output shape. The earlier `0f39c64...` projection included the then-current commit and was therefore commit-bound rather than a stable compatibility hash; deleting `engineCommit` is the only projection correction, and the old and new same-layout outputs both reproduce `f51f8d4...` under it.

Normalize each treatment/control replay with this exact projection:

```sh
jq -S '
  walk(
    if type == "object" then
      with_entries(
        select(
          (.key | test("(?:^elapsedMs$|ElapsedMs$|^wallSeconds$|^maximumResidentSetBytes$|^phaseElapsedMs$)"))
          | not
        )
      )
    else . end
  )
' INPUT.json
```

Input and parent identities, provenance fields, initial inactive order, layer states, transition history, work counters, exact rows, validation outcomes, and terminal placements must otherwise be byte-identical between same-arm replays. If an implementation uses an additional timing or RSS field name, the projection and its tests must be amended explicitly before interpreting results; no field may be discarded ad hoc after observing a mismatch.

The newly additive ordered selector rows are retained by that projection for every same-build replay and cross-arm diagnostic check. Compatibility with the pre-row modes `1` and `2` pinned in `persistent-vacancy-initial-evidence.json` uses this separate exact legacy projection and no other deletion:

```sh
jq -S '
  walk(
    if type == "object" then
      with_entries(
        select(
          (.key | test("(?:^elapsedMs$|ElapsedMs$|^wallSeconds$|^maximumResidentSetBytes$|^phaseElapsedMs$)"))
          | not
        )
      )
    else . end
  )
  | del(
      .engineCommit,
      .engineWorktreeDirty,
      .engineWorktreeStatus,
      .executableSha256,
      .relevantSourceTreeSha256,
      .relaxedDiagnostics.coupledDynamicSeparator
        .persistentVacancyPopulation.layers[].parentSelections,
      .relaxedDiagnostics.coupledDynamicSeparator
        .persistentVacancyPopulation.work.selectorDiagnosticPeakBytes,
      .relaxedDiagnostics.coupledDynamicSeparator
        .persistentVacancyPopulation.work.totalRetainedPeakBytes
    )
' INPUT.json
```

The omitted provenance and memory are not ignored: before projection, every contemporaneous arm must report one identical nonempty engine commit, dirty status, source-tree hash, and executable hash, and those new same-build identities are recorded alongside the result. Every arm must also report the additive selector-diagnostic and total retained-memory counters and pass the total-memory cap. The legacy projection removes the provenance only because implementation necessarily changes its old pinned values, and removes the two additive memory counters only because the old evidence predates selector rows; the old state-only `retainedPeakBytes` remains compared exactly. After this projection, contemporaneous mode `1` must hash to `b2d5ce99ecf31848abfcc5f08e8f0970168abdde939934067541bbb8fed3eaf7` and mode `2` to `603477dc250252930a3dfe9cd9c2109be6969ec3d4b5ef3bb9a314b8688374c8`. The projection is invalid for comparing modes `3` or `4`; their selector rows, additive memory counters, and same-build provenance remain required evidence.

Promotion requires all of the following:

- 61/61 pieces restored;
- canonical and independent source validation pass;
- independent depth at most `165.000 mm` and strictly below `168.361 mm`;
- treatment beats the equal-width control and the protected operational control;
- no cap fallback, incomplete row, incomplete layer, or audit disagreement;
- total engine time, measured around the complete production pipeline plus experimental arm but excluding process startup and JSON serialization, at most `6.000 s`;
- cold-process wall time after the unmeasured build at most `6.500 s`;
- peak process RSS at most `67,108,864` bytes and experiment-owned retained memory at most `64 MiB`;
- no baseline or concavity regression in the general-engine contract tests.

If no ejection child is produced, the candidate generator is closed for this role. If treatment and control retain identical signatures and states, diversity reservation is inactive. If treatment is worse than control, the signature policy is closed. If treatment retains a strictly better partial comparator state but neither arm completes, population persistence remains directionally supported while this schedule/objective is insufficient; it must not be mislabeled a full hypothesis failure. More terminal rounds, larger exact cleanup budgets, and another complete-layout restart pool remain prohibited.

## Follow-up selector-starvation screen

The first quota- and width-matched replay retained no complete state, but it did separate the structural mechanism from the schedule. Comparator retention reduced the initializer from 25 to 15 inactive pieces, while contact-signature retention reached 13 under the same 40 layers, beam width, and per-parent schedule. Realized geometry work differed after the retained populations diverged and is reported separately. The treatment therefore supports persistent topology diversity, but the late trace exposed a selector defect: layers 25 through 39 generated 1,462 ejection children and only six direct insertions, while the selected-piece stream repeatedly revisited the same 70 x 60 family and never selected several surviving 100 x 40, 40 x 100, and 30 x 30 pieces. Layer 39 alone generated 83 ejection children and zero direct insertions. Extending the same schedule would spend more work on a demonstrated churn cycle. The same-build raw and normalized identities, counters, timings, resident memory, and terminal partial identities are pinned in `persistent-vacancy-initial-evidence.json`; that file, rather than temporary process output, is the evidence boundary for this premise.

A second causal screen changes only the selected-piece scheduler. Modes `1` and `2` retain the reviewed two-hardest scheduler and comparator/contact-signature policies. Modes `3` and `4` use the same retention policies respectively, but replace the second hard slot with one deterministic coverage slot. This creates two paired comparisons: `1` versus `3` isolates scheduling under comparator retention, and `2` versus `4` isolates scheduling under structural retention. All four modes retain two selected-piece slots per parent and every downstream orientation, position, shortlist, exact-row, validation, memory, and publication quota remains identical.

For the stateless rotating scheduler, sort the current inactive pieces by ascending stable ID. Slot zero remains the first piece under the reviewed descending difficulty key. Slot one starts at `layer mod inactiveCount` in the stable-ID order and selects the first cyclic entry that is not slot zero. A one-inactive-piece state produces only slot zero. Population ordinals, retention mode, parent hash, and diagnostic history do not enter this choice. A physical state shared by any two arms at the same layer therefore receives byte-identical selected pieces and successor streams within the same scheduler family. For an unchanged inactive set of size `n > 1`, every non-hard entry is selected by slot one within at most `n` consecutive layers. Because insertions and ejections change that set, this is intentionally a stateless rotating-selector screen, not a global bounded-coverage claim. It adds no probe, lookahead, mutable age, or work.

For every expanded parent, diagnostics record an ordered row containing the layer, parent state fingerprint, ordered inactive-ID hash, scheduler family, slot-zero ID, rotation start index, slot-one ID when present, transition seed, and each slot's selected ordinal, piece ID, angle seed, and diversity seed. The row order is the comparator order of the retained parent population; the parent fingerprint makes that position nonbehavioral. These rows must prove that a shared physical parent in modes `1` and `2`, or in modes `3` and `4`, receives identical ordered selected pieces and downstream seeds. Focused tests cover cross-arm equality, inactive-storage permutation, a single inactive piece, cyclic skipping of slot zero, and the fixed-set `n`-layer bound.

Interpret the screen in stages. First, modes `3` and `4` must replay deterministically, remain under the existing time and memory gates, preserve the mode-zero compatibility hash, and reproduce contemporaneous modes `1` and `2` byte-for-byte after the declared normalization. Mode `3` must beat mode `1`, and mode `4` must beat mode `2`, with neither member capped, incomplete, or validation-disagreeing. A paired win is either unique exact-valid completion, strictly lower independently validated complete depth when both complete, or—when neither completes—a comparator-best partial with strictly lower inactive area and no more inactive pieces. If a rotating mode only exchanges which IDs churn without improving that paired objective, this stateless rotating selector is closed; selector coverage in general is not. If it improves the partial but does not complete, record the residual family distribution before changing candidate geometry; neither extra layers nor a larger beam is authorized by this screen.

The completed screen is pinned in `persistent-vacancy-selector-evidence.json`. All four arms replayed byte-identically after timing normalization, the old semantic modes reproduced their frozen hashes, the then-used legacy commit-bound mode-zero projection remained `0f39c64...`, and shared-parent selector rows had zero mismatches. Mode `3` passed its paired partial gate, improving mode `1` from 15 inactive pieces / `69619646821` inactive grid-area units to 12 / `60144097737`. Mode `4` failed its paired gate: it tied mode `2` at 13 inactive pieces but regressed inactive grid area from `58797801045` to `62614709968`. Every arm remained below 5.81 seconds wall time and 48.4 MiB RSS. Stateless rotation is therefore retained as a useful scheduler under comparator retention, while its direct combination with first-contact-signature reservation is closed. The next screen must preserve the complementary low-count and low-area basins explicitly; it may not spend more work on the same single global ordering.

## Follow-up dual-objective elite screen

Mode `3` exposed a loss-of-incumbent defect independently of the contact-signature result. Its area-first reported state reached 11 inactive pieces at layer 29, improved that reported basin to `59571041296` inactive grid-area units at layer 32, reported 11 through layer 35, then compulsory successor transitions lost that basin and the run finished with 12 inactive pieces / `60144097737`. These exact layer identities are now pinned under `observedAreaFirstHistory` in `persistent-vacancy-selector-evidence.json`. They do not establish the minimum-count state among all eight retained parents because the first screen recorded only `next[0]`. The population stores no unchanged parent and no best-ever partial. This is not an argument for more layers: the current lifecycle can destroy a better reported state before later scheduler phases can explore it.

Implementation therefore has an observational stage before mode `5` is interpreted. Every mode computes, but modes `1` through `4` ignore behaviorally, both the area-first and count-first elites over the full retained population at every layer. A nested `elite` diagnostic records their objectives and fingerprints plus best-ever values. The count-first comparator is exactly `(inactiveCount, inactiveArea, inactiveDifficultySequence, ejectedMaterialArea, ejectedCount, activeFrontier, stateIdentity)`. Rerun mode `3` twice after this instrumentation and pin its contemporaneous true count-first and area-first histories; the mode-`5` gate is derived from those records, not from the old 11-piece assumption.

Modes `5` and `6` share one dual-objective retention policy. Reserve the best area-first child, then the best count-first child if its identity differs, then fill remaining slots from the ordinary area-first order. Mode `6` applies this policy only to ordinary generated children. Mode `5` additionally identifies two incumbents before expansion in the current eight-state population—the first under each comparator—and, after ordinary child generation and complete publication, adds clones of those at most two distinct partial incumbents to the partial retention pool. They consume no orientation, position, hazard, exact-row, pair, or validator work. Deduplicate the combined pool by the existing semantic identity.

Both modes retain exactly eight states; the elite reservations occupy existing beam slots and do not widen the population. Because a mode-`5` unchanged incumbent is expanded again at a later layer, its stateless rotation slot and all layer-derived seeds change under the existing deterministic contract, so preservation explores a new neighborhood rather than repeating the same successor stream. The state and last-transition identities remain unchanged; no archive age or hidden history enters behavior.

Diagnostics record the two offered carryover fingerprints, whether they were distinct, the final reserved area/count fingerprints and objective values, the number of retained slots filled by carryovers, and whether each retained carryover fingerprint appears in the next layer's ordered expanded-parent rows. Before carryover injection, modes `5` and `6` compute a canonical hash over the comparator-ordered, deduplicated ordinary child identities with an explicit complete/partial byte, a separate complete-candidate order hash, the entering-population hash, and a snapshot of every generator, geometry, validator, and audit work counter. Whenever any paired arms enter a layer with the same population hash, these pre-carryover hashes and counter snapshots must be identical.

Memory accounting preflights the simultaneous live set consisting of the entering population, every generated ordinary child, both incremental carryover clones, and all existing plus pending elite/selector diagnostics before either deduplication or layer commit. Clones later removed as duplicates still count at this peak. State vectors, `Arc` handles, and conservatively repeated underlying collision geometry use the existing state-memory estimator; nested elite diagnostic capacities enter `selectorDiagnosticPeakBytes`; their simultaneous sum updates and is capped through `totalRetainedPeakBytes`. A cap failure discards the uncommitted carryover pool and layer. Focused tests prove monotonic preservation of both objectives, deduplication when both elites are one state, width eight, deterministic tie-breaking, distinct later-layer streams for a preserved state, pre-carryover equality for shared populations, and atomic live-pool memory failure.

First run the instrumented mode `3` twice and pin its true best-ever area-first and count-first diagnostics. Compatibility with the pre-elite mode-`3` evidence uses the ordinary timing normalization followed by deletion of `engineCommit`, dirty status, executable/source hashes, every additive `layers[].elite` object, and the additive `selectorDiagnosticPeakBytes` and `totalRetainedPeakBytes`; the expected semantic hash is `72c959094e6a1f0f76d139e798167775c62909a0a8dc3e0785c665751dd10f3d`. Provenance and full memory counters remain mandatory and equal before this semantic projection.

Then use one unmeasured release build for two paired screens. Run `MODE=3,6,6,3` to isolate dual-objective retention among ordinary children, followed by `MODE=6,5,5,6` to isolate incumbent carryover under the identical dual-objective policy. Same-mode normalized replays must be byte-identical, contemporaneous controls must match fully, all processes must share provenance, and mode zero must retain the protected hash. Mode `6` beats mode `3`, and mode `5` beats mode `6`, only by unique exact-valid completion, strictly lower independently validated complete depth, or—if neither completes—final area-elite inactive area no greater than the paired control's best-ever area-first value and final count-elite inactive count no greater than its best-ever count-first value, with at least one strict improvement and no regression in the other elite's corresponding objective.

Mode `5` additionally requires at least one offered carryover to survive retention and later appear as an expanded parent; otherwise it must match mode `6` and cannot claim a carryover result. Whenever the paired arms share an entering-population hash, their ordinary child/order hashes and work snapshots must match. A cap, audit disagreement, width change, shared-population pre-carryover mismatch, wall time over 6.5 seconds, RSS over 64 MiB, or mere preservation without strict progress rejects the relevant arm. Failure of mode `6` closes this dual-objective reservation policy. A mode-`6` win followed by mode-`5` failure closes this two-elite carryover policy while retaining the multiobjective result.

The completed screen is pinned in `persistent-vacancy-elite-evidence.json`. After Cargo reached terminal completion, one unchanged release executable ran the required `MODE=3,6,6,3` sequence. Same-mode replays were byte-identical after timing normalization, mode `3` reproduced the pre-elite semantic hash, and all three shared entering populations had identical ordinary-child hashes, complete-candidate hashes, and pre-carryover work. Mode `3` retained a best-ever state with 11 inactive pieces and `59571041296` inactive grid-area units. Mode `6` regressed both best-ever objectives to 13 inactive pieces and `64577591268` units at essentially equal runtime and memory. All four engine samples also exceeded the provisional `6000 ms` diagnostic gate by `27-138 ms`, while the `6.5 s` wall and `64 MiB` RSS gates passed. Mode `6` is rejected. Mode `5` was deliberately not run because its carryover treatment depends on the failed reservation policy and would not isolate a useful causal variable. It remains untested rather than causally closed; carryover may be reconsidered under a retention lifecycle that does not reserve permanent beam slots. Persistent exact-valid partial populations and the stateless rotating scheduler remain supported.

## Rejected stagnation-archive screen

The completed mode-`7` screen is summarized in `persistent-vacancy-archive-evidence.json`. It kept the mode-`3` generator, scheduler, width, and ordinary retention unchanged, but after three stagnant layers replaced the eighth beam slot for one generation with one never-before-revived member of a bounded 16-state archive. The mechanism was active: four distinct archived states survived retention and were expanded. It nevertheless regressed the best-ever partial from mode `3`'s 11 inactive pieces / `59571041296` inactive grid-area units to 13 / `65555025691`. The first revival preceded the treatment's divergence from the control, and the treatment then missed the control's layer-23-through-29 gains. Both arms passed exact-validity, width, cap, wall-time, and RSS gates; both missed the provisional `6000 ms` engine diagnostic gate.

No stagnation-threshold retuning was run after observing this fixture, because that would be post-hoc tuning rather than a causal screen. The evidence also records that the experimental implementation undercounted archive-only bookkeeping: persistent revived-fingerprint strings, a pending fingerprint string, transient fingerprint/signature sets, and archive hash/signature pair scans were absent from retained-memory or work counters. That limitation does not rescue the quality regression, but it prevents using the recorded incremental accounting as a complete cost claim. The exact dirty patch, executable bytes, raw output bytes, and complete toolchain identity were not retained before source removal, so this record is explicitly a non-reproducible historical summary rather than a pinned replay oracle. Mode `7` was removed from source. This result closes only the tested three-layer, one-slot archive-revival policy; it does not close archives in general or a bounded nonterminal ruin/recreate lifecycle that reconstructs and accepts a whole candidate without sacrificing a productive beam lineage merely to try it.

## Bounded macro-successor screen

Modes `8` and `9` preserve mode `3`'s ordinary population, stateless rotating selector, generator, comparator, and eight-state retention. After ordinary children are sorted and deduplicated, both modes select the comparator-best incomplete ordinary child and expand it once more with the same bounded transition operator at the same layer. The child identity changes the seed, so the second transition has its own deterministic stream without adding a hidden random phase. The selected intermediate would normally survive mode `3`'s beam, so this screen isolates same-layer depth-two lookahead and admission rather than claiming rescue of a pruned state.

Both modes clone the ordinary pool, append the macro children, charge the raw simultaneous ordinary/macro/combined state and diagnostic allocations, sort and deduplicate the combined shadow pool, and run every combined complete candidate through the same publication-audit schedule. Mode `8` then ignores the shadow result and retains from the untouched ordinary pool; a valid macro-only complete candidate is diagnosed but cannot terminate the control. Mode `9` uses the combined pool for the unchanged mode-`3` retention. Both modes record the macro parent fingerprint, selected-piece row, child-order hash, generated count, fingerprints absent from the ordinary pool, admitted count, retained novel fingerprints, insertion classes, and exact work delta. `admittedChildren` counts only macro identities absent from the ordinary pool, and `retainedChildFingerprints` proves whether those identities survived the beam. Existing `ordinaryChildOrderHash` continues to describe only the untouched ordinary child stream.

The paired sequence is `MODE=8,9,9,8` from one unmeasured release executable and the same frozen parent. When entering-population hashes match, ordinary child hashes and macro parent, selection, child hash, and work must match exactly; only `admittedChildren` may differ. Mode `8` must reproduce mode `3`'s population and best-state history after deleting the additive macro diagnostic and macro-only work. Mode `9` earns another iteration only if it completes the exact-valid 165 mm strip uniquely or improves the best-ever partial in one objective without regressing the other: fewer inactive pieces with no greater inactive area, or lower inactive area with no more inactive pieces. Exact completion at 165 mm is an architectural milestone, not the final quality target; the broader engine still aims below 160 mm and eventually toward the approximately 150 mm external reference. Any cap, validation disagreement, causal mismatch, RSS above 64 MiB, or wall time above 7 seconds rejects the arm. The 7-second bound is specific to this diagnostic screen and does not relax production runtime gates.

The reviewed screen is pinned in `persistent-vacancy-macro-evidence.json`. Mode `8` reproduced mode `3`'s full population history. Mode `9` retained 126 novel macro states across 34 layers and improved the best-ever partial at the same 11-inactive-piece count from `59571041296` to `53755610030` inactive grid-area units, a 9.7622% reduction. Its final partial also improved from `60144097737` to `56276626808`. Two clean-commit replays per arm were byte-identical after timing normalization, all shared-population causal rows matched, all caps and audits passed, wall time stayed within `6.07-6.39 s`, and peak RSS stayed below 47.5 MB. No arm completed the 165 mm strip. The result retains same-layer depth-two lookahead for another iteration, but does not promote a production engine or prove rescue of a state that ordinary beam pruning would discard.

## Preserved-best macro-parent screen

Mode `9` reaches its best observed area basin at layer 35 with 11 inactive pieces and `53755610030` inactive grid-area units, then compulsory successor transitions lose that state before the terminal layer. Mode `10` tests whether the existing single macro expansion can recover useful work from that pruned topology without widening the beam or adding another parent expansion. It stores one exact-valid best-ever area state outside the ordinary beam. When that state is absent from the current ordinary child pool, it becomes the next layer's macro parent; otherwise mode `10` expands the same comparator-best ordinary child as mode `9`. The macro children enter the same combined retention pool in both modes.

The sidecar is state, not hidden search history. It is updated only when the existing area comparator reports a strict improvement, uses the same semantic identity and deterministic parent seed as every ordinary state, and occupies no ordinary beam slot. Its complete owned state and every transient clone are charged before allocation against the existing retained-memory cap. Modes `8` and `9` omit the additive parent-origin fields, preserving their serialized diagnostic shape. Mode `10` records `ordinaryBest` or `bestEverArea` plus whether the preserved state was absent from the ordinary pool. Nonterminal modes `8`, `9`, and `10` must all retain exactly eight states.

Run `MODE=9,10,10,9` from one unmeasured release executable and the same committed parent fixture. Same-mode normalized replays must be byte-identical. Each mode must use exactly one macro parent expansion in every eligible layer, and mode `10` may not increase selected-piece slots, generator quotas, exact-row quotas, or layer count relative to its own realized trajectory. At least one layer must expand an absent `bestEverArea` parent or the treatment is inactive. Mode `10` passes only by exact-valid completion of the 165 mm strip or by a strict best-ever partial improvement under the existing paired rule: fewer inactive pieces with no greater inactive area, or lower inactive area with no more inactive pieces. Any cap, width change, audit disagreement, nondeterminism, wall time above 7 seconds, or RSS above 64 MiB rejects it. A pass supports a bounded nonterminal incumbent lifecycle; a failure closes only this single preserved-area-parent policy, not the broader possibility of topology-aware macro parent selection.

The completed screen is pinned in `persistent-vacancy-preserved-best-evidence.json`. From clean commit `5dcd8ca`, both same-mode replays were byte-identical after timing normalization, and every output retained the full rustc identity while the evidence retained exact rustc and Cargo captures. Mode `10` first used an absent preserved parent at layer 1 and did so in 39 layers; the pre-treatment macro stream and the first treatment layer's ordinary stream matched mode `9`. Both arms used 706 selected-piece slots, 8,472 orientation streams, 5,648 exact finalist rows, 41 partial audits, and zero complete audits. Other geometry-dependent counters differed after the trajectories diverged and are reported in full rather than described as equal. Mode `10` retained 32 novel macro states and improved the best-ever partial at the same 11-inactive-piece count from `53755610030` to `47975977789` inactive grid-area units, a 10.7517% reduction. Its final area elite equals that best-ever state, whereas mode `9` finishes with 12 inactive pieces. All four runs stayed within `6.18-6.36 s` wall time and below 47.1 MB RSS. No arm completed the 165 mm strip, so the preserved-best macro-parent lifecycle is retained while production promotion and sub-160 mm quality remain unproven.

## Rejected preserved-count macro-parent screen

This section is a non-reproducible historical summary from an uncommitted exploratory build. The repository does not retain its patch, executable, raw outputs, or complete toolchain capture, so its hashes must not be used as replay or portable benchmark evidence. An exploratory mode `11` retained both best-area and best-count sidecars and redirected the existing single macro expansion to an absent lower-count sidecar. The ungated form first forked at layer 28 on a merely one-piece-better state and regressed the terminal partial to 12 inactive pieces with `53386915399` inactive grid-area units. A second form allowed the count sidecar to take the macro slot only during the final eighth of the 40-layer budget. It preserved the mode-`10` trajectory through layer 36 and first expanded the pinned 10-piece state `1b2fd098813d00f01067e2ea95ad494c41b5126cc48da0df26c147bf6601c0df` at layer 37.

The paired exploratory order was `10,11,11,10`. Timing-normalized outputs were byte-identical within each mode. Both controls reproduced semantic trajectory SHA-256 `1edb02e2fcacfa5c3d749cb228eee735744171f5c25993c09daa9cd8054b7709`; both late-recovery runs reproduced semantic trajectory SHA-256 `c6331da7b36173e6855c26f160bcfdeb244464ff57d9b0356b0dcf380b45a476`. At the fork, entering population and ordinary child order matched. No exact ordinary-work equality is claimed: the available `preCarryoverWork` snapshot occurs after macro-dependent complete-candidate audits, and the rejected arm did not add a dedicated pre-macro counter. The two mode-`11` runs took 6.28 and 6.30 seconds wall time and peaked at 46.7 and 47.7 MB RSS.

The late treatment still left 10 pieces inactive. It changed the best-count inactive area from `50292939713` to `50292864396` grid-area units and the best-area inactive area from `47975977789` to `47975981677`, neither a completion nor a strict count improvement. It therefore failed the predeclared completion-oriented gate. The final-eighth threshold was also selected after observing the layer-35 basin, so it cannot support an independent general-policy claim. Mode `11` and its extra sidecar machinery were removed rather than retained as dead experimental surface. The result closes these single-parent count-redirection policies; it does not close topology archives, explicit multiobjective population retention, or a separately justified phase schedule evaluated on independent instances.

## Rejected complementary macro-parent admission screen

The next causal screen kept mode `10`'s primary preserved-area macro expansion unchanged and added an optional second expansion from the strict best-count sidecar. Mode `12` performed the full supplementary generation, exact scoring, deduplication, common completion audit, and diagnostics but discarded supplementary-only identities. Mode `13` admitted those identities into the existing area-first beam. Only these two modes received quotas derived from at most ten parent expansions per layer; all earlier modes kept their original ceilings. A dedicated pre-supplementary snapshot isolated ordinary and primary-macro work, and complete sidecar plus transient-pool accounting covered the simultaneous owned state. The reviewed implementation and runnable experiment remain at clean commit `6b2d1e061b150bbb952819cd2821b8878b5275e9`; the rejected modes are absent from the current source.

One release executable ran the paired order `10,12,13,13,12,10` from the committed platform-independent parent fixture. Same-mode normalized outputs were identical. Both mode-`10` controls reproduced semantic trajectory SHA-256 `1edb02e2fcacfa5c3d749cb228eee735744171f5c25993c09daa9cd8054b7709`; both mode-`12` controls reproduced mode `10`'s behavioral-history SHA-256 `b459ac24ec09b00325b623ebb67013b176cde9cbf48e5d75401930fcf80ca037` while exercising seven supplementary expansions and admitting none. The treatment crossed its causal boundary and retained seven supplementary-only identities. Ordinary children, the primary macro stream, pre-supplementary work, the supplementary stream, and final shadow-pool order matched through the first admission. All arms completed forty eight-state layers without cap or audit failure, stayed within `6.32-6.75 s` wall time, and peaked below 49.5 MB RSS.

Mode `13` nevertheless regressed both objectives. Its best-area partial had 13 inactive pieces / `55908065396` inactive grid-area units versus mode `10`'s 11 / `47975977789`; its best-count partial had 12 inactive pieces versus mode `10`'s 10. It produced no complete state. The negative result is pinned in `persistent-vacancy-complementary-evidence.json`. It closes direct admission of a separately expanded lower-count incumbent into the existing area-first beam. It does not close the broader lifecycle: the treatment proved that the secondary basin cheaply generates novel exact-valid topology, making an independent nonterminal ruin-and-recreate acceptance path one possible next test instead of displacing productive beam lineages immediately.

## Exploratory contraction and churn ablations

After restoring the current mode-`10` source exactly, four uncommitted exploratory ablations tested whether a smaller contraction step or a wider version of the existing lifecycle could remove the terminal deficit. These observations are design evidence only: their temporary executables and raw outputs were not retained, so their timings and intermediate identities are not replay or promotion evidence.

Moving the target from `165.000 mm` to `167.500`, `168.000`, `168.250`, and finally `168.350 mm` did not complete the layout. Even `168.350 mm`, only `0.011 mm` below the independently rebuilt `168.361 mm` frozen source depth, deactivated 14 pieces at initialization and recovered at most eight of them. Changing retention to count-first reduced the best observed inactive count to six but did not complete and did not retain that state terminally. Raising the exact blocker ceiling from two to four produced more ejection transitions without a retained improvement. Doubling the lifecycle from 40 to 80 layers likewise produced no improvement after layer 32; the additional layers continued the same churn plateau.

The trace motivates a structural hypothesis rather than proving it. A transition can insert inactive piece `A` by ejecting blockers `B` and `C`, but the following layer returns to the global hardest-plus-rotating selector instead of prioritizing `B` or `C`. The current so-called depth-two macro therefore may be interrupting the repair obligation created by a ruin step. The next controlled screen keeps the productive mode-`10` lifecycle unchanged and compares two separately bounded private repair expeditions whose intermediate states cannot displace the main beam.

## Proposed displaced-piece-first repair screen

Modes `14` and `15` are reserved for one paired post-lifecycle scheduler screen. Both modes execute mode `10`'s 40 ordinary layers and primary preserved-area macro policy unchanged before starting a private expedition. They retain the exact best-count state as a second, deliberately independent owner rather than aliasing the best-area owner. Each sidecar update is preflighted as one atomic allocation plan before either owned state changes. A contemporaneous mode-`10` control is mandatory. Modes `10`, `14`, and `15` record `preExpeditionWork` immediately after layer 39 with its three memory-peak fields zeroed and record `preExpeditionBehaviorHash` over the exact projection below. Modes `14` and `15` must be fully identical without projection until their first augmented-queue difference.

```jq
.relaxedDiagnostics.coupledDynamicSeparator.persistentVacancyPopulation
| {
    seedDomain,
    targetDepthMm,
    parentFingerprint,
    initialStateFingerprint,
    initialActivePieceIds,
    initialInactivePieceIds,
    initialInactiveOrderHash,
    layersCompleted,
    directInsertions,
    ejectionInsertions,
    immediateReversalsRejected,
    deduplicatedStates,
    distinctSignaturesRetained,
    completeStates,
    publicationRejections,
    preExpeditionWork,
    layers
  }
```

`preExpeditionBehaviorHash` is SHA-256 of the compact `serde_json` byte serialization of that field-ordered record, prefixed by ASCII `persistent-vacancy-pre-expedition-v1\0`. No other deletion, timing normalization, or optional diagnostics enter this digest. The full result still retains the additive sidecar-memory peaks, private expedition record, total work, and final outcome; they are never removed from same-mode replay comparisons.

Every layer record adds `retainedPopulationHash`, computed from the comparator-ordered `next` population after every retention, carryover, macro-admission, and completion decision and before it becomes the following layer's parent pool. The existing `enteringPopulationHash` and the new retained hash therefore prove both sides of all 40 population transitions, including layer 39's final retained population. `layers`, and therefore `preExpeditionBehaviorHash`, include both hashes.

After layer 39, both arms start from that exact best-count sidecar. An augmented expedition node owns a vacancy state and an ordered queue containing every currently inactive piece exactly once. Its behavioral identity is the current semantic state identity, including `lastTransition`, plus the ordered queue. There is no path-dependent rejection rule. Instead, one deterministic expedition-wide transposition set prevents an augmented identity from being expanded twice; the first occurrence in comparator order wins. The existing immediate-reversal predicate remains behavioral because `lastTransition` is part of the augmented identity.

The private search has a fixed horizon of 16 expanded levels and width four. Expansion depths are `0..15`; generated endpoint depths are `1..16`. Depth zero expands the root, and each later depth expands at most four retained incomplete nodes, for at most `1 + 15 * 4 = 61` parent/piece expansions. Each expansion uses the existing immutable baseline hints, 12-orientation stream, 32 positions per orientation, proxy ordering, eight exact finalists, boundary check, and one-or-two exact blocker rule. It differs from the ordinary generator only in receiving one explicit piece instead of invoking the two-slot scheduler.

Mode `14` is an explicit one-slot global-hardest control, not the existing two-slot scheduler: before each expansion it rebuilds the full queue by descending difficulty and stable ID and expands only its first piece. This predeclared projection is used because one repair-chain step must have exactly one obligation and both arms must spend one selected-piece slot per expanded node. Mode `15` is the displaced-first treatment. Its root queue is identical and therefore expands the same first piece. After a direct insertion it removes that head. After an ejection it removes the inserted head and prepends the newly displaced blockers in descending difficulty order, followed by the remaining queue. Duplicate or missing inactive IDs reject the child. The causal claim is correspondingly narrow: it tests global-hardest versus displaced-first selection inside the same private one-slot search, not equivalence to mode `10`'s two-slot scheduler. The causal boundary is the first full augmented-queue difference, because queue tails participate in deduplication and tie-breaking even when the selected heads still match.

The expedition seed domain is `persistent-vacancy-repair-expedition-v1`. Generator parent keys hash only the semantic vacancy state, including `lastTransition`, plus the selected head; they deliberately exclude the remaining queue. Orientation, position, and diversity seeds derive from that key with fixed private ordinals rather than endpoint depth. Therefore the same physical state and selected piece receive one generator stream even when queue tails or endpoint depths differ. The full queue remains part of augmented identity, transposition, and final tie-breaking but cannot perturb proposal generation. Cross-arm rows with the same semantic state and selected head must have byte-identical seeds, proposals, exact rows, and work deltas.

Each expanded parent records its augmented-identity hash, semantic-state fingerprint, full entering queue, selected stable ID, transition/angle/diversity seeds, the comparator-ordered proposal hash, the exact-finalist-row hash, the generated-child-order hash, and its exact work delta. Proposal and exact-row hashes encode only canonical placement keys, orientation ordinals, and diversity keys; floating proxy magnitudes do not enter the replay oracle after their ordering decision. The paired harness compares the complete root row before queue admission, proves the first retained augmented-frontier divergence, and compares every later cross-arm semantic-state/selected-head row after removing only queue identity. This makes the claimed scheduler boundary observable rather than inferring it from depth aggregates.

Both arms deduplicate by the full augmented identity and rank partial nodes with the existing `compareCountStates` ordering unchanged, appending the ordered queue only after every existing tie-breaker. The width-four frontier can therefore retain temporary worsening relative to the root. Complete nodes have empty queues; they are removed from the expandable frontier, publication-audited, and retained separately as terminal candidates. Remaining incomplete nodes continue to the fixed horizon. An empty frontier produces deterministic recorded no-op depths through endpoint depth 16. Valid completions rank by lower independently measured depth, then placement fingerprint.

Expedition semantic counters are staged separately from the protected lifecycle, and successful work uses a private ledger. On success, the global semantic counters, work ledger, and complete `repairExpedition` subtree commit together. On failure, semantic mutations, partial endpoints, and incomplete depth records remain rolled back, while a failure-only `repairExpedition` record owns the reason, cap classification, and all staged work and memory already consumed. Memory is preflighted before root ownership, every depth allocation, and every depth-diagnostic publication. The conservative reservation includes live node owners at the maximum possible active-collision ownership, all node-vector backing allocations, the expedition-wide ordered transposition set including tree-node overhead, comparator queue keys, terminal clones, every root/depth/parent/frontier diagnostic and scalar string, and the actual maximum stable-ID byte length rather than a fixed ID guess.

At every depth, record generated and deduplicated node counts, ordered frontier augmented identities, queues, best inactive count and area, direct/ejection counts, and a work-counter delta. The root and comparator-best incomplete node at every generated endpoint depth receive the same dual exact/source partial audit used by the ordinary lifecycle, for at most 17 extra partial audits. Every deduplicated complete candidate receives the dual publication audit in deterministic order before the next depth commits. The 488 extra exact-finalist rows are also the absolute extra complete-audit bound; attempting a 489th rejects the arm before either validator runs. Audited complete nodes never re-enter the frontier. The two arms have equal width, horizon, per-parent generator schedule, and nontransferable ceilings, but realized work may diverge after their queues select different physical parents.

Each arm reports its best valid complete endpoint, otherwise its comparator-best exact-valid partial seen anywhere in the expedition. A partial endpoint counts as an improvement only when it Pareto-dominates that arm's common root: inactive count and inactive area are both no worse and at least one is strictly better. It remains diagnostic-only and cannot replace or reorder any completed mode-`10` layer. A complete endpoint becomes that arm's experimental result. Treatment support requires mode `15` to beat both the common root and mode `14`; production promotion still requires 61/61, all publication gates, and a strict depth win over the protected operational result.

The mode-specific combined worst case is 781 selected-piece slots, 9,372 orientation streams, 799,744 source-feature visits, 4,957,788 position-source attempts, 299,904 returned positions and hazard queries, 18,294,144 proxy-pressure visits, 6,248 exact finalist rows, 15,681 experimental collision builds, 376,710 experimental pair visits, 58 partial audits, and 552 complete audits. The resulting 610 dual validator invocations fund 74,420 validator collision builds and 2,232,600 validator pair visits. The aggregate ceilings are 46,131,712 transformed collision vertices and 2,671,933,440 Clipper input vertices; the existing post-operation Clipper-output cap remains an independent atomic ceiling rather than a promise that all theoretical outputs fit. The protected 40-layer phase runs under mode `10`'s original smaller ceilings. Only the private staged ledger receives the combined ceilings after the pre-expedition digest is frozen, so unused repair headroom cannot rescue or alter the protected phase. Formula tests cover every independently enforced serialized counter.

Before allocating, cloning, reserving, or committing any sidecar, augmented node, queue, transposition identity, raw child vector, retained frontier, endpoint, audit clone, or pending diagnostic vector, preflight its full simultaneous capacity against the 64 MiB experiment-owned cap. Work and memory already consumed remain diagnostic on failure, but an incomplete expedition, endpoint, depth record, or semantic sidecar update commits nothing. Earlier modes retain their existing smaller ceilings. Modes `14` and `15` expose the ordinary pre-expedition snapshot and all private work separately. Same-arm population diagnostics, including all three deterministic memory peaks, must be byte-identical; only top-level measured timing/RSS fields lie outside that comparison. Cross-arm equality is required only through the causal boundary and for shared-parent rows thereafter.

The completed clean screen is pinned in `persistent-vacancy-repair-evidence.json`. One unchanged executable from commit `7043142` ran `10,14,15,15,14,10`; all arms shared pre-expedition behavior SHA-256 `a004394...`, and both repetitions of each mode reproduced the complete canonical population subtree byte-for-byte, including deterministic memory peaks. The common exact/source-valid root had 10 inactive pieces and `50292939713` inactive grid-area units. Mode `14` ended at 10 / `50292855011`; displaced-first mode `15` ended at 10 / `45454946952`, a 9.619% area reduction relative to its same-work scheduler control. Every run stayed below 6.63 seconds wall time and 49.2 MB RSS, with no cap, publication, or audit failure. The treatment therefore supports obligation-following repair as an architectural primitive, but neither arm completed the 165 mm strip. A single width-four, horizon-sixteen terminal expedition is closed as a completion mechanism; the next controlled change should preserve displaced-first scheduling while testing a bounded multi-round Pareto repair/restart lifecycle.

## Two-round repair restart screen

Modes `16`, `17`, and `18` run the exact mode-`15` private repair round first, then one fresh width-four, horizon-sixteen round with a new transposition set. Activation requires the complete round-zero repair subtree to serialize to SHA-256 `2350b92068d9aa71575db53aa25bd6b04984bd551d02e1ecc7e292692feec86d` and its comparator-best audited endpoint to match state fingerprint `bed29b45996a6bcccc5dad8f498f7522711976bd6d26e26ae78da18d99935da1`, augmented queue identity `819e0fb0ee3dfab5806359ddbc86e3ad695ac2a65928d5bbc902849fb1f8ef33`, 10 inactive pieces, and `45454946952` inactive grid-area units. This prevents an upstream or platform-dependent trajectory from silently changing the restart premise.

Mode `16` reseeds the original best-count root and rebuilds its global difficulty queue. Mode `17` continues from the round-zero endpoint but rebuilds that endpoint's queue globally. Mode `18` continues from the same state and preserves its displaced-first queue. The mini-factorial isolates state continuation through `16` versus `17` and queue continuity through `17` versus `18`; each root queue is an actual expedition input, not a diagnostic projection. The arm endpoint is the count-comparator minimum over the original root, round-zero endpoint, and round-one endpoint after exact partial audits. Round-zero and round-one semantic events and work commit together only when both rounds succeed. A failure publishes the failed round, reason or cap, consumed work and peaks, the round-zero activation identities when available, and a constructed round-one root when available, but no uncommitted round or endpoint subtree.

Round-one transition seeds are SHA-256 over ASCII `persistent-vacancy-repair-restart-v1`, one zero byte, the unsigned big-endian `u32` round ordinal, the existing 32-byte semantic state digest, and the selected stable ID framed by its unsigned big-endian `u32` UTF-8 length. The first eight digest bytes interpreted as an unsigned big-endian `u64` are the generator seed. Queue tails, endpoint depth, mode, counters, work, and input ordinal do not enter it. The same semantic state and selected head therefore retain one stream while the three roots can deliberately choose different heads.

The two-round worst-case ceilings are computed in `u64` and checked before conversion to `usize`: 842 selected slots, 10,104 orientation streams, 6,736 exact-finalist rows, 75 partial audits, 1,040 complete audits, 78,300,672 transformed collision vertices, and 4,594,575,360 Clipper input vertices. The existing full per-round reservation remains intact. Before allocating restart roots, queues, or diagnostic strings, an additive preflight reserves the retained round-zero diagnostic, all cross-round node owners, the pending root and comparison diagnostics, failure strings, and serialization scratch under the 64 MiB experiment-owned cap. The screen rejects any cap or audit disagreement, wall time above 7.5 seconds, OS RSS above 64 MiB, replay mismatch, or change to the pinned pre-expedition behavior hash. A treatment earns architectural support through exact completion or a strict comparator improvement over its paired control. The size and operational relevance of any passing effect are reported separately; this screen has no retrospective materiality threshold.

The completed screen is pinned in `persistent-vacancy-restart-evidence.json`. One clean executable from commit `6b3806f` ran `16,17,18,18,17,16`; both repetitions of every mode reproduced the full canonical population subtree byte-for-byte, all six runs completed without cap or audit failure, wall time stayed within `6.59-6.92 s`, and peak RSS stayed below 49.8 MB. Mode `16` returned the round-zero endpoint. Mode `17` continued the same exact-valid state with a rebuilt queue and passed the declared strict comparator gate: inactive doubled grid area improved by `41205`, or `0.0206025 mm²`, at the same ten-piece inactive count. That effect is formally positive but practically negligible relative to the still-missing completion, so another identical terminal restart is not prioritized. Mode `18` selected the preserved displaced-first head in the actual generator but returned the unchanged round-zero endpoint, rejecting queue continuity as the treatment in this lifecycle.

## Proposed vacancy-topology predictive screen

The next step is a predictive measurement, not a causal claim or a new mover. The fixed primary signal is selected-piece top-reachable configuration-space area, with larger area hypothesized to be better. It has no tunable clearance. For the next selected inactive piece, use the existing fixed 12-orientation stream. At each orientation, build the exact integer-grid domain of valid reference-point positions: erode the target-strip boundary by that oriented collision polygon and subtract the union of exact no-fit regions against every active collision polygon. Sum the doubled grid area of components whose closed boundary has positive-width contact with the removable top frontier across the 12 separate orientation layers. Point-only contacts do not count. The selected stable ID, orientation keys, no-fit input/output vertex counts, per-orientation component areas, and exact sum are retained. Raw free-space clearance snapshots, disconnected area, component count, and contact length remain secondary diagnostics and cannot select the result.

The measurement budget is nontransferable and specific to this Mixed-61 screen. Each of the three pinned trajectories visits at most its first 48 parent rows. A parent exposes only the first identity-ordered sibling pair that satisfies the comparator-prefix match before either signal is computed; no later pair substitutes for equality, agreement, geometry failure, or work mismatch. The screen therefore measures at most 144 pairs and 288 sibling signals. Each signal has exactly 12 orientation layers and at most 61 active-piece no-fit rows: at most 732 no-fit builds, 12 sheet erosions, 12 no-fit unions, and 12 final differences. A no-fit or sheet-erosion operation accepts at most 64 total input vertices and 512 output vertices. Per orientation, the no-fit union accepts at most 31,232 input and 8,192 output vertices; the final difference accepts at most 8,704 input and 8,192 output vertices and at most 1,024 components. Consequently one signal is capped at 526,848 Clipper input vertices, 577,536 output vertices, and 12,288 components; one trajectory at 50,577,408 inputs, 55,443,456 outputs, and 1,179,648 components; and the complete screen at 151,732,224 inputs, 166,330,368 outputs, and 3,538,944 components. These ceilings cannot fund continuation work or another trajectory.

Before every no-fit, erosion, union, or difference, check its actual input against the relevant row cap and reserve 32 MiB for all simultaneously live path, PolyTree, component, and scratch owners. Charge output vertices and components immediately after the operation before retaining any result. The complete pending ledger, identities, hashes, per-orientation aggregates, invalidation reasons, and serialization scratch have a separate 12 MiB retained reservation under the existing 64 MiB experiment-owned ceiling. Geometry work, continuation work, and retained memory use separate staged ledgers. The screen publishes successful rows only if all three trajectories reach their fixed parent limit without a cap or audit failure; otherwise it atomically publishes one failure-only record containing the reason and consumed work, with no partial predictive numerator or denominator.

Sample exact-valid sibling candidates before retention from the same parent, selected piece, and generator stream. A pair is eligible for preselection only when both siblings have the same ordered inactive stable IDs and the same sorted `lastTransition.ejected` IDs. This fixes inactive count, inactive area, inactive difficulty sequence, ejected material area, and ejected count—the existing comparator prefix through ejected count. Select exactly one pair from those eligibility fields and identity order before computing either topology signal. The measured pair contributes a predictive trial only when its signal values differ and the topology ordering opposes the existing remaining active-frontier/identity ordering; equality or agreement is recorded as no trial, and no later pair substitutes. Give each sibling exactly four subsequent displaced-first expansions under the existing deterministic generator and caps, then label its fixed-horizon endpoint by the existing count comparator. Before implementing the metric, pin three complete source trajectories by their existing population hashes. Within each trajectory, visit parent rows in ascending depth and augmented-identity order without filtering on any outcome.

Each parent contributes at most one descriptive trial. Sort its siblings by augmented identity and preselect the first pair satisfying only the inactive/ejected identity match above. Measure topology for those two siblings only, then orient that already-fixed pair from lower to higher signal without consulting either continuation outcome. Equal signals or topology/comparator agreement produce no trial and end that parent row. Both continuations must execute all four expansion slots and finish incomplete; early completion, frontier exhaustion, or failure is recorded and invalidates the trial without replacement. Every field of `GeneralPersistentVacancyWorkDiagnostics`, including geometry and audit counters, must match exactly between continuations. A mismatch invalidates the trial without replacement. This prevents both unequal realized work and the many dependent sibling pairs from one parent from becoming evidence. The ledger is worth advancing to a causal intervention only if every pinned trajectory independently supplies at least 30 valid contributing parents and exceeds 70% ranking agreement. That threshold is an engineering progression rule, not a confidence level or a claim that ancestry-correlated trials are statistically independent. Report per-trajectory numerator, denominator, every preselected pair identity, all no-trial and invalidation reasons, and exact work; do not pool them into a `p` value. No secondary signal, combined score, pair substitution, parent substitution, source trajectory, or signal direction may be selected after observing outcomes.

A passing predictive screen only justifies a later causal test. That test must generate and exact-validate the same candidate rows in control and treatment, spend equal continuation work, and differ only in whether the primary vacancy signal can alter candidate order. The control computes and records the signal but ignores it. Completion or a strict comparator improvement under that paired intervention, not correlation in the predictive screen, would support vacancy transport as an engine mechanism.

## Completed vacancy-topology probe

The read-only mode-`19` probe is pinned in `persistent-vacancy-topology-evidence.json`. One clean release executable from commit `a183228` ran `17,19,19,17`. After removing only the additive probe identity, all four population subtrees had the same SHA-256 `5d698cb...`; the two mode-`19` probe hashes were both `7e65bb5...`. Search work, terminal state, and the 10-inactive-piece / `45454905747` endpoint remained exactly mode `17`. All four recorded runs stayed within `6.59-6.69 s` and `48.15-48.89 MB` maximum RSS. The probe consumed 9,158 Clipper input vertices and 8,055 output vertices, far below its explicit cumulative caps. The evidence retains every raw-output, population, normalized-population, probe, timing-capture, executable, source-tree, request, fixture, and toolchain hash together with the complete common work and terminal records.

At zero clearance, the target-strip free space is one frontier-connected component in the repair root, round-zero endpoint, and round-one endpoint. Literal closed pockets therefore do not explain those three measured states; discarded siblings and transient states remain unmeasured. Clearance filtration changes the conclusion: at `5 mm`, disconnected free area is `13,491.473137`, `15,796.643943`, and `16,110.3708605 mm²` respectively; at `10 mm` it is `6,285.696487`, `4,222.0641585`, and `5,188.9226645 mm²`. The measured geometry contains substantial size-dependent bottlenecks even though its raw vacancy is connected.

The strict mode-`17` comparator improvement is only `0.0206025 mm²`, and its topology deltas are not directionally consistent. Relative to round zero, round one improves frontier-connected area at `2.5 mm` but worsens it at `1`, `5`, `10`, and `15 mm`; disconnected area moves in the opposite mixed pattern. These three endpoints are descriptive rather than causal and reject a post-hoc scalar such as maximizing disconnected area. They do not reject vacancy transport. The next admissible step remains the predeclared sibling-continuation ledger above: topology may influence search only after same-parent, equal-work continuations show that one fixed signal predicts the existing comparator outcome independently on every pinned trajectory.
