# Persistent vacancy articulation probe

## Question

The clearance-topology probe found no literal zero-clearance pocket in the Mixed-61 repair root or its two retained restart endpoints, but it found substantial disconnected free area after positive-clearance filtration. The next read-only screen asks a narrower mechanistic question before any new move operator is built: does removing one currently active piece connect a size-relevant amount of pre-existing internal vacancy to the `165 mm` frontier?

This is a counterfactual articulation test, not a ranking objective and not a nesting-quality experiment. A positive result only identifies concrete bridge pieces for a later sliding-vacancy transition. A failed mechanism gate means that fewer than two frozen states expose enough pre-existing vacancy at the actionable `2.752 mm`-or-greater scales; it can coexist with articulation at `1` or `2.5 mm`, or with actionable articulation in one state. It does not close vacancy transport generally or coordinated multi-piece transitions.

## Frozen trajectory and state identities

Experimental mode `25` starts from the committed Mixed-61 parent fixture and executes the exact mode-`17` repair/restart trajectory. The three measured states are the repair root, round-zero comparator-best partial, and round-one comparator-best partial. The probe runs only after both repair rounds and all ordinary mode-`17` diagnostics have completed. It may not alter a state, queue, seed, selected piece, candidate, work counter, audit, comparator, or publication decision.

The following identities are frozen. State fingerprints use the existing `state_fingerprint` contract. Active IDs are in input-piece order; inactive IDs are in the retained queue order. Each ID-list hash is SHA-256 of the UTF-8 bytes of its compact `JSON.stringify` array.

| State | State fingerprint | Active count | Active-ID hash | Inactive count | Inactive-ID hash |
| --- | --- | ---: | --- | ---: | --- |
| repair root | `1b2fd098813d00f01067e2ea95ad494c41b5126cc48da0df26c147bf6601c0df` | 51 | `73f2ad8c818bef0413bd04ede31a5634d6c0cdaea593f6a4ac2808eea66cc70a` | 10 | `c49ce0549e87d8b2734e6a37fe4228ccb03d108b70105c1e0859e72a6269ddd8` |
| round-zero best partial | `bed29b45996a6bcccc5dad8f498f7522711976bd6d26e26ae78da18d99935da1` | 51 | `e4a5cbd61cbf2b7e136018898bc104139f43c58796aa17145d1d6799cc714ead` | 10 | `fe3f1dace6e47b36dfb9515901c9909dc63bee2e886019a7349e4806a711196a` |
| round-one best partial | `6b42ff5e85749e4e3ce00bc36e6462f29a61290c74cd0bdfc5ae328ab5e16d28` | 51 | `baad00c34b65c4fca296fa15d3488d5d03799cc3830e12acae524f2c880900f0` | 10 | `9485763978e17bbb3992b6a63a210aec9fdb5f2c5d565109c9f50159bdbf9873` |

The round-zero inactive hash is the retained best partial's queue before rebuilding; it deliberately differs from `roundOneRoot.queuePieceIds`. Any mismatch rejects the probe before its first geometry allocation. Deleting only the additive `vacancyArticulationProbe`, mapping mode `25` to `17`, and mapping its arm label to `continuedStateRebuiltQueue` must reproduce the lossless current-source mode-`17` population SHA-256 `8f91c7fe755e1fac1dc237dda09f53a58fd538ff5decbc7df7a693f09cab135a`; the historical compact-JSON pin remains recorded in `persistent-vacancy-topology-evidence.json`. Run one freshly built release executable in order `17,25,25,17`; both contemporaneous mode-`17` controls must reproduce that hash. The two complete mode-`25` population subtrees, including `vacancyArticulationProbe`, must also be losslessly identical; the probe sidecar is not removed for this comparison. A separate whole-output comparison may remove only these explicitly listed timing/OS measurement field names: `elapsedMs`, `engineElapsedMs`, `firstQuartileElapsedMs`, `interquartileRangeElapsedMs`, `maxElapsedMs`, `medianElapsedMs`, `minElapsedMs`, `thirdQuartileElapsedMs`, `wallSeconds`, `maximumResidentSetBytes`, and `phaseElapsedMs`. Any additional timing or OS field requires an explicit design and oracle update before it can be ignored. No field inside the complete population subtree is discarded by the replay oracle.

## Geometry and direct component correspondence

The geometry domain is the existing exact contractual-grid `expandedCollisionExactGridClearanceFiltrationWithinTargetStripV1`. The fixed clearance schedule is `1`, `2.5`, `2.752`, `10`, and `15 mm`. The `2.752 mm` value is not fitted: the canonical command overrides total pair clearance to `5 mm`, and the frozen collision expansion is `pairClearance / 2 + clearanceSafetyMargin + conservativeOffsetAllowance = 2.5 + 0.25 + 0.002 mm`. Rows at or above that clearance are marked `actionableScale`; smaller clearances remain evidence and are not silently omitted.

For each measured state and clearance:

1. offset every active collision polygon once by the clearance;
2. compute the baseline free-space topology inside the inset target strip and retain each disconnected component as an exact polygon region;
3. for each active piece in stable input-piece-ID order, recompute the topology with only that expanded obstacle omitted;
4. build an exact material-node graph over the baseline and independent counterfactual regions. Baseline polygons remain verbatim graph nodes; no baseline path is re-clipped or re-rounded. A baseline/counter edge is added only for positive-area material overlap, a proper boundary crossing, or a shared positive-length segment. Pure point contact is never a vacancy connection. Union-find over these nodes derives whether each baseline component is connected to a frontier-connected counterfactual node, so sub-grid slivers cannot erase the baseline component. The exact material-subset predicate remains unchanged and independently contract-tested for material inclusion; it is not used as the semantic connectivity relation of this graph. Overflow, ambiguous geometry, or a missing graph node fails closed;
5. sum the already-exact baseline component areas mapped to frontier-connected counterfactual regions as `U`, and sum the remaining baseline component areas as `R`; and
6. drop the counterfactual topology before processing the next omission. No placement is moved and no result is admitted into search.

`U` is exact component correspondence, not an intersection-area estimate. The graph preserves the immutable exact area of each baseline component and changes only its connectivity classification. The exact partition `U + R = D` proves that the reported unlocked area was already free before the omission and became frontier-connected afterward. Counterfactual total/frontier/disconnected scalar areas remain diagnostic measurements; they are not used to upper-bound `U` or `R`. The unchanged `exact_material_subset_of` contract still rejects holes and disconnected material fragments in its own direct tests; graph connectivity does not weaken that predicate.

Let `T`, `F`, and `D` be baseline total, frontier-connected, and disconnected doubled-grid areas; let primed values be the counterfactual quantities; and let `A` be the exact doubled-grid area of the omitted expanded obstacle before clipping to the target rectangle or unioning with other obstacles. Checked integer arithmetic must prove every row satisfies:

- `T = F + D` and `T' = F' + D'`;
- `0 <= T' - T <= A`;
- `F' - F = (T' - T) + (D - D')`;
- `0 <= U <= D`, `0 <= R <= D`, and `U + R = D`;
- region areas separately sum to `F`, `D`, `F'`, and `D'`; and
- frontier-connected plus disconnected region counts equal the corresponding total region count.

The materiality reference is also frozen. For each inactive piece in each state, construct its unrotated, unmirrored, origin-translated polygon and apply exactly the effective collision expansion (`2.752 mm`); reject geometry above the ordinary `512`-vertex collision cap; and compute its exact doubled-grid area. The state threshold is the minimum of those ten canonical areas. This orientation is only a deterministic capacity reference and does not claim shape fit. A row is a material articulation row when `U` is at least that state threshold. It is an actionable material articulation row only when its clearance is at least `2.752 mm`.

## Ordering and frozen work ceilings

Rows are emitted in `(state ordinal, clearance ordinal, stable active piece ID)` order. Diagnostic hashes never select or order work. Within a state, pieces rank for reporting only by maximum actionable clearance with a material articulation row descending, unlocked baseline-disconnected area at that clearance descending, total unlocked area across clearances descending, then stable piece ID. No fitted coefficient enters the probe.

Let `S = 3` states, `C = 5` clearances, `P = 51` active pieces, `V = 512` vertices per collision polygon, and `Q = 4096` output vertices per topology. The probe has `R = S*C*P = 765` counterfactual rows, `B = S*C = 15` baselines, and `N = R+B = 780` topology calls. A different active or inactive count, a topology above `Q`, or a collision above `V` rejects the probe rather than changing a quota.

The following checked cumulative ceilings are frozen before implementation:

- active-clearance offset builds: `R = 765`; input vertices `R*V = 391,680`; output vertices `R*V = 391,680`;
- canonical inactive collision builds: `S*10 = 30`; offset input vertices `30*V = 15,360`; output vertices `30*V = 15,360`;
- free-space topology calls: `N = 780`; input vertices `B*(4+P*V) + R*(4+(P-1)*V) = 19,978,800`; output vertices `N*Q = 3,194,880`;
- exact material-graph node pairs: at most `R*(Q/3)^2 = 1,425,367,125` baseline/counter-region pairs. Exact-grid bounding boxes are cached once per region and disjoint pairs are rejected before any edge scan; the rejection count has the same cap;
- exact material-graph edge checks: at most `R*Q^2 = 12,834,570,240` integer-grid edge pairs; point-only contacts are rejected without creating a graph edge; and
- exact-area vertex visits across cached collisions and topology outputs: at most `9,868,800`.

Every operation preflights its full input-vertex charge before allocation and charges its actual output before publication. The implementation must expose per-category consumed counts and caps. No partially computed row is published after a cap failure.

## Simultaneous memory and failure transaction

Only one `(state, clearance)` batch is live. Expanded polygons are cached immutably for its 51 active pieces, not for all 765 rows. The following intentionally conservative ownership reservation is frozen:

- `3,343,992 bytes` for the active expanded-polygon cache (`Vec` owner plus `51` entries, each with a `64 KiB` polygon-heap reservation), with each polygon's measured heap checked against its per-polygon reservation;
- `8 MiB` for the baseline topology, its component polygons, and `BD`/`BF` unions;
- `8 MiB` for the current independently reconstructed counterfactual topology;
- `256 KiB` for graph union-find state and the bounded baseline-node frontier map;
- `16 MiB` for the single live Clipper/PolyTree operation and its paths; and
- `4 MiB` for the final JSON serialization buffer. Mode `25` writes its typed
  output schema directly into this fixed-capacity buffer; it does not construct
  an intermediate `serde_json::Value` tree. On overflow, only the additive
  articulation sidecar is replaced in the typed diagnostics and the same
  bounded writer is retried.

These transient owners total `41,354,872 bytes` from the named reservations above. The scalar row vector, rankings, strings, counters, and complete failed-row prefix have a separate `1 MiB` retained reservation. Their measured capacities are checked against that reservation before attachment. If that retained reservation is exceeded, the diagnostic vectors are cleared only after recording the pre-clear retained peak, so the failure sidecar cannot under-report the attempted allocation. Adding both reservations to the pinned protected population peak `6,353,345 bytes` yields `48,756,793 bytes`, below the existing `64 MiB` total-retained ceiling. A focused formula test must enumerate every simultaneously live vector and backing allocation; it must fail if a later type or owner is added without updating the formula. Cold-process wall time must remain at most `7.5 s` and process RSS below `64 MiB`.

Mode `25` is transactional. It first constructs and retains the complete mode-`17` result and the three verified state snapshots. The probe then computes a local sidecar. On probe allocation, arithmetic, geometry, cap, or invariant failure, it attaches a bounded failed sidecar containing the completed-row prefix, consumed counters, peaks, and reason, and still returns the untouched successful base result. The benchmark serializes and prints that result, flushes stdout, and only then returns a nonzero error when the requested probe sidecar reports failure. Environmental stdout or serializer I/O failure is outside the experiment because no program can guarantee an observable payload after its output channel fails.

## Decision and validation rules

The mechanism gate passes only if at least two of the three measured states contain an actionable material articulation row. The same active piece need not win in both states. Passing this gate justifies a separately predeclared sliding-vacancy operator that relocates a measured bridge piece while preserving exact feasibility; it does not promote a search policy. If the gate fails, single-piece omission did not expose enough pre-existing vacancy at the request's collision-expansion scale in at least two frozen states. Pair removals, wider beams, more repair rounds, and a treatment search are not added post hoc.

Validation requires the exact mode-`17` projection, both control hashes, the six pinned state/ID hashes, identical same-mode replays, all 780 topology calls, bounded graph node/edge counts, all three state summaries, complete cap and memory ledgers, every exact identity above, the protected portability harness, focused geometry tests for deterministic witnesses, the 941-grid² sliver graph repair, point-contact rejection, a true bridge, and a frontier-adjacent non-bridge, the complete core test suite with `jagua-experimental`, and adversarial review before evidence is interpreted.

The derived bidirectional boundary-edge ceiling exceeds 32-bit `usize`. Its cap
and consumed-work diagnostic are therefore `u64` even when the host pointer
width is 32 bits; each per-call `usize` count is checked before conversion.
