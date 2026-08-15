# Persistent vacancy population experiment

## Decision boundary

This is a diagnostic-only Mixed-61 experiment behind a new final benchmark argument, after the retained zero-valued retired-terminal argument. Mode `0` disables it. Modes `1` and `2` use the reviewed two-hardest scheduler with comparator and contact-signature retention respectively. Modes `3` and `4` use the stateless rotating scheduler defined below with those same two retention policies respectively. Mode `6` applies dual-objective retention to mode `3` without carryover, and mode `5` adds the bounded elite carryover to that mode-`6` policy. Nonzero modes must not change the protected constructor, relaxed search, coupled separator, public profiles, or returned result. A diagnostic candidate is reported separately until every promotion gate passes.

The frozen request SHA-256 is `dfd2ceecf02efe3475e3344dfefbfb2a2a5bd8a673008b449f5689507c933ba1`. The required retained boundary-projection placement fingerprint is `b9335a72cdcdd8df29be21450818f4ab1766ea1ea0b16765ad3998942a2ea6c5`, with reported depth `168.625 mm` and independently rebuilt source depth `168.361 mm`. The experiment searches a fixed `165.000 mm` strip. An identity mismatch skips the arm before any experimental work.

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

Each arm has its own nontransferable ceilings. The declared worst-case schedule is funded as follows:

- layers: `40`;
- parent slots: `40 * 8 = 320`;
- selected-piece slots: `320 * 2 = 640`;
- orientation streams and collision builds: `640 * 12 = 7,680`;
- canonical source-feature visits across both mirror traversals: `640 * 2 * 512 = 655,360`;
- pre-deduplication position-source attempts per orientation: `1 + 8 + 61 * 8 + 16 + 16 = 529`;
- total pre-deduplication position-source attempts: `7,680 * 529 = 4,062,720`;
- returned positions, hazard queries, and placement attempts: `7,680 * 32 = 245,760`;
- proxy-pressure visits: `245,760 * 61 = 14,991,360`;
- exact finalist rows and finalist collision builds: `640 * 8 = 5,120`;
- initializer collision builds: `61`;
- experimental collision builds: `61 + 7,680 + 5,120 = 12,861`;
- initializer exact pair rows: `61 * 60 / 2 = 1,830`;
- finalist pair visits: `5,120 * 60 = 307,200`;
- experimental pair visits: `1,830 + 307,200 = 309,030`;
- partial dual audits: `41`;
- complete dual publication audits: `64`;
- total dual audits: `41 + 64 = 105`;
- validator collision builds per dual audit: `2 * 61 = 122`;
- validator collision builds: `105 * 122 = 12,810`;
- validator pair visits per dual audit: `2 * 1,830 = 3,660`;
- validator pair visits: `105 * 3,660 = 384,300`;
- aggregate collision-build ceiling: `12,861 + 12,810 = 25,671`;
- aggregate pair-visit ceiling: `309,030 + 384,300 = 693,330`.

Reject an input piece whose canonical source representation exceeds 512 features. Check every rebuilt expanded collision immediately and reject the arm if it exceeds 512 vertices. The resulting transformed-collision vertex ceiling is `25,671 * 512 = 13,143,552`. AABB-disjoint pair visits still consume the logical pair-visit ceiling but skip Clipper. Before each pair operation, charge its actual two input vertex counts; the conservative aggregate Clipper input ceiling is `2 * 512 * 693,330 = 709,969,920` vertices. This is a monotonic work counter, not a simultaneous-memory allowance.

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
  0 10 1 0 0 0 0 MODE
```

Run four cold processes after one unmeasured release build in the order control, treatment, treatment, control, with `MODE=1,2,2,1`. The canonical identity is Apple M4 Max, `aarch64-apple-darwin`, eight requested and actual threads, rustc `1.95.0 (59807616e 2026-04-14)`, LLVM `22.1.2`, locked dependencies, and no `RUSTFLAGS`. Every output records the engine commit, dirty status, relevant-source-tree hash, executable hash, full rustc identity, machine, thread counts, request hash, frozen parent fingerprint, mode, seed domain, and exact command. The two outputs for each arm must match those identities.

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

The projected mode-`0` JSON must hash exactly to `f51f8d4e767c4828430af4f154616b9c73aa237f1cbfbf0cc3e04d6cadfe85d0` with SHA-256. The earlier `0f39c64...` projection included the then-current commit and was therefore commit-bound rather than a stable compatibility hash; deleting `engineCommit` is the only projection correction, and the old and new same-layout outputs both reproduce `f51f8d4...` under it.

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

## Portable pinned-parent fixture

The frozen `b9335a72...` parent is a fingerprint of the boundary-projection trajectory on the canonical Apple M4 Max platform. Arbitrary-angle trigonometry is not promised byte-identical across numeric platforms, so a different machine cannot reproduce that parent in-run: on `x86_64-unknown-linux-gnu` the same request converges to an independently valid `181.938 mm` endpoint whose projection fingerprints as `51b77c0f...`, and every nonzero mode therefore skipped before any experimental work. The benchmark now accepts one additional optional trailing argument naming a pinned-parent fixture. `tests/fixtures/mixed-61/persistent-vacancy-parent-b9335a72.json` carries the 61 frozen placements extracted verbatim from `docs/experiments/mode-zero-new.json` (`boundaryProjectionTreatment.finalPlacements`); placement coordinates are exact 0.001 mm grid values and rotations exact 1e-6-degree values, so the fingerprint is platform-independent. The loader verifies the fixture's pinned request hash against the current request; the engine then applies the unchanged compiled-in frozen fingerprint, parent-depth, and dual-validation checks to the loaded layout, and records `parentSource` plus the fixture path and SHA-256 in the output. Without the argument, behavior is unchanged and output is byte-identical under the declared timing/provenance projection. With the pinned parent, mode `3` on `x86_64-unknown-linux-gnu` reproduces the canonical M4 endpoint facts exactly: the same initializer (25 inactive), the same best-ever `c2329244...` state at 11 inactive pieces / `59571041296` grid-area units, the same final `d12e7ee3...` state at 12 / `60144097737`, and the same exact finalist-row count. Per-layer M4 trajectory artifacts are not retained in the checkpoint, so full-trajectory equality is not independently demonstrated; the endpoint claim is what the pinned M4 evidence supports, and it already shows the vacancy lifecycle's endpoints are platform-stable while the upstream continuous-separator trajectory is not.

## Out-of-beam topology archive screen (modes 7 and 8)

The elite screen closed fixed-slot reservation and left open "preserve promising topology without permanently taxing every generation's scarce beam slots". Modes `7` and `8` implement that open variable with a bounded archive held outside the beam. Both are exactly mode `3` (stateless rotating scheduler, area-first comparator retention, no carryover pool) plus:

- After each layer's retention, whenever the monotone best-ever area-first or count-first elite improves, the archive stores a full clone of that retained elite state (at most two entries, area and count). Archive clones are charged into the raw, live, and retained memory preflights through `totalRetainedPeakBytes`; `retainedPeakBytes` remains population-only.
- Deterministic stagnation detection: a revival may fire at layer `L` only when `L - lastImprovementLayer >= 3` and at least `3` layers passed since the previous expanded revival, with at most `13` expanded revivals per run (`1 + (40 - 1 - 3) / 3`). Candidates alternate area/count by expanded-revival parity; a candidate whose semantic identity is already in the entering population falls through to the other elite, and if both are present the attempt is recorded as skipped without consuming the alternation ordinal.
- Mode `7` expands the revived clone as one additional parent after the ordinary eight, with the identical per-parent schedule. The ordinary child-order hash keeps its cross-mode meaning: it is computed over the ordinary parents' children only, before revival children merge into the same sort/dedup/retention pipeline. The revival lane is funded by revised quota formulas (`640 + 26` selected-piece slots and every downstream ceiling raised by the same term); the two formula tests and this document change together with the constants.
- Mode `8` swaps the revived clone into the comparator-worst entering slot before the entering-population hash, only when the archived state is strictly better than that slot under its own comparator and the population holds at least two states. Width stays eight and no work is added.
- Modes `7` and `8` do not alter modes `0` through `6` semantics: all archive fields serialize as `Option` values that stay absent elsewhere, shared quota ceilings were only ever raised (never lowered), the shared expansion code refactor was verified signature-identical, and the mode-`0` output remains byte-identical under the timing/provenance projection. At-HEAD runs of every frozen mode reproducing their historical terminal semantics are pinned in the archive evidence.

Run one unmeasured release build, then cold processes `MODE=3,7,7,3` followed by `MODE=8,8`, every arm with the pinned-parent fixture argument, on one machine. Same-mode normalized replays must be byte-identical under the declared timing projection. Because the first mode-`7` revival cannot fire before layer `3` and retained revival children first appear later, every layer before the first retained revival child must show entering-population hashes, ordinary-child hashes, and per-layer work identical to the contemporaneous mode `3`; a divergence there invalidates the screen. Treatment beats control only by unique exact-valid completion, strictly lower independently validated complete depth, or — when neither completes — the established partial gate: a terminal comparator-best partial with strictly lower inactive grid area and no more inactive pieces than the control's terminal comparator-best, with best-ever objectives not regressing. Preservation of an already-known best-ever without a strictly better terminal population rejects the arm. A cap, audit disagreement, replay mismatch, wall time over `6.5 s`, or RSS over `64 MiB` also rejects. These gates are declared before the evidence runs; results are pinned in `persistent-vacancy-archive-evidence.json`.

## Descending-target contraction lane (modes 9 through 12)

Modes `1` through `8` are frozen diagnostic screens: their `165.000 mm` target and `b9335a72...` parent identity are part of the pinned experiment contract and never change. Modes `9` through `12` form a separate opt-in contraction lane that turns the same exact-valid partial-state machinery into a depth-descent driver:

- Every descent mode requires an explicitly pinned parent fixture and an explicit target depth (a new final benchmark argument after the fixture path). The frozen fingerprint and depth equality pins are skipped — the descent lane is designed to chain across parents — but the parent still passes full-sheet exact validation, its fingerprint and independently measured depth are recorded, and every published state passes the unchanged dual publication gates. A target override outside modes `9`-`12` fails closed, and modes `9`-`12` fail closed without a pinned fixture; modes `1`-`8` continue to run without one.
- Mode `9` is the population lane itself: mode-`3` retention and scheduling plus the mode-`8` archive/swap revival, at the requested target.
- Mode `10` adds an odd-layer blocker-relocation slot: slot one is replaced by a relocation of the active piece most often named in slot zero's ejection sets (fallback: the deepest active piece), implemented by expanding the piece as an insertion into a temporary state with itself removed, at identical slot funding.
- Mode `11` adds a translation-only exact settling prelude before target deactivation: three bottom-up sweeps over all 61 pieces, each attempt sliding its piece straight down with a `0.512` to `0.001 mm` step ladder under at most 64 exact probes, every probe gated by full-sheet containment plus zero exact pair intersection against every other piece. Settling is an endpoint-exact re-placement move, not a swept-motion contract: near-tangent neighbors can form forbidden bands thinner than one step, so a probe may land beyond a band no continuous slide could cross — exactly like every other placement operator in this experiment, all of which relocate pieces discontinuously. Validity rests entirely on the per-probe exact gates and the final dual publication audit, never on motion continuity. The settle phase charges one orientation stream per attempt, charges its live state pair against the retained-memory gate, and counts a settled complete candidate before its audit so a publication rejection is visible in diagnostics. When settling alone pulls every piece inside the target, the settled complete state passes the dual publication audit directly. A lateral (bottom-left) ladder variant was tried and rejected: left-compaction disturbed the vertical channels the boundary offenders need and lost the completions the vertical prelude achieves.
- Mode `12` combines the settle prelude with the relocation slot.
- The quota constants are derived formulas carrying all three lanes explicitly (ordinary `640` slots, archive `26`, settle `183`; finalist rows `666*8 + 183*64`); the aggregate formula test changes together with the constants and this document.

Mode `13` is a guided reconstruction lane: it accepts a deliberately invalid hint fixture (skipping parent validation only), inserts pieces in ascending hint-depth order through displacement probes, the ordinary generators, and an upward shelf fallback with a deferred second pass, and publishes only through the unchanged dual gates. Its first application — re-seeding the engine from the committed Sparrow calibration layout — is a characterized negative: the Sparrow layout is packed at `5.0 mm` separation across the full `2000 mm` width, and the engine's contract requires `totalPaddingMm + 2 * flatteningSagToleranceMm = 5.5 mm`, so width-saturated piece runs cannot absorb the extra `0.5 mm` per gap; the completed deferred pass places 39 of 61 pieces from the unstretched hint field and 42 of 61 from a 12% depth-stretched field, and every deferred piece fails its retry. This exposes a program-level calibration correction: the `154-155 mm` band was measured under Sparrow's `5.0 mm` contract and is not directly comparable to engine results, whose validators enforce `5.5 mm` pair and `5.25 mm` boundary clearance; the contract-native reachable band is necessarily deeper and remains unmeasured until an upstream run with `5.5 mm` separation is calibrated.

The committed driver `docs/experiments/persistent-vacancy-descent/run-descent.py` chains mode-`11` runs: each completed hop's layout becomes the pinned parent of the next, deeper target, with an adaptive delta schedule (or an exact recorded target sequence for replay). On `x86_64-unknown-linux-gnu` the chain from the frozen `168.361 mm` parent completed at targets `168.550`, `168.544`, `168.538`, and `168.532`, reaching an independently validated complete depth of `168.277 mm`, and stalled at target `168.526-168.527`: settling slack is exhausted (zero accepted settle moves) and the population plateaus at one to two inactive pieces. Modes `9`, `10`, and `12` never completed any probed target without the settle prelude's zero-offender path; the operator family's limit on this parent is approximately `0.084 mm` of recovered depth. Evidence, exact replay target sequences, raw hop outputs, and chained parent fixtures are pinned in `persistent-vacancy-descent-evidence.json` and `docs/experiments/persistent-vacancy-descent/`.
