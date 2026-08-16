# Next-generation polygon nesting engine

## Objective

Build an anytime, short-side-first nesting engine that preserves real polygon topology, produces a useful feasible layout quickly, and improves it for as long as the caller's time budget permits. Concave parts, continuous rotation, deterministic replay, and independently validated output are core requirements.

The existing convex engine remains available as a compatibility baseline. Its current protocol, output, and 18-row golden matrix must stay byte-stable unless a separately reviewed migration deliberately changes them.

## Why a new engine path is required

The current collision builder replaces each flattened source contour with its convex hull before offsetting it. The strict decoder then enumerates only immediately legal placements from a finite transform and contact catalogue. This makes the implementation deterministic and robust for convex geometry, but permanently removes concave pockets and prevents the search from crossing temporarily infeasible states to escape a poor early structure.

Improving candidate ranking inside that architecture cannot recover geometry that was discarded before search.

## Architecture

### 1. General polygon topology

Introduce an internal `PolygonSet` representation containing one or more regions, where each region has one outer ring and zero or more hole rings. Every ring is implicitly closed, normalized deterministically, and retains both its original and canonical-grid vertices. Multiple regions are required because inward erosion can split connected material and outward expansion can merge previously separate regions. Simple concave outer rings are supported from the first milestone.

The current public source-geometry protocol has no contour grouping, so it cannot represent holes. The first milestone preserves the multi-region/hole-capable internal representation but accepts only one source outer ring. A later additive protocol version introduces explicit contours and winding-independent hole classification before source holes become a public capability.

For protocol-v1 ingress, segments must form one ordered closed cycle. Consecutive segments may be reversed individually to connect to the preceding endpoint, but the importer may not globally reorder ambiguous edges. Every consecutive and closing endpoint must be identical after contractual-grid snapping; the importer never inserts an artificial closing edge or merges a one-grid-unit gap. Branches, repeated non-adjacent vertices, disconnected loops, and multiple cycles are rejected. Curve samples replace their source segment in that same cycle; global point deduplication is forbidden on the general path.

Convex hulls and axis-aligned bounds remain derived broad-phase data only. They must never replace authoritative source or clearance geometry on the general path.

### 2. Feasibility kernel

Use a layered collision query:

1. transformed bounds and a spatial index reject obvious non-collisions;
2. integer-grid polygon-set intersection and containment establish contractual-grid legality;
3. offset polygon sets encode requested clearance;
4. an independent final validator rebuilds transformed source rings and checks sheet, overlap, and clearance without using the search kernel's offset/intersection implementation.

The search kernel may use approximations for ranking, but publication is an AND gate: the transformed source is rebuilt, snapped, and offset on the contractual integer grid, then the result must also pass the independent source-ring validator. Neither validator may mark a state publishable alone.

#### Numerical and clearance contract

- Source line and curve parameters remain finite `f64` values. Curves are flattened with the request's sag tolerance before any grid conversion.
- The authoritative collision grid is 0.001 mm. Coordinates are rounded to the nearest grid unit and must stay inside the existing safe-coordinate guard. A ring that collapses, self-intersects, or changes topology after snapping is rejected rather than repaired silently.
- `totalPaddingMm` means requested pairwise clearance. Each part is expanded by `totalPaddingMm / 2 + clearanceSafetyMarginMm`, followed by the existing 0.002 mm conservative Clipper allowance. Two expanded boundaries may touch, but their interiors may not overlap.
- By default the same expanded part must remain inside the sheet, preserving the legacy sheet-edge clearance of `totalPaddingMm / 2`. An explicit `sheetEdgeClearanceMm` may instead set the source-to-sheet clearance independently; the search rectangle is inset relative to the pairwise collision expansion so the two requirements are not conflated. Hole-edge clearance uses the same rule when source holes are introduced.
- The safety-margin invariant remains `clearanceSafetyMarginMm >= flatteningSagToleranceMm`; grid and curve error budgets are never replaced with an arbitrary relative epsilon.
- Clipper output is the deterministic search authority, not a claim of exact real-number geometry. The publication validator uses robust segment intersection, winding-based containment, and explicit segment-to-segment distance over independently transformed flattened source rings; it does not consume Clipper offsets or intersection output. For pairwise clearance it requires at least `totalPaddingMm + 2 * flatteningSagToleranceMm`; against the sheet it requires at least the effective sheet-edge clearance plus `flatteningSagToleranceMm`. Ambiguous non-finite or degenerate cases fail closed.
- The initial general path accepts at most 2,048 snapped vertices per ring, 8,192 vertices per polygon set, and 131,072 vertices per job. One exact pair query may contain at most 4,096 combined vertices. The 250 ms stress target is measured evidence, not a proof for unseen input. Untrusted general-profile geometry therefore remains in a killable worker process until every synchronous geometry operation is interruptible or a stronger bound exists.

#### Topology-preserving Clipper contract

- Outer rings are canonicalized counter-clockwise and holes clockwise. Clipper operations use `FillRule::NonZero` with every region submitted as one outer path followed by its hole paths.
- An inward offset may split one outer into multiple outer regions; an outward offset may merge regions. Every resulting outer becomes its own deterministically ordered `PolygonRegion`.
- Holes that collapse during an outward offset are removed. Surviving holes are attached by the original offset `PolyTree` ancestry and sorted by their canonical ring key.
- An empty result is valid only for an inward offset explicitly allowed to consume all material. An outward offset of non-empty material that returns empty fails closed.
- Multi-level islands become independent regions. A ring that cannot be assigned unambiguously from PolyTree ancestry fails closed; flattened `Paths64` are never used to infer ownership after the fact.

### 3. Fast feasible seed

The general engine is short-side-first by construction, not a compact engine with a directional observer added afterward. Resolve the physical short and long sheet axes once, normalize `x` to the short axis and `y` to the long axis, fill the available short-axis width, and minimize long-axis depth. Candidate score is lexicographic: placed count descending, long-axis depth ascending, unused short-axis projection gaps ascending, occupied envelope area ascending, then canonical placement key. A generic compactness score is never allowed to defeat a better long-axis depth.

Run a small deterministic constructor within an explicit work quota. Its role is to provide a valid incumbent and fallback quickly; its layout is not the quality ceiling:

- measured default piece order by longest source span for mostly convex jobs; when at least one third of pieces contain reflex vertices, prefer actual material area because bounding spans exaggerate concave occupancy. Hull-area, concavity-, and elongation-led orders remain bounded portfolio alternatives;
- first-class arbitrary rotation in the fast constructor: reserve `0/90`, then prioritize longest moving-edge alignment with sheet axes and moving/placed edge-direction differences before filling unused quota with uniform seeds; mirrored variants derive their own angles from reflected edge directions;
- translations from short-axis sheet supports, moving-vertex-to-placed-vertex contacts, moving-vertex projections onto placed edges, and placed-vertex projections onto moving edges;
- proposals carry a canonical key ordered by angle, mirror, feature kind, fixed piece ID rank, ring/edge ordinals, and grid translation;
- score candidates by long-axis bottom-left gravity, then short-axis gravity, strip depth, unused short-axis projection, envelope area, and canonical key;
- rebuild every competitive non-orthogonal candidate from transformed source geometry before accepting it, because offset-then-transform and transform-then-offset are not interchangeable;
- generate at most eight cheap bounds-scored proposals per exploratory exact-evaluation slot, reserve three quarters of that arm's exact quota for deterministic key-order coverage, and use the remainder for globally scored non-overlap and concavity candidates across all sampled angles;
- at most 64 angle candidates and 4,096 exact placement evaluations per piece per portfolio arm; the exploratory arm is separately budgeted and may be disabled without changing the primary result;
- the first M1 constructor uses one stable measured default order, no randomness, no exhaustive coordinate scan, and no fixture-specific coordinates;
- no repair in the first M1 constructor; remove-and-reinsert begins in M2 after the base constructor is independently characterized.

Arbitrary rotation is not an optional polishing phase: removing it reproduces a known weaker design. The constructor does not run separate orthogonal and arbitrary-angle beams by default; the full orientation set shares one explicit exact-evaluation quota, and changes to its priority order must pass the protected benchmark corpus. Every completed optional phase contributes its best valid incumbent and must never discard a better valid layout. Deadlines are cooperative between bounded geometry operations. The general profile will impose explicit per-ring, per-piece, and per-job complexity limits before claiming a bounded interruption latency; a synchronous Clipper call is not itself interruptible.

On the current Mixed-61 research request with 5 mm pair and edge clearance, the measured pivot from area-led orthogonal construction to longest-span, edge-aligned arbitrary-angle LBF reduced independently reconstructed depth from approximately 217.970 mm to 184.476 mm and reduced a three-run median from approximately 3.261 s to 1.162 s. The 132.462 mm expanded-area lower bound shows that this is a meaningful constructor improvement, not the quality ceiling. The reflex-density order selector simultaneously produces 15.809 mm on the protected eight-piece concave fixture, improving its 16.008 mm golden and retaining a material advantage over the 20.130 mm convex-hull ablation. Larger angle catalogues, hull-first ordering, fixed narrow bands, shape-family pairing, single-piece repair, angular repair, and the first relaxed-search probes did not clear their promotion gates; they remain disabled rather than adding seconds for marginal or zero improvement.

### 4. Relaxed exploration and compression

After the fast constructor has produced any feasible incumbent, run an independently designed relaxed search as the main quality engine. Literature and the controlled Mixed-61 comparison show that an irrevocable constructor cannot be expected to discover the best global structure. The relaxed phase is therefore explicitly responsible for rescuing weak seeds while the immutable feasible incumbent guarantees safe anytime output:

- temporarily permit overlaps;
- use a cheap broad-phase collision engine and a smooth overlap proxy for millions of sample evaluations; reserve canonical Clipper reconstruction for candidate feasibility and publication gates;
- decompose outer material into deterministic convex cells or conservative poles and score colliding cell pairs by a decaying penetration proxy plus a shape difficulty term;
- for intersecting convex cells, derive the minimum translation vector from SAT axes and order ties by penetration depth, axis angle, cell ordinals, then signed direction; sum pair pressure only for ranking and revalidate every move on the full polygon set;
- use closest boundary features for separated pairs and containment cases with no boundary crossing, ordered by squared distance, fixed/moving ring and edge ordinals, then endpoint parameter;
- assign dynamic penalties to colliding pairs;
- sample globally inside the current strip and locally around each colliding piece, retain several promising diverse samples, and refine translation, diagonal, and angle coordinates adaptively;
- periodically disrupt or reinsert large/high-pressure pieces;
- move only colliding pieces in multiple independently seeded orders and select the best epoch result;
- share only monotonically improving feasible incumbents;
- alternate aggressive exploration shrinks with progressively smaller strip-compression attempts;
- give independent restart lanes deterministic large-piece swaps and contained-neighbor relocation. A retained infeasible population is not part of the current engine; any future population arm must add distinct successor operators or additional bounded work and beat the single-parent control.

The protected incumbent is the best independently valid constructor result, complete or partial. Relaxed search always initializes a separate state containing every requested piece: it retains every constructor placement and deterministically shelves missing pieces in an oversized strip in canonical piece order. This initializer may overlap and may exceed the incumbent strip; it is never publishable until all pieces fit and both exact gates pass. Thus relaxation can rescue an incomplete constructor without weakening or discarding the protected incumbent.

The surrogate is neither a legality oracle nor a publication filter. A surrogate false positive can suppress a legal candidate and reduce search quality but cannot invalidate the incumbent; a surrogate false negative can trigger an exact validation attempt, which may be rejected. Surrogate-zero states are rebuilt from transformed source geometry and must pass both publication gates. The implementation tracks surrogate-zero/exact-rejected counts so approximation drift is visible. If deterministic triangulation or surrogate complexity limits are exceeded, the relaxed phase is skipped and the protected incumbent is returned.

The rollback triangle surrogate obeys the same transform order as publication. Convex cells and their cell index are cached by stable `(piece ID, canonical angle key, mirror)` only after transforming source geometry, snapping it on the canonical grid, and applying the contractual offset. Translations are canonical-grid coordinates. A cache miss may cost geometry preprocessing but cannot change the work accounting or result order. This angle-cached contract applies only to that rollback backend; the dynamic backend below retains continuous `f64` angles and has no per-angle geometry cache. The first public source protocol has no holes; any internal polygon set containing offset holes is not flattened or filled. It uses a hole-aware exact/NFP fallback or disables relaxation for that request until a hole-preserving cell representation is available.

Replay mode assigns every worker a seed derived from `(request seed, epoch, worker ordinal)`. Each worker starts from the same epoch state, consumes fixed sample and refinement quotas, and returns one state. Results are replayed in worker-ordinal order; infeasible outcomes are compared by physical surrogate loss followed by a canonical transform key, while every surrogate-feasible outcome is serially rebuilt, checked by both publication gates, and compared by exact short-side-first metrics followed by the canonical state key. Completion order is never observable. Worker count, target triple, toolchain, and numeric platform remain part of the replay identity.

Per-move cost is bounded explicitly. In the rollback backend, a spatial grid first selects overlapping transformed bounds and triangle SAT runs only for those candidate pairs. The relaxed profile caps convex cells per piece, total cells per job, global and focused samples per move, coordinate-descent refinements, workers, epochs, and restart disruptions. The dynamic backend replaces cell/SAT quotas with complete-query, fail-fast-query, hazard-update, ambiguity-confirmation, and retained-candidate `f64` confirmation quotas. A cap violation disables relaxation for that request rather than weakening exact validation or failing a feasible constructor result.

#### Collision-kernel decision checkpoint

The first M2 prototype proved that the search topology is viable but that its geometry kernel is not the intended production architecture. On Mixed-61 it evaluates roughly ten million candidate poses through almost sixty million triangle SAT tests, while every geometry-class orientation is materialized on a 2.5 degree grid. This both limits rotation freedom and spends most of the work rediscovering collisions that a dense, mostly static spatial index should answer directly.

The replacement prototype must decouple exploration geometry from publication geometry:

- keep the current triangle-cell backend as an ablation and rollback until the replacement passes its gates;
- materialize separate original and mirrored base rings in `f64`, recenter each around a deterministic local anchor, cast the centered coordinates to the exploration backend, and derive the adjusted translation from that anchor. Conversion collapse, self-intersection, multiple offset regions, or holes disable this relaxation backend for the request;
- keep candidate angles normalized in `f64` and keyed at the existing 10^-6-degree canonical precision only for deterministic comparison and replay. Cast to radians and the backend numeric type only at the adapter boundary; do not round candidates to a catalogue step or cache per-angle geometry;
- maintain a lane-local dynamic hazard index containing every accepted placement. An adapter-owned lifecycle map binds each stable input-piece ID to its current backend item and hazard handles; backend handles are replaced atomically after an accepted move, rebuilt after restore or strip reconstruction, and never participate in scoring, ordering, replay, or diagnostics;
- make query completeness explicit: a query returns either `Pruned { lower_bound }` or `Complete { boundary, sorted_pairs }`. Only a complete result may produce a tracker delta. The initial dependency experiment may prune after the backend's public fail-fast pass but must use its complete native edge and containment query otherwise;
- use fail-fast interior surrogates before complete queries, but treat backend contacts as an ambiguous candidate set rather than exact clearance truth. Before committing a retained move, confirm its AABB-selected neighbors with the existing `f64` surrogate so false negatives are added, contact false positives are removed, and the stable collision set is complete;
- keep guided weights in a dense stable `PairState { colliding, guidedWeight }` matrix and install each complete, confirmed result as one sparse sorted `MovedRowDelta { boundary, collidingPieceIds }`. The first kernel experiment gives every confirmed collision unit raw value; it does not invent a magnitude metric or score unreported pairs;
- rebuild the dynamic hazard index only at epoch initialization, strip compression, rollback, or disruption boundaries; an accepted single-piece move updates one hazard;
- preserve the existing `f64` contractual-grid and independent source-ring validators as the only publication authorities. A faster exploration backend may rank candidates, but it cannot publish a placement or weaken clearance;
- treat the exploration backend's numeric representation as part of its approximation contract. Any `f32` implementation is explicitly non-authoritative, and every surrogate-zero state still passes both `f64` publication gates;
- preserve the current one-outer-ring limitation of relaxed search. Multiple regions and holes require grouped hazard semantics and must not be flattened into filled simple polygons.

The preferred first experiment is optional, non-default, and pins the unmodified MPL-2.0 `jagua-rs` 0.7.2 collision engine behind an internal exploration trait. Its quadtree, fail-fast poles, dynamic hazards, and continuous rigid transforms directly match the workload, but its answers are not assumed conservative in either direction: nonparallel endpoint contact, collinear contact, approximate containment, and `f32` conversion do not share the publication convention. This is an evidence-driven dependency experiment, not adoption of Sparrow's optimizer: ordering, sampling, guided weights, compression, incumbent protection, and deterministic merge policy remain ours. If numeric ambiguity, update cost, topology limits, toolchain support, or dependency impact fail the gates below, retain the trait boundary and replace the implementation rather than contaminating exact validation with tolerances.

The kernel experiment first replays two fixed candidate streams. The throughput stream contains the current 10,045,836 Mixed-61 candidate queries and accepted-update sequence. The agreement stream contains 250,000 deterministic concave, mirrored, containment, boundary, contact, and near-contact poses over 257 angles that are not multiples of 2.5 degrees; its on-demand `f64` oracle runs outside timed measurements and pruning is disabled so every complete pair set is observable. Preprocessing, fail-fast queries, complete collections, hazard updates, retained-candidate confirmation, conversion failures, pair-set disagreements, structured exact rejections, and peak RSS are reported separately.

The dependency/kernel stage is accepted only if all of the following hold on the same machine and deterministic quota:

- three canonical-platform stream replays produce the same conversion, query, pair-set, and update fingerprints;
- conversion has no silent repair: every collapsed, self-intersecting, multi-region, or holed exploration polygon is rejected and counted;
- the agreement stream has no false negative outside an explicitly derived `f32`/contact ambiguity band, every ambiguous retained candidate is confirmed before tracker commitment, and complete stable pair sets otherwise match the `f64` oracle;
- complete-query throughput is at least three times the rollback kernel, with preprocessing and update costs reported separately;
- every confirmed update matches the sorted stable moved-row contract and every ambiguity is resolved before commitment;
- the protected concave fixture remains complete and demonstrates a legal pocket placement;
- peak resident memory remains at or below the approximately 143 MiB pre-sharing relaxed baseline;
- the unit-loss Mixed-61 run is reported only as a diagnostic. It cannot accept or reject the kernel for packing-quality reasons and is not evidence of progress toward 154 mm.

Only after the kernel clears these gates may a separate scoring/search stage begin. The earlier prescription to start that stage with a fixed whole-polygon coverage-pole magnitude is superseded by the controlled rejection recorded below. Any future magnitude experiment must identify a different causal variable and still specify its normalization, loss equation, positive fallback, and hard operation counters. It must preserve both 172.635 mm reported and 172.348 mm independently reconstructed depth with median engine time at most 2.6 seconds, or reach at most 165 mm within six seconds, before the search spends the resulting throughput on additional global/focused samples, angle coordinate descent, or parallel successor exploration. No result is retained for a marginal depth improvement bought with multi-second extra work. Faster geometry is infrastructure; it is not itself evidence of a better nesting algorithm.

The surrogate has separate feasibility and ranking contracts. Boolean feasibility means no boundary violation and no material-cell intersection for any broad-phase-selected pair. Ranking first aggregates triangle contacts into one normalized penetration value per polygon pair, then applies lane-local guided weights; tessellation density may not multiply a pair's weight. Deterministic work quotas count piece broad-phase probes, cell-index probes, and SAT tests, not just accepted samples. Every sample is an incremental delta involving the moved piece rather than a full-layout rescore.

#### Controlled search-topology evidence

Source and paper review must stay connected to measured ablations rather than being copied wholesale. Sparrow's retained infeasible population, Jagua's separated collision engine, Umetani's directional penetration moves, Egeblad's overlap-area minimization, Imamichi's iterated swap search, and Milenkovic's coupled translation/rotation compaction are architectural references, not interchangeable components.

A pinned same-machine calibration now gives the redesign an independently checked quality target. Upstream Sparrow revision `14f4868fcd7e97036700dbebaf193fb159180aa9`, built with Rust 1.95.0 and `-C target-cpu=native`, placed all 61 Mixed-61 pieces with continuous rotation, 5 mm item separation, 5 mm sheet inset, seed 0, and eight workers at `154.44858 mm` under a three-second CLI budget. A separate `f64` source-ring validator reconstructed every exported transform and confirmed all 61 pieces, minimum pair distance `5.000259999999969 mm`, and minimum boundary distance `5.0003288600859594 mm`. The input, raw solution, converter, validator, exact commands, hashes, and validation output are retained in `docs/experiments/sparrow-mixed61/`. A historical same-machine ten-second observation reached `152.49449 mm`, but its raw solution was not retained and must be replicated before it becomes acceptance evidence.

The practical target is therefore the demonstrated `154-155 mm` band, with approximately `150 mm` as an aspirational next boundary, all measured as strip depth rather than square millimetres. The protected current endpoint is independently valid at `168.361 mm`; reaching `154.44858 mm` would recover `13.91242 mm`. This is a feasibility bound from an independently validated external run, not a claim that the present lifecycle can already reach it or that Lapas uses Sparrow internally. New work should seek Pareto progress toward that band and report quality, runtime, and memory together. The provisional six-second diagnostic gate is a causal-screen guard rather than product truth: a material and reproducible quality improvement may justify a seven-second result, but must be reported as a time/quality trade-off instead of silently relaxing the gate.

The retained Sparrow bundle proves only the final `154.44858 mm` layout; it does not retain enough intermediate telemetry to attribute that result to one upstream phase. Our own controlled constructor-order experiments still show that multiplying greedy order variants does not close the gap. Constructor order and local placement-score changes are therefore no longer the default response; new experiments must instead test a distinct state transition, retain useful failed trajectories, or improve the overlap/search objective.

The existing continuous dynamic-hazard path does not close that gap by configuration alone. At the fixed 24-epoch quota, continuous global/focused seeds reached 179.901 mm independently with continuous triangle pressure and 177.134 mm with dynamic-pole pressure. Adding Sparrow's coarse and fine rotational coordinate-descent steps to that path in isolation produced no accepted exact improvement and returned the 184.476 mm constructor. The change was removed. Continuous angular refinement is retained as a required capability, but only after the overlap objective and state-transition lifecycle provide a useful gradient; it is not promoted as an isolated motion-axis transplant.

A controlled whole-polygon coverage-pole replacement had already tested the missing surrogate-construction hypothesis in isolation. Replacing the triangle-incircle proxy with coverage-controlled whole-polygon poles regressed the same Mixed-61 contract to approximately 183.8 mm independently in 4.84 seconds and introduced exact-boundary rejections, so the implementation was reverted. A later review recovered that result before a duplicate experiment was implemented. This closes standalone coverage-pole substitution, including a coverage-pole plus hull-factor crossed arm: such poles may be reconsidered only as part of a materially different tracker and nonterminal search lifecycle with a separately attributable control.

Enabling the existing Umetani/Egeblad-inspired NFP whole-axis minimizer on every local refinement was also rejected. It reached 172.855 mm independently instead of the 172.348 mm control, increased solver time from approximately 3.2 seconds to 10.1 seconds, built 523,900 pair NFPs with 9,391,852 components, and generated 122,537,344 axis events. The mechanism is not discarded, but it must be restricted to a small stalled-collision neighborhood with cached pair geometry; applying it to every sampled move destroys the intended cost advantage.

A separate rollback-only pressure-model ablation tests literal Umetani cardinal penetration without enabling whole-axis event search. For every SAT-confirmed colliding pair at fixed catalogue orientations, it evaluates `min(rho_x, rho_y)` over the union of convex-cell pair NFPs, without pole pressure, shape-difficulty scaling, or pair normalization. It preserves the legacy boundary penalty in the first experiment so the pair-loss change is attributable, and records pair-versus-boundary distributions rather than claiming dimensional equivalence. Candidate scoring atomically preflights all sorted pair keys and both component-visit and cache-allocation caps before inserting any NFP; an over-budget candidate is unscorable, an initially unscorable lane cannot enter reduction, and no pole fallback may mix objectives. Contractual-grid interval operations make every SAT-positive collision either strictly positive or a fail-closed recorded model inconsistency. This is an opt-in measured ablation, not the global normalized-pole contract or a production default.

#### Disabled stage-two directional repair ablation

The first directional measurement retains the legacy boundary term and random continuous move machinery only to isolate the pair metric. Stage two is a separate disabled experiment:

- directional collision classification bypasses all absolute-`f64` indexes and bounds filters, quantizes absolute placements independently, subtracts checked grid integers, and uses strict `i128` triangle SAT; unrepresentable coordinates fail closed;
- contraction computes every expanded oriented surrogate's inner-fit rectangle on the canonical grid, rejects empty intervals atomically, and relocates protruders in stable piece-ID order using targets derived only from seed, lane, epoch, axis, and piece ID before committing the batch;
- after relocation, containment is a structural invariant and is excluded from the directional objective; every candidate stays inside its exact closed inner-fit rectangle, while any containment failure rejects the candidate or contraction;
- orientations and mirrors remain fixed during the first translation-only experiment;
- cardinal-axis interval endpoints are a bounded candidate generator for an axis-specific surrogate, not an exact minimizer of the full directional objective; retained candidates are fully rescored with `sum(w_ij * min(rho_x, rho_y))`, accepted only on strict improvement, and alternating search stops after a complete cycle with no accepted move;
- translation lanes atomically preflight only the NFP keys of the fixed orientation assignment under component and estimated-byte caps; future rotation perturbations use one disposable bounded scratch cache and can promote a new assignment only after complete exact rescoring and atomic cache/tracker preflight; and
- the experiment passes only after producing a complete independently exact-valid state, not merely a surrogate-feasible state. Mixed-61 remains bounded by the documented cold-process time gate, and a result above the structured-pole depth is diagnostic progress rather than a quality win.

Reserving lane zero for Sparrow's deterministic center-strip compression while leaving the other seven lanes unchanged was not beneficial in this engine. It reached 174.717 mm independently in approximately 3.25 seconds, versus the 172.348 mm random-split portfolio control. Center compression is meaningful inside Sparrow's full separation lifecycle, but replacing one of this engine's split positions removes useful diversity without importing that lifecycle.

A selected-state-only axis escalation for layouts with at most two remaining collision pairs was cheap but ineffective. Across the fixed run it built 1,115 pair NFPs with 19,005 components, generated 66,032 axis events, and evaluated 153 retained axis candidates without producing a new exact-valid incumbent; runtime remained approximately 3.2 seconds. The failure confirms that the 20 mm quality gap is created before terminal repair, in topology survival and successor selection, rather than by one unresolved cardinal-axis move.

An independently reviewed two-endpoint escape then isolated that conclusion from cluster transport. On each one-pair stalled directional lane it tested both endpoint orders, compared an arm that temporarily omitted only the mutual row with an equal-work ordinary-axis control, rebuilt every incident directional row, and required bit-exact agreement with a complete tracker before accepting only a strict feasible win. Across the fixed Mixed-61 run it attempted 33 stalled lanes and evaluated 1,786 retained endpoint candidates without one feasible sequential state or control state; runtime remained approximately 3.59 seconds, with zero quota failures and zero tracker mismatches after two scorer repairs. The operator was removed. Its invariant check exposed and permanently fixed two defects: moved-piece scoring now constructs every directed NFP in canonical pair-index order, and complete trackers install the current guided weight on noncolliding as well as colliding rows. Terminal cardinal repair is closed for this design; the next experiment must introduce topology-diverse states at the contraction boundary, before a lane collapses to its final collision pair.

Deterministic contraction-boundary splits then tested that earlier intervention directly. Quarter, center, and three-quarter cuts preserved the strong edge-clamp lane and translated the region beyond each cut before separation, matching Sparrow's compression topology without importing its stochastic schedule. At 40 sweeps the arm produced exact-valid contractions to 182.763 mm reported and 182.511 mm independently reconstructed, but raised solver time from approximately 3.5 seconds to 6.75 seconds for a sub-2-mm gain. Fifteen and twenty split sweeps preserved that depth but still required approximately 5.95 and 5.99 seconds; ten sweeps returned to approximately 3.1 seconds but produced no contraction. The split arm was removed. An independently reviewed immutable ordered-pair NFP table was retained because it changed neither layout nor logical lane budgets while reducing Mixed-61 process RSS from roughly 32-36 MiB to 17-19 MiB; a five-run edge-clamp replay preserved 184.728/184.476 mm reported/independent depth with a 3.105-second median. This establishes that repeated NFP construction was a memory duplication problem, while split-induced collision density and exact scoring were the time cost.

The dormant slot-swap ejection chain also failed as a local operator. In improving-only mode it evaluated 6,935 two- and three-piece swaps, accepted none, and reproduced the 172.348 mm independent control. Allowing the already-disrupted lanes to accept bounded worsening chains evaluated 7,004 candidates and accepted 89, but regressed to 174.319 mm independent depth while increasing wall time and peak memory. This rejects slot exchange as an in-sweep perturbation; any retained topology-changing successor must restore from a separately preserved infeasible state, transport its local neighbourhood, and receive a complete separation attempt without consuming the control portfolio.

### Added-work disrupted successor experiment

The next bounded experiment keeps the current eight-lane batch byte-for-byte as the control. If that batch contains no feasible state, it selects its best infeasible outcome by calling the existing comparator directly: common loss, boundary loss, boundary-violation count, collision-pair count, canonical state, then ordinal. A ninth, additional-work successor then:

1. clones the selected outcome, including guided pair weights;
2. applies one seeded large-piece swap with contained-neighbour transport using the Sparrow-derived `disrupt_state` lifecycle, after correcting that helper to compose the complete old/new rotation, reflection, and translation frames and to classify neighbours with a verified material-interior point rather than an AABB center;
3. runs a complete ordinary separation attempt at the same target depth and sweep budget;
4. joins the publication reducer only after the eight control lanes are complete; and
5. can become the fallback parent through the unchanged comparator when it is surrogate infeasible or when it is surrogate feasible but exact rejected, while only an exact-valid feasible outcome may update the protected incumbent.

The successor must use separate disruption and search seeds from a dedicated domain that cannot be confused with normal lane ordinals or the angular-repair tags; reducer ordinal 8 is only a tie-break. The fresh search explicitly receives the cloned guided weights. Diagnostics record the source lane, seed domain, source/post-disruption/final objective and violations, successor-only work, surrogate/exact outcomes, a fallback-parent reason enum (`None`, `SurrogateInfeasible`, or `ExactRejected`), exact-feasible selection, protected-incumbent improvement, and lineage without prevalidating or reordering the reducer. Neither fallback reason may update the protected incumbent. The experiment is rejected if it fails to beat 172.348 mm independently on Mixed-61, exceeds six seconds without reaching 165 mm, changes a fixed-seed result across repeated runs, or regresses the general-concave feasibility fixture. A positive single run is only a directional signal; retention requires repeated timing/quality samples and the protected corpus.

The added-work successor was rejected after implementation. With twice the normal separation sweeps it became feasible early, replaced the control trajectory, and regressed to 176.442 mm independently measured in 4.44 seconds wall time with 26.8 MB peak RSS. Restricting admission to surrogate-feasible successors and giving it three times the normal sweeps reached 170.344 mm independently, but required 5.27 seconds and 26.4 MB. A roughly 2 mm gain for roughly 1.6 seconds of additional wall time misses both promotion routes and still depends on collapsing an auxiliary trajectory into the sole parent. The experimental code was removed. Any future disrupted successor must retain a fully independent anytime incumbent and search trajectory rather than replacing the control parent at an intermediate compression target.

The first reducer audit found that independent lanes were collapsed by surrogate loss and canonical key before publication depth was reconstructed. Surrogate-feasible lanes all have zero collision loss, so this discarded the optimization objective at the merge boundary. Serially validating every feasible lane and then comparing exact depth improved the fixed Mixed-61 24-epoch result from 173.006 mm reported depth to 172.635 mm, with 172.348 mm independently reconstructed, in approximately 3.2 seconds and 25.7 MB peak RSS.

A separate equal-work two-parent experiment retained the best two infeasible states with distinct collision/boundary/frontier signatures and split the same eight lanes four-plus-four. It regressed to 176.955 mm when activated after each failed compression. Restricting it to four consecutive non-improving epochs still produced 172.909 mm at 24 epochs, while the single-parent control produced 172.635 mm. At 32 epochs the control reached 171.631 mm and the retained-parent arm remained at 172.909 mm. That branch is therefore rejected. Future population search must add genuinely different successor operators or additional bounded work; merely dividing the existing lane portfolio across multiple parents reduces the strong control arm without creating useful diversity.

A source-level Sparrow comparison exposed its minimum-raw-loss snapshot across guided-weight updates. Retaining the analogous per-lane snapshot at identical work regressed the fixed Mixed-61 run to 176.475 mm reported and 176.200 mm independently reconstructed, versus the 172.635/172.348 mm control, while solver time remained approximately 3.1 seconds and peak RSS approximately 25.8 MB. The isolated transplant is therefore rejected. In Sparrow, raw-loss rollback participates in a larger lifecycle with compression, an infeasible solution pool, and explicit exploration; replacing this engine's final guided state without those surrounding transitions discards useful guided progress.

A faithful Umetani-style active/inactive sub-neighborhood scheduler was then tested without changing the protected constructor or exact publication gates. It began with every piece active, deactivated a piece after no strict weighted improvement, reactivated pieces colliding with an accepted move before or after that move, updated guided weights only at a local optimum, and capped work at `pieceCount * sweepsPerEpoch` neighborhood searches. On the fixed edge-clamp Mixed-61 route it reproduced the 184.728/184.476 mm reported/independent control exactly, but raised the three-run median from 3.015 seconds to 19.898 seconds. It consumed 175,680 neighborhood searches, 1,149,713 surrogate evaluations, and exhausted the per-lane cap 72 times without one exact incumbent improvement; process peak RSS rose from approximately 18.1 MB to 21.1 MB. The implementation was removed. Active-neighborhood scheduling is not rejected as a principle, but it cannot rescue the current candidate/loss surface in isolation; any retry must first produce a materially stronger bounded move gradient and compare equal measured work rather than only equal worst-case quotas.

A bounded top-collision directional proposal experiment also failed. Restricting directional axis events to the highest one, two, four, or eight guided collision pairs reduced the edge-clamp route from approximately 26.0 million axis events to approximately 7.8-8.2 million and improved its independent depth only from 184.476 mm to 184.038 mm at roughly three seconds. Reusing that generator inside the stronger structured-pole route under the same refinement-evaluation budget was worse: top one produced no exact improvement, while top two and four regressed the 172.669 mm, 3.53-second control to 179.819 mm in approximately 4.21-4.26 seconds. The implementation was removed. This rejects cardinal NFP minima as both the primary directional surface and a structured-search proposal supplement; the remaining gap requires a state/topology transition rather than another locally rescored axis candidate.

A terminal two-piece vacancy probe then tested whether the existing exact contact generator could realize that topology transition cheaply. It selected eight deterministic high-frontier root/blocker pairs, evaluated both insertion orders, and compared temporary two-piece removal with an identical-generator no-vacancy control. Each arm completed all sixteen schedules, generated 3,609 poses, retained 126 complete candidate rows, spent approximately 7,500 exact pair queries, and sent 32 complete layouts through both publication gates. Every validated layout was a non-improvement: both arms returned the 172.934 mm reported and 172.669 mm independently reconstructed incumbent. The incremental engine cost was approximately 0.1 seconds and peak RSS remained approximately 27 MB. The implementation was removed. This closes cheap pair removal that merely reuses the current contact placement objective; it does not reject an Imamichi-style NFP-boundary pair neighborhood or a future operator with a demonstrably different placement objective.

A stricter terminal pair-beam screen then tested whether four diverse first-insertion states plus explicitly reserved incumbent, sheet-aligned, and partner-conditioned continuous orientations could recover the missing permutation. Cheap snapped-geometry/AABB ranking was separated from exact confirmation and the whole job remained below the durable 128-row, 8,192-query, and 32-validation ceilings. On the fixed Mixed-61 route it executed all sixteen schedules, cheaply evaluated 1,024 first poses and 1,200 second poses, retained 30 exact-confirmed first states, confirmed 94 rows with 5,576 exact pair queries, and produced 30 complete candidates. Twenty-nine distinct layouts passed both publication gates, but every one was a non-improvement; the best candidate measured 272.606 mm, rank zero won, and the four-state beam produced no comparator inversion. Total engine time rose only from approximately 3.49 seconds to 3.56 seconds and peak RSS from 29.4 MB to 30.1 MB, so cost was not the limiting factor. The implementation was removed. This rejects cheap AABB-proxy finalist selection as the bridge from contact proposals to a useful two-piece topology transition: it collapses the beam before exact geometry can expose a useful partial. It does not reject the separately specified NFP-boundary beam with round-robin pose allocation and bounded exact confirmations.

The full canonical union-NFP pair beam closed that remaining variant. It built exact union boundaries from triangle-pair NFP components, normalized mixed winding and collinear output deterministically, and round-robined boundary vertices across sixteen contact streams. The fixed Mixed-61 treatment completed all sixteen schedules with 1,024 first poses, 2,490 second poses, 288 refinement poses, 184 NFP builds, 2,018 convex components, 6,301 canonical boundary vertices, and approximately 344 KB of estimated cache. It retained twenty-two first partials and twenty-two complete candidates; ten distinct layouts passed both publication gates, all ten were exact-valid non-improvements, rank zero remained best, and the best layout exactly reproduced the 172.934 mm reported and 172.669 mm independently reconstructed incumbent. Two deterministic treatment replays took 3.576 and 3.589 seconds engine time, versus a 3.526-second untouched control, and peak RSS rose from 29.7 MB to 31.0 MB. The implementation was removed. Full NFP topology at the proposal boundary is therefore not enough when the terminal operator still ranks and retains partials by the same depth/envelope objective; the next useful experiment must change the nonterminal state transition or partial-layout objective rather than add another contact representation to two-piece reinsertion.

A constructive exact-free-region probe then tested `IFP ∖ union(NFPs)` at every insertion rather than only at a terminal pair boundary. It supported concave collision polygons through deterministic ear clipping, triangle-pair Minkowski components, canonical non-zero union/difference, and exact revalidation. The four-orientation arm stopped atomically at its reviewed 65,536 emitted-vertex cap after 227 beam states and 905 free-region builds. An equal three-orientation schedule completed all 61 pieces and four final layouts under every cap, where its sampled-contact control completed none, but its best collision depth was 222.066 mm versus the protected constructor's 184.728 mm. Forcing that exact-valid seed through the unchanged 24-epoch, eight-lane relaxed search ended at 193.385 mm independently reconstructed, versus the protected path's 172.669 mm. The implementation was removed because every absolute promotion gate failed. Because the equal-policy contact control was incomplete, this does not causally close exact free-region topology or prove that the lifecycle alone caused the regression; it shows only that this bounded topology-plus-current-objective construction did not earn retention. Future exact topology work must arrive with a demonstrated nonterminal operator and a successful equal-policy control.

The next source-faithful scalar ablation replaced the triangle-pole shape factor based on bounding-box area with Sparrow's convex-hull-area factor while preserving the same decay equation, pole set, lane schedule, and work quota. It regressed Mixed-61 from 172.669 mm independently to 179.771 mm, although elapsed engine time fell from approximately 3.59 seconds to 3.35 seconds. The implementation was removed. This demonstrates that Sparrow's shape penalty is coupled to its pole-of-inaccessibility surrogate and search lifecycle; applying the coefficient to this engine's triangle-incenter poles does not reproduce Sparrow's objective and actively damages the useful gradient.

A reserved-lane Egeblad exact-overlap finalist reranker then isolated whether the pole proxy was merely ordering otherwise useful endpoints incorrectly. It preserved the structured generator and refinement schedule, exact-scored at most four finalists using boundary loss plus guided-weighted intersection area, and atomically fell back above 64 broad-phase-confirmed pair queries per move. The compute-but-ignore control exact-scored 3,822 finalist sets with 45,243 Clipper intersections, observed 960 rank inversions, and reproduced the 172.934/172.669 mm reported/independent control. Treatment changed 1,351 selections across 3,927 finalist sets and regressed sharply to 182.277/182.010 mm. No cap or query fallback occurred. The implementation was removed. Exact material-overlap area is therefore not an absent local tie-break in this lifecycle: it frequently disagrees with the proxy, but following those disagreements damages the trajectory. The next experiment must change state retention and separation lifecycle rather than substitute another local scalar.

A bounded retained-infeasible lifecycle then tested that state-transition hypothesis directly at one fixed 165 mm target. Both arms started from the same deterministic compression, ran eight attempts with eight workers and 40 synchronized sweeps per attempt, and applied the same seeded large-piece disruption between attempts. The control always restarted from the compressed feasible parent; treatment retained each attempt's minimum-raw-loss snapshot in a bounded pool and selected a deterministic biased rank before disruption. Neither arm produced a single surrogate-feasible state, so neither reached exact publication. Treatment reduced the best observed raw overlap loss from 1,596.150 to 974.806 and its added phase from approximately 1.18 seconds to 0.86 seconds, but the protected 172.934/172.669 mm layout remained unchanged and total runtime rose from approximately 3.44 seconds to 4.33 seconds. The implementation was removed. Retaining failed states is directionally useful, but a terminal restart loop at an aggressive fixed depth still lacks the nonterminal layout transitions needed to cross into feasibility; the next lifecycle experiment must create and preserve useful partial structure instead of repeatedly repairing a fully compressed complete layout.

An incremental retained-parent experiment then removed the aggressive fixed target as a confounder. Starting from the same 172.934/172.669 mm reported/independent incumbent, it contracted by 0.1% only after each strict exact-valid improvement, allowed four same-seed attempts per target, and compared ordinary feasible-incumbent restarts with a six-state pool of immutable proxy-ranked infeasible parents. The restart control reached 172.037/171.785 mm in 4.60 seconds engine time and 29.4 MB peak RSS. Retained-parent treatment selected pooled states on three later attempts but stopped at 172.588/172.331 mm in 4.09 seconds and 29.4 MB: 0.546 mm worse than its same-seed restart control and far short of the required one-millimetre advantage over both controls. The implementation was removed. Cheap incremental restarts can recover approximately 0.884 mm beyond the protected baseline, but retaining complete infeasible states under the current proxy and disruption operator is not the missing lifecycle; future population work must change the nonterminal transition semantics rather than preserve more instances of the same local state.

A source-faithful Sparrow separator lifecycle then kept eight workers persistent across guided-weight rounds, merged them by weighted loss, retained the minimum raw-loss snapshot, and rolled back to that snapshot after ten non-improving rounds while preserving current weights. Both arms attempted up to eight consecutive 0.1% contractions from the protected incumbent; the quota-matched control omitted only rollback and strikes. From the 172.934/172.669 mm reported/independent baseline, control reached 172.409/172.150 mm after three accepted contractions in 3.68 seconds, while treatment reached 172.176/171.915 mm after four accepted contractions in 3.55 seconds. Treatment beat control by 0.235 mm and baseline by 0.754 mm independently, used six rollbacks with seven full-rescore agreements, and stayed below 30 MB peak RSS, but missed the predeclared at-least-one-millimetre promotion gate. The implementation was removed. The lifecycle is causally useful and cheap, but another local contraction wrapper is not enough to close the quality gap; the next retained experiment must alter the conflict objective or introduce a stronger nonterminal topology transition while preserving the weighted/raw separation discipline.

Reapplying continuous rigid coordinate descent inside the later dynamic-pole persistent separator produced the first material retained improvement. A translation-only control reached 171.377 mm independently, while the equal-lifecycle treatment added only the rotation axis to the existing refinement budget and reached 170.097 mm independently. Three cold replays reproduced fingerprint `4ef3b69d9eefbc4697ecf393cff7e9c4e942a24d539277c24e969c8d73069080`, approximately 3.8 seconds engine time, and 35 MB peak RSS. The rigid refinement is retained. Continuing the failed target from 40 to its natural three-strike stop at round 54 did not improve raw loss or depth, so a larger round cap is not the missing mechanism.

Three subsequent source-difference screens were negative. Increasing each move from 10/10 global/focused samples to Sparrow-like 50/25 regressed to 172.502 mm independently and accepted only one contraction. Replacing the mild guided-pair multiplier with Sparrow's more aggressive update regressed to 170.991 mm and exposed a rollback/full-rescore disagreement; the arm was removed rather than weakening the invariant. A reviewed discrete feasible-angle sampler then used 16 fixed launch angles, canonical inward-rounded translation domains, the same 10/10 proposal ceilings, and unchanged rigid descent. Its contemporaneous control reproduced 170.350/170.097 mm reported/independent depth, the frozen fingerprint, and every frozen work counter exactly; treatment stopped at 170.866/170.607 mm. The sampler code was removed. Sparse proposal density, fixed launch-angle selection, aggressive GLS scaling, and extra terminal rounds are therefore closed as isolated explanations for the remaining gap.

Removing the engine's periodic non-colliding frontier-blocker moves also failed to expose a hidden source-faithful win. With rigid refinement in both arms, the current forced-blocker control again reproduced 170.097 mm independently, while a colliding-only treatment stopped at 170.447 mm. The extra blocker motion is therefore retained as a useful legacy diversity operator. The remaining architectural gap is not explained by moving too many pieces in an otherwise unchanged complete-layout sweep.

A reviewed source-faithful large-piece transition screen then corrected three material deviations in the earlier retained-parent probe: it used Sparrow's multiplicity-weighted convex-hull cutoff, discovered follower interior poles only after both roots were swapped, and evacuated the destination neighborhood through the full new-root-to-old-root rigid transform. A common persistent-separator prelude accepted four 0.1% contractions and failed at a 172.071 mm target with minimum raw loss 181.802. On the single paired retry, the compute-but-ignore control restored exact-valid feasibility at 172.071 mm after 19 rounds and 38,836 surrogate evaluations. The transitioned arm found zero followers for both selected roots, exhausted its 32-round quota without feasibility, and ended at raw loss 419.092 after 159,924 evaluations. Two replays were diagnostics-identical; wall time was 3.91 and 3.57 seconds and peak RSS was 29.7 and 29.9 MB. The implementation was removed because the transition failed its mechanism gate. This closes that bounded destination-evacuation operator on Mixed-61: the faithful follower mechanism was inactive and the naked large-piece swap damaged the basin. It does not reject minimum-raw retry, which succeeded, or topology operators whose vacancy construction does not depend on one piece containing another piece's interior pole.

A source-faithful Sparrow boundary-weight ablation then extended guided local search from pair conflicts to per-piece container violations. The computation-only control evaluated and updated the combined pair/boundary schedule but continued to score boundary loss at weight one; it reproduced the 172.934/172.669 mm reported/independent baseline exactly in 3.47 seconds with 29.6 MB peak RSS. Applying the resulting boundary weights changed the search trajectory but regressed to 174.584/174.312 mm in 3.44 seconds with the same peak RSS. The implementation was removed after the first predeclared directional run. Boundary weights are useful in Sparrow's full separator lifecycle, but they do not improve this engine's structured-pole local objective in isolation and are not the missing topology transition.

A source-faithful synchronized-reducer ablation corrected a real mismatch: synchronized workers had been merged by raw loss, while Sparrow merges by guided weighted loss and tracks its minimum raw-loss snapshot separately. With identical worker outcomes, the control observed 2,910 pairwise rank inversions and 226 winner disagreements over 534 sweeps. Following the weighted winner changed the trajectory but improved the synchronized route only from 182.245/181.992 to 182.206/181.954 mm reported/independent, with both arms at approximately 1.7-1.8 seconds and 29 MB peak RSS. The implementation was removed. The reducer scalar mismatch was real but not causal for the quality gap: the synchronized one-sweep master topology remains materially weaker than the independent portfolio.

The existing forced-move selector historically ranks `translateY + unrotatedSourceMaxY`; it is an orientation-anchor diversity heuristic, not a physical frontier measurement. Replacing it in place with transformed source `maxY` regressed the fixed run to 180.792 mm reported and 180.311 mm independently reconstructed at approximately 3.0 seconds. The control operator therefore keeps its old semantics under an explicit legacy name. New topology operators must use the separate transform-aware frontier function, whose rotated-asymmetric regression prevents the same conceptual mistake from entering ruin selection.

An exact-feasible forced-vacancy screen then isolated whether the protected contact constructor could exploit a one-piece vacancy without preserving a second removed piece as a blocker. Control and blocker arms each evaluated sixteen identical schedules, consumed the full 8,192 exact-candidate budget, and produced sixty-four complete exact-valid layouts in approximately 0.255 seconds of incremental engine time. Neither arm found one strict improvement over the 171.377 mm independently reconstructed endpoint; the best schedule candidates were materially worse, mostly between approximately 207 and 316 mm, and moving the blocker to the end of the fixed-piece frontier order barely changed the candidate population. Peak process RSS remained below 38 MB. The implementation was removed. This closes exact-feasible single-vacancy reinsertion under the current depth/contact objective; it does not close a deliberately infeasible ruin/recreate transition whose overlap-minimizing placements are handed back to whole-layout separation.

The next ruin/reinsert experiment must not reuse the removed infeasible-pool placeholder. Removal is represented explicitly by an active-piece bitset; the removed pieces' tracker rows and dynamic-index hazards are removed atomically, and every surviving row remains keyed by stable input-piece ID. Reinserted continuous angles require bounded on-demand exploration geometry rather than the rollback backend's 2.5-degree catalogue. A terminal-only arm may publish a fully reinserted state after exact validation but may not resume rollback search. Any arm intended to resume rollback search must first install every retained continuous orientation into the surrogate/index boundary under a deterministic quota. Off-sheet coordinates and AABB-only contacts are not valid removal or reinsertion semantics.

The first terminal-only arm removes exactly two pieces so it can test a permutation change without introducing three-piece combinatorics. A job with fewer than two placed pieces skips the arm with a structured reason. It ranks roots by transformed source `maxY`, then material area, then stable piece ID. For each of the first eight roots, it excludes that root and derives one blocker from a deterministic geometric blocker order; this is a search selector, not a claim of mechanical support. Every value is derived from transformed material contours on the contractual integer grid. Positive short-axis AABB projection overlap ranks before no overlap, followed by overlap length descending. A blocker whose material `maxY` is at or below the root material centroid ranks first. Pair clearance slack is the minimum squared segment-to-segment distance over every canonical ring and edge pair minus the squared requested clearance, compared as an exact rational. For positive short-axis overlap, vertical gap is `max(0, root.minY - blocker.maxY)` in grid units; for no overlap its key is `i64::MAX`, so a lateral piece cannot masquerade as vertical support. Stable piece ID follows. Canonical region, ring, and edge order breaks feature ties. A non-representable or overflowing distance excludes that blocker with a diagnostic; no `f64` comparison may silently decide this order. A root with no eligible blocker is skipped, and zero resulting pairs skips the whole arm. This remains defined for a feasible incumbent and does not depend on vanished collision pressure or lane weights.

Each root/blocker pair is evaluated in both reinsertion orders. At most sixteen pair/order schedules exist; if fewer than eight pairs are eligible, the absent schedules' slices remain unused. Every existing schedule receives a fixed 256-pose slice and schedules advance in deterministic round-robin quanta, so an early schedule cannot consume later work. Within each slice, 64 poses are reserved for the first insertion, 160 for the second, and 32 for refinement; unused quota is not transferred across phases or schedules. First-insertion poses are round-robined one at a time over canonical `(mirror, angle-key)` streams. The first insertion retains a four-state partial beam, deduplicated by canonical absolute geometry and sorted by `(partialDepth, candidateLongAxisPosition, unusedShortAxisProjection, occupiedEnvelopeArea, canonicalPlacementKey)`. Diversity selection keeps the best overall, then the remaining candidate with maximum circular angle distance from it, then the best opposite-mirror candidate, then the next comparator-best candidate, with canonical placement keys resolving every tie. Second-insertion poses are nested-round-robined one at a time over beam rank and that partial's canonical orientation streams; all surviving partials receive one pose before any receives its next. Refinement poses are similarly round-robined over the best two complete states and their twelve ordered `(piece, delta)` streams for plus/minus 5, 2.5, and 1.25 degrees. Empty streams are removed, and their work may be redistributed only within the same schedule phase.

Angle seeds are derived independently for each legal candidate mirror after reflecting the moving source edge. For first insertion, paired-edge alignment targets the partner's incumbent mirrored-and-rotated world longest edge. For second insertion, it targets the first piece's newly selected mirrored-and-rotated world longest edge. The required candidate rotation is the normalized difference between that world target direction and the separately reflected moving-source direction. The other seeds are the current angle and the reflected stable longest source edge aligned to the sheet short and long axes. Transform seeds are deduplicated by `(mirror, 10^-6-degree angle key)` under the schedule's pose slice; an unmirrored edge angle is never reused as the reflected alignment by assumption.

For every transform seed, authoritative proposal geometry follows the same order as publication: transform the flattened source, snap to the contractual grid, then apply the collision expansion. Translation events come from sheet supports and clearance-expanded moving-vertex/fixed-vertex contacts plus both expanded vertex-to-edge projection directions, ordered by stable piece/ring/edge/vertex/key. Literal coincident source-feature contacts are forbidden because they violate the pair-clearance contract. Refinement begins only after both pieces have been reinserted: the best two complete states receive angular deltas of plus/minus 5, 2.5, and 1.25 degrees in that order; every refined angle rebuilds its expanded contour in the same transform-snap-offset order.

Pose evaluations are not the only bounded work. The whole arm also caps transformed-orientation builds at 512, canonical offset-output vertices at 262,144, source feature visits at 131,072, pre-dedup contact attempts at 131,072, and generated proposals at 32,768. Each counter is job-wide and deterministic. Reaching any cap rejects the experimental arm atomically and records which limit was exhausted; a partially explored arm cannot publish. Diagnostics distinguish feature visits, contact attempts, deduplicated proposals, orientation builds, offset vertices, pose evaluations, retained pair confirmations, and complete validations.

The whole arm is capped at 4,096 candidate-pose evaluations, 128 retained `f64` candidate-row confirmations, 8,192 individual exact pair queries, and 32 complete dual-gate layout validations. A candidate-row confirmation counts once only after all of its fixed-piece pair tests complete. Before starting a row, the arm computes its exact pair-query cost from the active fixed-piece cardinality; if the remaining pair-query quota cannot cover the whole row, the arm exhausts atomically without retaining a partial confirmation. Before either publication gate, a candidate must have every active bit restored, placement cardinality equal to the input-piece cardinality, and a duplicate-free stable-ID set exactly equal to the requested input IDs; failure is a structured incomplete-reinsertion rejection. Two controls start from the same incumbent. The operational control applies the existing rollback placement search to the same ordered root/blocker pairs under 4,096 `surrogateEvaluations`. The causal control uses the experimental arm's exact generator, schedule slices, partial beam, confirmations, and validation quotas, but never removes both pieces: it evaluates both sequential move orders while the partner remains active, so only the temporary two-piece vacancy differs. Work is reported rather than falsely equated: proposal, feature, geometry-build, candidate-row confirmation, exact-pair-query, validation, elapsed-time, and peak-RSS counts remain separate for all three arms. Both controls have the same 128 candidate-row confirmation, 8,192 exact-pair-query, and 32 validation ceilings, and unused quota is not transferred. All arms record removal sets and orders, contact and angle candidates, budget-pruned candidates, partial failures, completed layouts, incomplete-reinsertion rejections, canonical-gate failures, independent-gate failures with structured offending IDs, exact-valid non-improvements, strict improvements, depth gain, phase time, and peak RSS. An exact-valid non-improvement is not an exact rejection. The ruin arm remains disabled unless it produces a deterministic strict exact-valid improvement, beats both controls, and reaches the documented quality/time gate; because it starts after the approximately 3.2-second baseline, default promotion must reach the separate at-most-165-mm within-six-seconds route.

The later exact-terminal boundary experiment identified a real oracle mismatch without weakening exploration. The retained rigid separator stopped at `170.097 mm` because Jagua's guarded exploration envelope classified several exact collision contours as outside the strip. Keeping that guard for search but inward-clamping boundary-only terminal pieces on the canonical grid, followed by independent publication validation, accepted five additional contractions and reached `168.361 mm`. Removing the guard globally was rejected because it changed the deterministic trajectory and regressed to `172.150 mm`. Authoritative tracker rows and counts remain byte-exact; one-ULP agreement is permitted only for derived floating sums after a full rescore showed identical rows.

A capped projected-root repair then tested whether that terminal overlap exposed a useful local topology transition. It spent 6,912 cheap queries, 662 orientation builds, 432 exact finalists, and 27,639 exact pair intersections to reduce exact overlap from `0.035046` to `0.032290 mm2`; three overlap pairs remained, exact validation failed, and depth did not improve. The repair was removed from the active path. This closes larger terminal repair beams and retries: the next experiment must create compact partial structure before full-layout compression, not spend more work resolving microscopic residual overlap after the topology has already collapsed.

The first exact congruent-pair catalogue then confirmed that reusable local structure is cheap to enumerate: all 28 eligible Mixed-61 pairs retained four exact non-overlapping templates, for 112 templates total, in approximately 30 ms of catalogue time with no pair fallback. Mandatory rigid use of those templates failed the whole-layout mechanism gate. Every useful hard band from 138.835 through 259.924 mm became infeasible early; on the full 2700 mm sheet the rigid arm completed at 1770.012 mm while its unbonded same-orientation macro control completed at 1652.234 mm. Pre-feasibility truncation and under-sized accounting ceilings were corrected before this comparison, so the remaining loss is architectural: irreversible pair decisions remove necessary future placement options. The mandatory macro constructor remains diagnostic-only and is rejected. The cheap catalogue may be reused later for optional seeding or nonterminal neighborhoods, but not as a permanent bond or as evidence that pairwise clustering is generally ineffective.

Single-pair greedy seeding then removed mandatory binding and family-grouped continuation as explanations. Eight exact seeds preplaced one compact pair from the four largest distinct families at left or right sheet support, then returned the remaining 59 pieces to the unchanged winning ordinary constructor order. Every seed completed, but the best treatment exactly reproduced the 184.728 mm constructor control while consuming approximately 10.37 seconds and 1,131,530 candidate rows versus 141,132 control rows. This closes constructor-level pair seeding: the greedy continuation erases local structure at excessive cost. Any reuse of the catalogue must be a bounded nonterminal correlated move whose children become independent immediately.

A reviewed cold-process constructor-basin oracle then tested whether immediate constructor ranking was discarding a topology that the unchanged quality pipeline could compact materially better. The four supported order strategies produced distinct exact-valid constructor depths of 184.728, 190.945, 233.339, and 258.103 mm. After the identical 24-epoch, eight-lane rollback search and current coupled separator, their independently reconstructed exact-terminal projection depths were respectively 168.361, 180.331, 195.814, and 204.996 mm. The all-four production selector chose the long-span candidate and reproduced its constructor fingerprint exactly. Separate cold routes took approximately 4.34, 3.36, 2.30, and 2.08 seconds with peak RSS between 35.2 and 37.4 MiB; the non-timing selector control took approximately 4.79 seconds and 41.8 MiB. A second cold replay was byte-identical after removing timing fields. No losing seed reached the predeclared 165 mm gate or beat the selected route, so the diagnostic implementation was removed. Constructor-order basin preservation is closed: the next quality experiment must change partial-layout structure or the nonterminal lifecycle rather than spend more relaxations on the current greedy order family.

A source-faithful container shape-factor ablation then isolated the remaining scalar difference between the dynamic separator and Sparrow without changing the outside-area term. The control used the transformed collision-bounds-area square root; the treatment reused Jagua's already-built `f32` surrogate convex-hull-area square root, with no new hull construction. Fixed-state tests proved identical transformed bounds, overflow classification, violation counts, pair rows and pressures, guided weights, and publication outcome for rotated mirrored and fully disjoint placements; only positive boundary loss differed. Five interleaved cold runs per arm were deterministic after the exact diagnostic projection. Every control projection was byte-identical to the frozen `172.669 / 170.097 / 168.361 mm` reference. The treatment regressed the rigid separator and exact terminal projection to `172.317 mm` in every run. Its median engine and wall times fell from `4777.62 ms / 4.81 s` to `4130.00 ms / 4.16 s`, and median peak RSS fell from `45,547,520` to `41,320,448` bytes, but only because the worse trajectory performed far less search work. The treatment observed effective factor ratios from `0.659085` to `0.999969`; it did not create a better basin. The diagnostic implementation was removed. This closes the convex-hull boundary scalar as a quality mechanism while leaving Sparrow's distinct fully-disjoint centroid-distance branch untested; the remaining gap requires a nonterminal partial-layout lifecycle rather than further scalar fidelity work.

A reviewed order-preserving pair-shadow experiment then tested the last bounded way to reuse the exact congruent-pair catalogue without changing the proven piece order or permanently bonding pieces. The production control trajectory remained untouched. Four deterministic treatment states injected an exact pair template only when its first member reached its natural order position, made both pieces independent immediately, and continued through the same exact per-piece transition. All four states produced distinct independently valid complete seeds, but every seed regressed from the `184.728 mm` constructor control to `220.232 mm`. Passing every treatment seed through the unchanged full relaxation, coupled separator, and terminal projection produced `182.644`, `190.386`, `193.190`, and `185.029 mm`, all substantially worse than the retained `168.361 mm` control. The diagnostic required approximately `7.699 s`, `299,284` exact candidate rows, `410,244` fixed-piece visits, `10,821` generated proposals, and `13,041` proposal attempts before relaxation. The implementation was removed. This closes greedy pair-template shadowing, not pair geometry itself: a small state beam still commits topology too early. The next quality mechanism must retain a persistent population across multi-piece remove/reinsert transitions instead of grafting local structure onto a one-piece-at-a-time greedy lifecycle.

A target-frontier reconstruction then tested that population hypothesis before promotion. The independently reconstructed `168.361 mm` endpoint had twenty-four source pieces above the `165.000 mm` publication target and a `164.856 mm` immutable-background depth. Rebuilding only that protruding layer with the production contact generator completed cheaply, but the fixed-order and dynamic-class beams stacked it to `1345.885` and `1381.789 mm` respectively in approximately `44` and `130 ms`. This disproved the selector: pieces below the target are structural blockers even when they do not individually cross it. A throwaway all-piece reconstruction separated that failure from the background assumption. An eight-state dynamic beam with twelve exact candidates per expansion reached `2598.967 mm` in `0.353 s`; raising visibility to sixty-four exact candidates and four successors reached `1142.564 mm` in `0.909 s`. Reallocating work to one greedy state with full next-class choice and 512 exact candidates reached `228.476 mm` in `0.565 s`; increasing the same decisions to 4,096 exact candidates reproduced exactly `228.476 mm` while increasing incremental time to `2.285 s`. Every endpoint was independently source-valid, but none beat the `184.728 mm` retained constructor. The implementation was removed. Dynamic order is therefore not rejected generally; dynamic order plus the current myopic contact/depth transition is closed. A future population must survive infeasible multi-piece transitions or rank a materially different nonterminal objective rather than multiplying the existing greedy placement primitive.

A reviewed pre-compression frontier-vacancy screen then used the first failed exact-boundary target only as a selector, rewound to its `168.625 mm` authoritative parent, removed the three projected boundary offenders, and rebuilt them under the existing continuous-angle exact-overlap/frontier comparator before any compression. The rebuild exhausted its fixed `6,912` cheap queries and `432` exact finalists in approximately `46 ms`. A later validation audit corrected the original interpretation: besides the incumbent fingerprint, two distinct rebuilt children were source-valid at `168.625 mm` even though the conservative surrogate tracker still reported one boundary row; a fourth child had one exact overlap. Direct contraction from the first source-valid rebuilt child completed forty rounds and remained infeasible. Repairing the overlapping child at the old depth produced a distinct independently source-valid Stage A state, but its corrected Stage B also completed forty rounds without feasibility and ended at raw loss `1639.8386495144705`; the combined handoff crossed its predeclared layout-load cap. A final reviewed screen proved that the remaining directly valid child differed canonically from the incumbent and first child both before and after compression. Two deterministic cold runs took `5.39` and `5.03 s` with peak RSS below `51 MiB`, yet forty unchanged target rounds again remained infeasible and converged to the same minimum raw loss `1010.8935506900488` as the first direct child. The diagnostic implementation was removed. The exact evidence identity, child fingerprints, validation outcomes, and removed second-parent result are pinned in `docs/experiments/precompression-frontier-vacancy-evidence.json`; baseline mode preserves the frozen `0f39c64...` projection. This closes all independent retries from this four-child vacancy rebuild. The shared nonterminal target-search lifecycle, rather than which surviving child seeds it, is now the evidenced bottleneck; the next mechanism must let multiple partial or infeasible states interact across topology-changing moves instead of replaying the same separator from another complete parent.

A subsequent reviewed target-native screen contracted first, independently validated the fifty-eight-piece background inside `168.456375 mm`, and rebuilt the three projected offenders directly in that strip. Four complete states were retained; the best contained only `0.031953 mm2` of exact expanded-polygon overlap across three pairs. A real two-parent three-pose crossover changed the canonical state and cut the unchanged separator's forty-round minimum raw loss from the relocation control's `831.0346881516451` to `440.9107782545774`, versus the older `1010.8935506900488` floor. Nevertheless neither arm became exact-valid. Both control replays and both treatment replays were byte-identical after the frozen normalization, completed in `5.06-5.44 s`, and stayed below `52.2 MB` peak RSS. The diagnostic source was removed after its negative gate; exact identities, scores, resource counters, and replay hashes are pinned in `docs/experiments/target-native-frontier-evidence.json`. The target-native transition materially improves the surrogate basin but exposes the next defect more narrowly: three microscopic exact overlaps survive because the pole-pressure separator is not aligned with final material-intersection area. The next experiment must change bounded final exact-overlap separation or ranking, not add global rounds, retry old-depth parents, or rewrite the constructor.

A bounded exact-pair terminal repair then tested that narrowed hypothesis directly. It reconstructed the same four target-native parents, selected the frozen `0.031953 mm2`/three-pair parent, and rebuilt every moved collision from source before scoring authoritative Clipper intersections. Monotone local descent reduced total exact overlap only to `0.0313595 mm2` while retaining all three pairs. A second forced-elimination schedule spent four layers, `336` pose and collision builds, and `15,600` repair intersections; its successive best totals were `3.548009`, `0.088821`, `6.244761`, and `0.318559 mm2`, again retaining all three pairs. The latter cold run took `5.36 s` with `48,873,472` bytes peak RSS. The implementation is removed before the next retained build. This closes independent one-pair-at-a-time terminal nudges, including larger forced exits: the microscopic overlaps are symptoms of a coupled complete-layout local minimum, not an epsilon defect or a missing scalar radius. The next bounded screen moves every incident piece simultaneously from complete exact configuration-space constraints; if that fails, work returns to the already-evidenced persistent partial-layout lifecycle rather than another terminal repair.

That final complete-layout screen used exact configuration-space exits for every positive pair, split every relative exit simultaneously across both incident pieces within sheet slack, and compared damping factors `1/2`, `3/4`, `1`, and `5/4`. The independently fixed lanes removed one of the three constraints and reduced total overlap to `0.0018675` and `0.001025 mm2`, but remained infeasible. A reviewed shared-current selector then evaluated all four factors from the identical current state before every commit, with atomic complete-round preflight across exact-intersection, Clipper input, and conservatively bounded output work. It completed twenty-eight rounds under the unchanged `16,384`-intersection ceiling, selected factor `1` every time, and alternated between large and progressively smaller two-pair residuals until reaching `0.0004375 mm2`; it still never reached feasibility. Two mode-2 replays were byte-identical after timing normalization, took `4.906` and `4.983 s` of engine time, and peaked at `53,886,976` and `53,002,240` bytes RSS; the frozen mode-1 structural trace remained identical after excluding newly additive audit counters. Evidence is pinned in `docs/experiments/coupled-projection-evidence.json`. This closes exact terminal projection, adaptive factor choice, and larger complete-layout separation budgets as quality mechanisms: the remaining residual is an asymptotic Jacobi oscillation inside a fixed topology. The next experiment must maintain a persistent population of partial or infeasible states across topology-changing remove/reinsert transitions and rank nonterminal structure before reconstruction; another terminal cleanup is prohibited.

A reviewed persistent exact-valid vacancy population then replaced terminal repair with a real partial-layout lifecycle. Starting from the independently valid `168.361 mm` parent projected into a `165.000 mm` strip deactivated 25 boundary offenders. Under the same forty-layer, eight-state, two-piece-per-parent schedule, comparator retention restored ten pieces and ended with 15 inactive, while contact-signature retention restored twelve and ended with 13. Both stayed below `5.6 s` wall time and `47.3 MiB` RSS, and neither completed. The result is not a quality win yet, but it causally supports persistent topology diversity and disproves the earlier terminal-only framing: the engine can migrate an exact-valid vacancy cheaply across thousands of one- and two-blocker ejections. Durable evidence also exposed scheduler starvation—layers 25 through 39 generated 1,462 ejection children but only six direct insertions, leaving ten terminal inactive IDs never selected in that interval. The reviewed design, corrected counters, identities, and omissions are pinned in `docs/experiments/persistent-vacancy-population-design.md` and `docs/experiments/persistent-vacancy-initial-evidence.json`.

A same-work stateless rotation screen then isolated that scheduler defect. Replacing the second hard-piece slot with a stable-ID rotation improved comparator retention from 15 inactive pieces / `69619646821` inactive grid-area units to 12 / `60144097737`, while remaining deterministic at roughly `5.55 s`. The same scheduler combined with contact-signature retention tied the old treatment at 13 inactive pieces but regressed inactive area from `58797801045` to `62614709968`. Mode zero retained the then-used legacy commit-bound `0f39c64...` projection, old modes reproduced their legacy semantic hashes, shared-parent selector rows matched exactly, peak RSS stayed below `48.4 MiB`, and experiment-owned retained memory remained below `334 KiB`. The corrected stable projection is `f51f8d4...`; both source observations and both projection variants are pinned in `docs/experiments/persistent-vacancy-elite-evidence.json`. Evidence for the scheduler screen is pinned in `docs/experiments/persistent-vacancy-selector-evidence.json`. This closes the direct rotation-plus-contact-signature combination, not the scheduler or population lifecycle. The next causal variable is retention across the complementary low-count and low-inactive-area basins; more terminal rounds, a wider beam, and a single global comparator remain prohibited.

A reviewed dual-objective retention screen then tested that next variable without widening the eight-state population or changing successor work. Two post-build cold replays per arm were deterministic. The area-first mode `3` control preserved a best-ever 11-inactive-piece state at `59571041296` inactive grid-area units, while reserving fixed slots for the area-first and count-first elites in mode `6` regressed both best-ever objectives to 13 inactive pieces and `64577591268` units. The four runs took `6.07-6.19 s` wall time and at most `47.86 MiB` RSS; all three shared entering populations had byte-identical ordinary successor streams and work snapshots. Every engine sample missed the provisional `6000 ms` gate by `27-138 ms`, independently preventing promotion. The dependent mode-`5` incumbent carryover was not run because it could not isolate carryover from the already failed reservation policy. Durable evidence is pinned in `docs/experiments/persistent-vacancy-elite-evidence.json`. This closes fixed-slot dual-objective reservation, not carryover itself or the persistent partial-layout lifecycle. The next strategy must preserve promising topology without permanently taxing every generation's scarce beam slots.

An out-of-beam topology archive then implemented that open variable, after first removing a portability defect that had silently disabled the whole experiment off the canonical machine: the frozen `b9335a72...` parent is a fingerprint of the platform-bound boundary-projection trajectory, and on `x86_64-unknown-linux-gnu` the same request converges to an independently valid `181.938 mm` endpoint, so every nonzero mode skipped before any experimental work. A committed pinned-parent fixture extracted verbatim from the checkpoint's own `mode-zero-new.json` now supplies the frozen placements behind one optional benchmark argument; the compiled-in frozen fingerprint, depth, and dual-validation checks remain the acceptance authority, mode-zero output stays byte-identical under the timing/provenance projection, and with the pinned parent mode `3` on x86_64 Linux reproduces the canonical M4 endpoint facts exactly, including the best-ever `c2329244...` 11-piece / `59571041296` state and final `d12e7ee3...` 12-piece / `60144097737` state (per-layer M4 trajectory artifacts are not retained, so the claim is endpoint equality, not full-trajectory equality). The archive screen itself stores full clones of the best-ever area and count elites outside the beam, detects deterministic stagnation (three non-improving layers, cooldown three, at most thirteen revivals), and revives one archived elite either as one extra funded parent (mode `7`, quota formulas revised together with their tests) or by swapping the comparator-worst entering slot under a strict-improvement guard (mode `8`, zero added work). Both treatments were signature-identical to the contemporaneous mode `3` through layer 17 (entering-population hash, ordinary-child hash, and per-layer work; treatment layer rows additionally carry archive diagnostics) and diverged exactly at the first revival at layer 18; no layer shared an entering population while differing in ordinary children. Late revivals at layers 35 and 38 rescued the 11-piece basin that the control loses near layer 36: both treatments ended with a terminal comparator-best of 11 inactive pieces / `59571224242` versus the control's 12 / `60144097737`, passing the predeclared partial gate with unregressed best-ever objectives, while mode `7` cost `+8` funded selected slots (about `+1.3%` funded work, engine medians statistically indistinguishable from control on this machine) and mode `8` used the same funded slot and finalist quotas as control, with realized downstream work naturally differing once trajectories diverge. Neither arm completed 61/61, and best-ever objectives did not improve, so this is preservation, not discovery: the archive keeps the elite basin alive but successor generation from revived states still uses the insertion/ejection operator that plateaus at 11 inactive pieces. Evidence, raw outputs, and gates are pinned in `docs/experiments/persistent-vacancy-archive-evidence.json`. The next causal variable is a materially different transition from archived states — bounded multi-piece remove/reinsert around the surviving inactive pieces' blocker neighborhoods, or an active-piece relocation operator inside the partial lifecycle — with modes `7`/`8` as the control.

A virgin-basin descent then measured how the accumulated machinery behaves away from the M4-projected parent: seeded from the native pipeline's seed-11 endpoint (`177.205 mm` independent), one settle-descent hop reached `177.011 mm` and one mode-17 walk reached `176.591 mm` — a `0.614 mm` gain in two productive hops, several times what the same machinery extracts from the already-projected M4 basin — before the basin converged and twelve salted restarts changed nothing. The generalization is confirmed and the arithmetic is now complete on both sides: the machinery reliably extracts the residual slack of whatever basin it is given, basin quality dominates the outcome, and on this platform no native basin starts anywhere near the M4-projected `168.4 mm` family. The program's two structural exits remain a contract-native upstream calibration to fix the true target band, and a fundamentally stronger constructor or multi-hour population search to supply better virgin basins for this descent machinery to finish. Evidence: `docs/experiments/persistent-vacancy-descent/virgin-basin/`.

A band-ruin screen (mode `19`) then tested the joint co-placement hypothesis the certificate had isolated: the K deepest pieces are removed as a set regardless of adjacency — the configuration no spatial-neighborhood ruin ever produced — and reinserted through the optimizer-equipped lifecycle, whose 181 full-key improvements include cross-well swaps. The frontier still returns to grid `166031`. The measured explanation completes the picture: the frontier band's pieces occupy separate wells walled by survivor pieces; each well readmits its own piece at its old depth, wells do not communicate, and improving jointly would require moving the survivor walls — already excluded up to ruins of 56 pieces. The settled `168.275 mm` incumbent is now evidenced as a deep local optimum of the entire ruin-recreate paradigm at this reinsertion quality, from single-piece moves through whole-band joint repacks. The untested frontier that remains is of a different kind: applying the full accumulated machinery — diagonal settling, overlap-mediated separation with relocation escapes, the endpoint optimizer, vacancy-transport acceptance, salted multi-start — to virgin basins rather than the M4-projected parent, for example descent chains seeded from the native pipeline's own constructor states, whose slack this machinery has never touched.

A frontier-band feasibility certificate (mode `18`) then converted the residual question into a measured fact. For each of the five deepest pieces of the settled incumbent, every conflict-ruin orientation was swept across an 8 mm translation lattice over the sub-frontier strip — roughly 163,000 poses — with hazard screening and exact confirmation: zero exact-valid sub-frontier poses exist for any frontier-band piece. No single-piece re-placement anywhere on the sheet at any rotation can lower this frontier. The certificate explains every saturation of modes 14 through 17 simultaneously and reduces the remaining mechanism space to exactly one: simultaneous joint co-placement of the frontier-band pieces into each other's vacated space — a five-piece exact packing subproblem that sequential greedy reinsertion structurally cannot express, and the concrete specification for the next engine build. Evidence: `docs/experiments/persistent-vacancy-descent/frontier-certificate/`.

Extending the ruin schedule to nearly total repacks (neighborhoods of 28 through 56 of the 61 pieces in the third round block) completed the neighborhood-size axis: every large-K round still reaches a feasible zero-overlap endpoint with zero reinsertion failures in 3.6 seconds — the lifecycle can rebuild two thirds of the layout exactly and deterministically — yet acceptance quality is unchanged (three accepted, one wandered, frontier immobile). The convergent statement across modes 15 through 17 is now singular: feasibility, diversity, walk length, cycle breaking, multi-start, and neighborhood size are all solved or saturated, and the sole remaining constraint is that greedy hint- or depth-guided reinsertion cannot out-pack the incumbent at any K. The endpoint generator must itself be an optimizer: separator-guided endpoint generation — the surrogate separator's guided global sampling driving where lifted pieces land — inside the elitism-protected accepted-round framework, with the mode-17 walk as its control.

A vacancy-transport acceptance evaluation (mode `17`) then tested the community suggestion of routing free-space pockets instead of pieces, and split cleanly into a confirmed mechanism and a negative outcome. A trapped-void raster (2 mm cells, flood-filled from the above-frontier band) became the middle acceptance key, a post-endpoint settle sweep converted drained voids into measurable key progress — without it no endpoint ever entered any wander tolerance, with it wander engaged — and walk tabu plus target-salted deterministic multi-start eliminated intra-walk and cross-hop limit cycles. The outcome remained negative at the evaluated scales: after three productive hops, roughly 480 diverse feasible non-monotone rearrangements across twenty salted restarts were all rejected by the (frontier, trapped-voids, depth-sum) acceptance, every restart output equalling its parent. The settled incumbent's frontier basin is profoundly stable under neighborhood ruins up to K=24. Endpoint diversity is no longer the binding constraint; endpoint quality under any static acceptance key is. The remaining levers inside the elitism-protected framework are a population of parents over salted-walk endpoints held in the existing archive, larger funded ruin neighborhoods, and separator-guided endpoint generation. Evidence: `docs/experiments/persistent-vacancy-descent/vacancy-transport-evidence.json`.

The mode-`16` operator sweep then completed with a qualitative mechanical milestone and a precise remaining question. Adding a global relocation escape — the stuck piece is deactivated and reinserted anywhere through the depth-ranked, hazard-screened generator, strictly decreasing total raw overlap — made every separation round reach a zero-overlap exact-valid endpoint (eight of eight, from one of eight), with rotational probes, bilateral recruiting, coordinated pair moves, and guided pair weights all previously saturating on multi-body locks. Record-to-record wander with best-ever elitism was added on top, but relocated pieces land well above the frontier, so no endpoint enters even a two-millimetre wander tolerance and chained hops still stall at `168.275 mm`. The bottleneck has therefore moved entirely from feasibility to endpoint quality: the lifecycle can now generate unlimited feasible non-monotone rearrangements, and what it lacks is a quality signal and walk length able to descend through them — longer wander schedules with population diversity over round endpoints, or the surrogate separator's guided global sampling adapted as the endpoint generator, both hosted in the same accepted-round framework with elitism guaranteeing the published record never regresses.

A non-monotone LNS screen (modes `15` and `16`) then attacked the routing gap directly. Mode `15` — lift the frontier piece with an adaptive nearest neighborhood of up to 24 pieces, resettle the survivors diagonally into the vacancy, reinsert greedily lowest-fit with hazard screening at full-sheet working settings, accept on a lexicographic (frontier, total-depth-sum) key with snapshot revert — was rejected outright: zero accepted rounds at every neighborhood size, closing greedy-exact multi-piece reinsertion on the settled incumbent. Mode `16` replaced greedy reinsertion with overlap-mediated separation: removed pieces return at their old poses overlapping, and a bounded deterministic descent of the grid-quantized total exact overlap moves the worst soft piece along a compass ladder, recruiting the worst-overlap anchor when stuck and competing for acceptance only from zero-overlap endpoints. This produced the first accepted non-monotone rounds in the program — three consecutive productive chained hops (`168.277` to `168.275 mm`, plateau moves on the depth-sum key) — before the translation-only separation operator saturated with the frontier unmoved: seven of eight rounds stay stuck even with bilateral recruiting. The causal reading is sharp on both sides: overlap mediation is the correct family, the only operator class to move the settled incumbent at all, and compass translation alone is insufficient to resolve squeezed configurations. The lifecycle now needs rotational separation probes and coordinated simultaneous pair moves inside the same accepted-round framework. Evidence and the chained `168.275 mm` state are pinned in `docs/experiments/persistent-vacancy-descent/lns-evidence.json`.

A monotone exact-compaction screen (mode `14`) then closed the last translation-only family with a two-sided causal result. Guillotine group drops — translating every piece above a horizontal cut as one rigid body, so intra-group pairs need no re-checking — were completely inert on every probed parent: a settled layout rests transitively on the floor, so every horizontal cut crosses at least one exact contact and no cut group can descend. Diagonal strict-descent settling then proved the sharper fact: at the `168.277 mm` chain fixpoint it accepted 83 interior moves — distributed diagonal slack demonstrably exists — while the frontier moved `0.001 mm` and no micro-contraction completed; from the unconverged root the same operator compacts marginally deeper than vertical settling (`168.289` versus `168.291 mm`). Monotone exact motion in any direction therefore cannot route interior slack to a frontier piece whose descent cone is blocked; the routing requires non-monotone coordinated moves in which pieces temporarily rise or exchange pockets. This is now the sharpest statement of the remaining architectural gap: the next engine lifecycle must couple exact-valid partial states with temporarily infeasible motion — the separator operating inside partial layouts, or bounded multi-piece remove/reinsert with lateral freedom — using the deterministic chain and modes `11`/`14` as controls. Evidence is pinned in `docs/experiments/persistent-vacancy-descent/monotone-compaction-evidence.json`.

A guided-reconstruction screen (mode `13`) then tested whether the committed Sparrow calibration layout could be re-seeded into the engine's exact contract, and closed with a mechanism-level negative that recalibrates the program's quality target. The Sparrow `154.44858 mm` layout is packed at `5.0 mm` separation across the full sheet width, while this engine's publication validators enforce `totalPaddingMm + 2 * sag = 5.5 mm` pair separation and `5.25 mm` boundary clearance; re-separating the same topology needs `+0.5 mm` per gap that width-saturated piece runs cannot supply, so guided reconstruction with displacement probes, shelf fallbacks, and a completed deferred second pass places 39 of 61 pieces from the unstretched hint field and 42 of 61 from a 12% depth-stretched field, with every deferred piece failing its retry. The demonstrated `154-155 mm` band therefore compares a weaker separation contract against this engine's stricter one and overstates the reachable gap by an unmeasured amount; a contract-native upstream calibration at `5.5 mm` separation is required before that band can gate promotion decisions. A fifteen-seed portfolio of the unchanged native pipeline on `x86_64-unknown-linux-gnu` was also negative: independent depths span `177.205-184.476 mm`, all far above the frozen-parent chain, confirming that the `168.361 mm` M4 endpoint is a platform-specific basin rather than typical pipeline output and that seed diversity alone cannot substitute for the missing topology-scale operator.

A descending-target contraction lane (modes `9`-`12`, documented in the design file) then tested those operators directly against the frozen `168.361 mm` parent and produced the first strict complete improvement below that endpoint. The population operator alone (mode `9`) cannot close even a `0.005 mm` contraction: the boundary-projected parent is a contact-locked wedge in which two remaining offenders churn without homes, and at `0.125-0.5 mm` contractions ten to eighteen offenders plateau at three to nine inactive pieces. Odd-layer blocker relocation (mode `10`) does not break the deadlock. A translation-only exact settling prelude (mode `11`) does: three bottom-up sweeps with a `0.512-0.001 mm` step ladder and exact pair/containment gates on every probe redistribute the distributed micro-slack downward (66 accepted drops on the first settling prelude), and when settling pulls all 61 pieces inside the target the settled complete state passes both unchanged publication gates directly. The chained descent driver completed targets `168.550`, `168.544`, `168.538`, and `168.532` and reached an independently validated complete depth of `168.277 mm` — `0.084 mm` below the protected `168.361 mm` endpoint — before stalling at target `168.526`: settle slack exhausts and the population again plateaus at one to two inactive pieces. A bottom-left lateral settle variant was tried and rejected (it destroyed the vertical channels offenders need), and settle-plus-relocation (mode `12`) does not break the stall either. Two full chain replays were byte-identical per hop after timing normalization apart from the recorded fixture path strings. This establishes three causal facts: the remaining gap at this parent is not preservation, scheduling, or single-piece relocation but coordinated multi-piece rearrangement; exact micro-settling is a cheap, sound, and reusable primitive that composes with the partial lifecycle; and the `154-155 mm` band cannot be approached by micro-moves on this topology — the next experiment must change piece-level topology at scale, for example bounded multi-piece remove/reinsert with lateral freedom around the wedge's contact graph, or a coupled exact-valid/infeasible population with the separator lifecycle operating inside partial states.

Active moves begin with surrogate-colliding and boundary-violating pieces, but are not restricted to them. Exact-gate failures return structured offending piece, pair, or boundary IDs and feed those IDs into lane-local penalties. Bounded escalation expands to their conflict neighbors and high-pressure non-colliding blockers, followed by deterministic ruin/reinsert or large-piece swaps when local moves stall. Validation errors exposed publicly remain stable strings; the internal validation result carries structured violation data.

Randomness is an explicit request seed. Exact replay is promised only for a deterministic work quota, deterministic worker partitioning and merge epochs, the same request, seed, engine version, worker count, target triple, Rust toolchain, and libm implementation. Arbitrary-angle trigonometry is not promised byte-identical across a different numeric platform; canonical CI and golden promotion record this identity. Wall-clock mode is valid and anytime but is not replay-identical because cutoff timing and worker arrival order are observable. Feasible incumbents are merged only at deterministic epochs in replay mode.

### 5. Output and compatibility

The existing public profiles continue to route to the legacy engine until the general engine passes its own contract. The experimental profile is opt-in and carries a new geometry/search algorithm identity in cache keys and diagnostics.

The result schema remains backward compatible. Additive diagnostics may report:

- source and offset topology counts;
- concave and holed part counts;
- sampled and refined rotations;
- feasible and infeasible iterations;
- incumbent improvement history;
- collision-query broad-phase and exact-phase counts;
- peak resident memory when the delivery boundary can measure it.

## Delivery milestones

### M0: controlled evidence

- Record legacy results for the canonical 18 rows and the 74-piece fixture.
- Add a reproducible comparator using identical geometry, sheet, clearance, rotation, seed, hardware, and time budgets.
- Score placed count, used strip length, occupied-envelope density, runtime distribution, and peak memory independently from engine diagnostics.
- Pin upstream Sparrow by source revision, compiler, release features, request hash, contour adapter, spacing and sheet-inset semantics, rotation and mirror permissions, seed, and worker count. Report conversion/preprocessing separately, compare solver-only 3.2-, 6-, and 10-second windows on the same machine, and independently validate placed count, clearance, and reconstructed depth; do not equate its wall-clock result with this engine's deterministic epoch quota.

### M1: topology-preserving geometry

- Add multi-region `PolygonSet`, deterministic normalization, transforms, bounds, area, and point classification.
- Preserve concave contours through import and collision preparation.
- Add a topology-preserving Clipper adapter with canonical region/hole ordering and defined behavior for vanished holes, split results, and empty offsets.
- Add contractual-grid concave intersection, clearance, and sheet-containment tests plus an independently implemented publication validator.
- Add fixtures for L/T shapes, stars, narrow concavities, touching edges, internal hole topology, snapping collapse, and invalid rings.
- Add a deterministic internal constructor and a protected fixture where a valid placement occupies a concavity and beats the convex-hull baseline.
- Keep every legacy golden row unchanged.

### M2: complete general optimizer

- Add the opt-in general profile and real-contour contact candidates.
- Add a cheap overlap proxy, global/focused transformation sampling, adaptive translation/rotation coordinate descent, and guided pair penalties.
- Add bounded remove-and-reinsert and infeasible-state disruption for pieces that produce poor frontier growth.
- Produce valid complete layouts for Shapes-17, Triangle-20, Mixed-61, and the concave fixtures.
- Demonstrate a substantial quality improvement on Mixed-61 and at least one concave case with no regression in placed count or validity anywhere.

### M3: deterministic parallel anytime search

- Add deterministic multi-worker replay and wall-clock anytime modes.
- Compare fixed 1 s, 10 s, and 30 s budgets against the fast constructor, legacy engine, and upstream Sparrow where input semantics permit.

### M4: hole ingress and consumer migration

- Add a versioned source-contour protocol for multiple outer rings and holes, with compatibility conversion for current single-ring requests.
- Stabilize protocol/schema diagnostics and N-API/CLI capability discovery.
- Shadow-run in Configurator before selecting the new profile by default.
- Preserve the legacy profile as an explicit rollback until production evidence covers representative workloads.

## Quality gates

- No published layout may overlap, violate clearance, or cross the sheet boundary under both the contractual-grid search check and the independent publication validator.
- Legacy profiles retain byte-exact canonical outputs.
- New-profile goldens are promoted only when placed count and validity do not regress; quality regressions require explicit bounded approval.
- Every benchmark stores the request fingerprint, engine commit, profile, seed, worker count, budget mode, hardware, compiler flags, runtime samples, and independent geometry score.
- Concave fixtures verify end to end that the constructor discovers at least one placement that occupies a concavity the convex hull would forbid.
- Performance work reports both elapsed time and peak memory.
- General-profile input limits and the maximum measured duration of one non-interruptible geometry operation are regression-tested before enabling the profile in production. The gate runs an adversarial corpus at every maximum-size boundary: dense crossings, combs, spirals, nearly collinear edges, alternating windings, deep concavities, many holes, and offset-induced splits. The 250 ms threshold is a measured target only. All untrusted general-profile geometry uses a killable worker process with a hard deadline until the underlying operations support cooperative cancellation; a widened timeout is never the fallback.

## First implementation slice

The first pull request should deliver M0 and a vertically complete internal M1 slice: topology-preserving polygon sets, contractual-grid legality, a separately implemented validator, fixtures, a deterministic test constructor that demonstrably exploits one concavity, and a benchmark boundary. It must keep public profiles on the byte-stable legacy engine and must not claim public hole support before the contour protocol exists.

### Reproducible M1 evidence command

Run the internal constructor and its convex-hull ablation on the protected L/T/star/narrow-cavity/hole fixture with a deterministic work quota:

```sh
cargo run --release -p polygon-nesting-core --example general_fast_benchmark -- \
  tests/fixtures/general-concave/constructor-v1.json 10 0
```

The JSON report includes the request hash, engine commit, internal profile, deterministic seed state, worker count, budget mode, hardware and compiler identity, instance descriptor, topology counts, exact-evaluation quota, every placement, independent placed-material score, used strip depth, strip utilization, and min/median/interquartile/max elapsed time. Repeated results must be identical. The CI-protected fixture fails unless all pieces are placed, the best-known depth is preserved or improved, and the topology-preserving constructor beats the same constructor run over convex-hull geometry. Peak resident memory remains an external process measurement (for example `/usr/bin/time -l` on macOS) until the delivery boundary owns a portable sampler.

The long-horizon basin harvest closed the remaining budget question on the
native portfolio. Five further pinned-parent descent chains (mode-11 settle
prelude, then salted mode-17 vacancy-transport walks, eight-stall
termination) ran against the strongest committed native basins: seed13
178.373 to 177.937 (yield 0.436 mm), seed2 180.207 to 179.756 (0.451 mm),
seed15 180.605 to 180.306 (0.299 mm), seed3 180.640 to 180.291 (0.349 mm),
seed12 182.095 to 181.733 (0.362 mm). With seed11 (177.205 to 176.591,
0.614 mm) that is six independent basins whose machinery yield is bounded in
0.299-0.614 mm and does not scale with additional salted budget; no native
basin reaches 176.5 mm (evidence and every chain artifact under
docs/experiments/persistent-vacancy-descent/basin-harvest/). The harvest
hypothesis is closed: the program's next increment must be a structurally
different constructor, and that constructor is now in design.

The program now carries a defensible depth floor. A certified area bound
(docs/experiments/depth-lower-bound/) inflates every piece by half the
5.5 mm pair separation (exact Steiner areas for convex pieces, certified
0.02 mm grid lower bounds for the nine non-convex stars) and divides by the
1995 mm usable width: no exact-valid Mixed-61 layout under this engine's
contract can be shallower than 131.978 mm (127.228 mm without the
depth-metric strengthening). The same construction at Sparrow's 5.0 mm
contract gives 124.887 mm, so contract overhead alone explains about
7.09 mm of the 13.83 mm gap between the engine record (168.277 mm) and the
Sparrow 3-second calibration (154.449 mm) at the bound level - the
geometry-side confirmation of the mode-13 recalibration finding. The
155 mm session target is not excluded by area: the residual gap is packing
structure, which is exactly what the mode-20 constructor now attacks.

The second structural exit is now built and measured end to end. Mode 20 -
the skyline beam constructor - produces complete exact-valid dual-gate
Mixed-61 layouts from an empty sheet in twelve seconds (four seeded order
restarts, width-aware lowest-fitting skyline windows, valley-local and
global escape ladders, per-parent beam diversity, trapped-void child
scoring), deterministically under the runs=2 replay gate, publishing at
271.716 mm. Fed to the accumulated descent machinery the constructed basin
yielded 59.982 mm in five accepted hops - two orders of magnitude beyond
the 0.299-0.614 mm pipeline-basin yields - converging at 211.734 mm
(evidence under docs/experiments/persistent-vacancy-descent/
constructed-basin/). The measurement isolates the remaining loss precisely:
bounding-box-level skyline placement forfeits the non-convex star
interlocking that the pipeline separator finds natively, leaving the
constructed family about 35 mm above the 177.081 mm native floor. The next
lever is therefore sub-bbox interlock at insertion time - negative contact
rungs and polygon-profile stations that let exact confirmation, not the
bbox overestimate, decide how deep a piece may nest.

Four constructor variants were then measured against the 271.716 mm v1
baseline on the identical protocol (target 320, pinned b9335a72 anchor).
Negative interlock rungs below the box top (278.312 mm) displace productive
confirmation rows with poses the exact gate almost always rejects. Free
lookahead piece selection (315.407 mm) is a pathology: under a
frontier-first key the beam defers every large piece into the saturated
endgame, where three of four restarts fail even the 320 mm audit.
Fallback-gated lookahead (276.857 mm) still loses: deferral without a
debt term degrades the layer structure even when limited to shelf-only
ranks. Only capacity scaling helped: a six-slot beam reaches 270.288 mm
(-1.428 mm) and is retained. The order-side hypothesis family is closed;
the binding constraint is pose quality - the bounding-box skyline never
proposes interlock poses, and only a polygon-profile skyline or NFP-grade
contact generation at insertion can. That is the constructor's next
structural increment.

The constructed-basin endpoint is operator-locked. A remodel chain rotating
band ruin, vacancy-transport walks, and settle (modes 19/17/11) against the
211.734 mm state accepted nothing across twelve salted attempts: the
layered basin the v1 constructor produces is terminal for the entire
current operator family, exactly as the 168.x basin was. Meanwhile the
six-slot beam basin (270.288 mm) descends past the four-slot endpoint
(202.204 mm and falling at hop six) - constructor quality propagates
through the descent, which is the causal signature the program needs: the
way down runs through better construction, not more post-hoc budget.

The insertion-settle family is compounding. Generalizing the vertical drop
into a full bottom-left push (drop to contact, slide left to contact, drop
again - every attempt an exact charged row starting from an already-valid
pose) moved the raw constructor from 229.121 to 203.208 mm, and its basin
descends to 191.572 mm - the constructed line has closed the gap to the
pipeline-native floor from 35 mm to under 15 in four measured increments
(211.734, 202.204, 201.626, 191.572). The variant discipline that got it
there is now explicit in the evidence: an insertion-time push pays exactly
when it starts from a confirmed-valid pose (drop-settle, bottom-left), and
costs exactly when it spends ranked rows on speculative poses (negative
rungs, extra stations, deep rungs, lookahead in every form, left-compact
key). The remaining loss to the 168.x family is now split between residual
constructor quality and the descent family's known operator locks.

The eight-salt screen on the bottom-left constructor calibrates its
variance: salts one through seven land in a tight 215.3-218.2 mm band
while the base salt's 203.208 is a five-sigma outlier trajectory - the
committed baseline is lucky, not typical, and single-trajectory variant
judgments below ~7 mm are statistically void. The rotation-nudge variant
(218.011) is rejected on exactly those grounds: within the typical band,
no demonstrated value, and it spends confirmation rows. Two consequences
are now explicit: cheap salt exploration is the correct way to harvest
constructor luck, and any future variant must either beat the 215-218
typical band systematically or beat 203.208 on the base salt to count.

Three closures land together. The 191.572 mm record chain replayed
end-to-end bit-exactly in an independent portfolio run (203.208
construction and every descent hop identical). A typical-band basin
(215.300, salt 3) descends to 202.502 mm, so seed luck is worth about
11 mm at the endpoint and the descent yield is stable at 11-13 mm per
constructed basin. And the 191.572 endpoint is operator-locked (band ruin,
vacancy transport, and settle all accept nothing across twelve salted
attempts), placing it in the same terminal class as every converged basin
this program has produced. The polygon-profile skyline (committed) then
shifts the typical constructor band from 215.8 to 214.3 median with best
typical 211.297: the luck-harvest now runs on the strictly better family.

The sixteen-salt harvest on the profile constructor decouples two
quantities the program had been conflating: constructor depth and endpoint
quality. The deepest basin (salt 11, 208.782) descends only to 201.260
(yield 7.5), while a mid-band basin (salt 1, 211.297) descends to 195.260
(yield 16.0) - the second-best endpoint the program has produced, behind
the standing 191.572. Basin structure, not basin depth, determines what
the descent machinery can extract, so the harvest protocol now descends
across the band instead of cherry-picking the deepest screens.

External observational evidence (a frame-by-frame recording of a reference
product's fast mode on a 61-part any-angle instance at 5 mm / 5 mm
clearances, 9.2 s reported optimize time) shows a phase-separated
lifecycle: a complete feasible candidate appears early, an exploration
phase replaces whole candidate layouts, then placement optimization,
strip compression, and bounded fine-tuning. The recording proves no
internals and no parity target, but two readings matter here. First, it
independently converges on the architecture this program reached by
measurement today - constructor portfolio producing complete candidates,
whole-incumbent selection, then compression and fine-tuning - and against
deep repair around a single trajectory, which our operator-lock and
basin-dominance results had already rejected. Second, the visible
clearances are the 5.0 mm contract again: every external reference point
now sits on the looser contract, and the ~7 mm bound-level overhead stands
between their numbers and ours. Alongside the recording, a read-only
articulation probe over three frozen partial states repeatedly identifies
one concave star as a free-space bridge whose omission reconnects about
5,499.68 mm^2 of sealed vacancy - the unlock mechanism for exactly the
sealed-void yield ceiling this program is measuring in the current batch.
The next increment is the matched-arm bridge-relocation operator (mode 21):
removal-set selection by maximal vacancy reconnection instead of frontier
depth, identical budgets and seeds against the mode-17 control, promotion
only on an independently validated complete improvement.

The band-wide batch descent sets a new constructed-line record and returns
an honest negative on the pre-registered predictor. Eight profile basins
descended: salt 3 (214.251, 5 sealed-void cells) reaches 190.737 mm with
the largest yield the machinery has ever produced (23.514 mm), beating the
191.572 record; salts 15/12/10/1 land 193.969/194.580/197.238/195.260 and
salts 7/4/11 trail at 201.3-202.7. The pre-registered prediction - sealed
voids suppress yield - is NOT confirmed: low-void basins average more
yield (18.0 vs 15.3 mm) but salt 12 (822 cells, 21.1 mm) and salt 7 (34
cells, 11.7 mm) break the pattern in both directions. Neither constructor
depth nor sealed-void count predicts endpoint quality; band-wide descent
remains the correct harvest protocol, and the record chain (all artifacts
under profile-harvest/salt3-record/) is the program's new reference at
190.737 mm.

The bridge operator's matched-arm verdict is negative and clean. Mode 21
attacked both standing records (191.572 and the new 190.737) through the
full salted ladder: the connectivity probe found and seeded a bridge piece
in all twenty-four rounds of every attempt (1,488 scans per run, the
structural cap exactly), and not one round was accepted on either state.
Selection is not the binding constraint - no removal choice rescues the
ruin-recreate family on a converged basin, which is now a four-way
replicated result (frontier, band, remodel rotation, bridge). The
articulation probe's reconnection signal is real but its legal-relocation
premise fails: sealed vacancy stays sealed because no exact-valid
relocation of the bridge piece exists within the operator budget. The
program's productive direction stays constructor-side: the salt-tail
harvest that produced 190.737 continues on wider screens.

The record falls again inside the same harvest. Completing the sixteen
first-harvest descents, salt 6 - a mediocre 216.852 mm constructor with
767 sealed-void cells - yields 27.489 mm, the largest extraction the
machinery has produced, and lands at 189.363 mm: the first endpoint below
190 and the new constructed-line record (artifacts under
profile-harvest/salt6-record/). This is the terminal exhibit for both
rejected predictors: the best endpoint of the program now comes from a
bottom-quartile constructor with the second-highest sealed-void count in
the band. Endpoint quality is a property of basin structure that no cheap
observable measured so far anticipates; wide sampling with band-wide
descent is not a fallback protocol but the correct one.

The thirty-two-basin table closes the sampling question. The second
sixteen-salt harvest descends to a best of 191.260 (salt 22) with the same
5-27 mm yield spread; best-of-32 equals best-of-16 at 189.363, so the
endpoint tail of this constructor family saturates at 189-191 and further
brute sampling is waste (full table under salt-harvest-2/). Per the
pre-declared fork, the program moves to the next structural increment:
checkpoint settling inside construction - running the exact settle ladder
on the beam leader mid-order, at thirty and forty-five placed pieces, so
every subsequent insertion builds on a compacted profile instead of
leaving all compaction to the post-hoc descent. A deep-budget continuation
of the record chain (salted stall budget tripled) runs alongside as the
zero-code control on the same question.

Two terminal verdicts close the day's second engineering round. Checkpoint
settling inside construction is rejected by matched endpoints - every
settled basin loses to its baseline twin after descent (209.756 vs
197.233, 203.167 vs 190.737, 190.925 vs 189.363) even though the raw
constructor improved: in-run compaction spends the very slack the descent
machinery converts better, the fourth distinct form of the same lesson
(the post-hoc machinery must be fed structure, not pre-chewed layouts).
And the deep-budget continuation of the record chain is dry - thirty
salted stalls without one acceptance from 189.363 - so the record stands
as the terminal endpoint of the current constructor-plus-descent
architecture. The measured next steps are pose-generation quality
(NFP-grade contacts) or a structurally different exploration paradigm;
both are engineering, and the ledger now carries every calibration a
successor needs (typical band 213-218, endpoint tail floor 189-191,
yield spread 5-27 unpredicted by any cheap observable).

The contact walk sets a new record and sharpens the architecture's central
tension. Its eight matched descents split four-four against the baseline
twins and the band median worsens slightly (198.3 vs 196.9): in-run
contact compaction does consume descent slack, exactly as the
checkpoint-settle rejection warned, and the deepest raw basin (195.645,
salt 2) yields almost nothing (0.385). But the metric the program actually
optimizes is best-of-portfolio, and there the walk wins outright: salt 0
descends 207.162 to 187.463 mm (yield 19.699), beating 189.363 (chain
artifacts under contact-walk/salt0-record/). The walk is retained on that
verdict. The tension is now explicit and measured: raw constructor depth
and descent yield trade against each other, and the portfolio tail - not
the band average - is where records come from. The harvest widens on the
walk family next.

Contact micro-rotation is measured and rejected in both sizes. Adding a
plus-minus two-degree (then five-degree) tilt with a short re-walk to
every settled finalist pulls all four probed salts into a tight
205.5-206.1 attractor: it stabilizes the constructor median slightly but
destroys the tail - the 195.645 salt-2 outlier disappears, and best-of-
portfolio regresses ten millimeters. Every record this program has
produced came from the tail of a high-variance family; rotations trade
tail for median, which is the wrong trade for record hunting. The
translation-only contact walk stands.

The deep-budget continuation converts once more before the family dries:
from 187.463 the tripled salted ladder extracts a further 2.704 mm and
terminates at 184.759 mm - the constructed line's standing record
(artifacts alongside the salt0 chain under contact-walk/salt0-record/).
The walk-family screens (salts 8-23) complete in the 202-209 band with no
tail candidate below the record path, and every background harvest has
now naturally terminated. The program is single-threaded on the fifth
family jump: exact contact-pose enumeration (NFP-grade candidates) inside
the existing rank-confirm-walk pipeline.

Contact-pose enumeration - the planned fifth family - is measured and
rejected in both of its bounded forms. Naive vertex-and-midpoint
enumeration floods the ranked rows with mostly-intersecting poses
(constructor collapses to 220-231); support-mapped sampling with outward
normals and a 0.01 mm backoff generates geometrically correct touches and
still loses (214.8-220.5 against the walk's 195.6-207.2). The mechanism is
now understood: enumeration is context-blind - it produces poses touching
ONE neighbor while ignoring every other constraint, and the landing key
cannot distinguish a well-seated pose from a deep overhang, so enumerated
contacts displace the station-anchored candidates that the walk then
finishes properly. The translation-only contact walk already reaches the
NFP boundary from inside the context, which is why it wins. Within this
insertion framework the walk stands as the measured optimum of candidate
generation; closing the remaining gap to the 177/168 references requires
either full NFP paired with a context-aware scoring (a multi-day build)
or a structurally different exploration paradigm.

Both descent-side budget axes are measured dry on the record state. A
wide-target ladder (best plus five millimetres, ten-fold salt steps,
thirty attempts) accepts nothing from 184.759; doubling the internal LNS
schedule to forty-eight rounds churns four to seven accepted interior
rounds per walk and still returns the identical endpoint on every salt,
so the doubled runtime buys nothing where it matters and the schedule
stays at twenty-four. The record basin is now exhausted across four
independent axes (driver salt budget, target width, interior rounds,
operator family). What remains within this architecture are multi-day
builds - full NFP with context-aware scoring - or a structurally new
exploration paradigm; every hour-scale hypothesis in the map has a
measured verdict attached.

Support-aware ranking closes the smarter-ranking family the same way its
siblings closed. Penalizing overhang candidates by their unsupported width
fraction lands 205.3-208.9 against the walk's 195.6-207.2, improves one
salt of four, and destroys the tail outlier again. Eight consecutive
same-family refinements have now been rejected by matched measurement
(negative rungs, both lookaheads, stations, deep rung, left-compact key,
rotation in two sizes, contact enumeration in two forms, support
ranking): the plain landing key over station-anchored candidates plus the
translation-only contact walk is measured as the local optimum of this
insertion architecture, and its variance is not noise to be engineered
away but the mechanism that produces records. Two structurally different
builds remain live, in causal-evidence order: warm-starting the legacy
continuous separator from a constructed basin (the separator co-optimizes
all pieces globally, which is exactly the capability sequential insertion
lacks, and an optional seed input preserves every protected default), and
full NFP paired with placement scoring - noting honestly that both of its
ingredients were individually rejected at insertion time, so its value
must come from the pairing or not at all.

The fifth family exists and it is the hybrid. Warm-starting the legacy
continuous separator from the constructed-line record improves on the
separator's own native control twin (180.708 vs 182.196 on identical
settings), the descent machinery then extracts what the separator cannot
(180.384), and one more round of each confirms a joint fixpoint - the two
engines have complementary locks, exactly as the architecture evidence
predicted. The alternation record stands at 180.384 mm from a 2.6-second
separator budget (artifacts and the native control twin under
constructed-basin/alternation/); the immediate scaling axes are the
separator's relaxed budget and seed, both untouched.

The hybrid's fixpoint distribution is tight and sub-180. Six alternations
from banked constructed basins land at 179.756 (twice, from different
starts and with structurally different layouts at the same grid depth),
180.280, 180.387, 180.496, and 182.465 - the alternation record moves to
179.756 mm, 2.7 mm above the native pipeline floor, and the attractor
band is barely a millimetre wide (portfolio artifacts under
alternation/portfolio/). Two distinct sub-180 layouts now seed the next
round: a separator-seed sweep from both.

The hybrid closes the day by certifying the deepest attractor and moving
the absolute record by its final two microns. The seeded sweep is
unanimous - eight alternations across two sub-180 layouts and four
separator seeds all hold 179.756, so the hybrid attractor is
seed-invariant. Against the imported M4 basin the separator alone holds
168.277, the descent arm re-extracts the known 168.275, and the separator
then holds that too: 168.275 mm now stands as the branch's absolute
record and as a certified joint fixpoint of both machines - the first
state this program has produced that neither engine can improve at any
tested budget, seed, or operator (artifacts under
alternation/m4-record/). The day's ledger is complete: five constructor
families built and measured, the hybrid as the fifth, every record
replayable, every negative carrying its matched verdict, and the honest
distances standing at 2.7 mm from the native floor for the from-scratch
line and 13.3 mm of measured contract-and-structure gap between the
absolute record and the recalibrated external reference band.

Correction on the scaling claim, for the record's integrity: the
double-budget separator runs did not measure anything - the frozen
coupled-experiment configuration guard silently skips arms whose settings
exceed the admitted probe (the arms return the incumbent unchanged, which
is exactly what the outputs showed). The 179.756 and 168.275 fixpoints
therefore stand certified at the canonical separator budget only;
separator-budget scaling remains contract-capped and untested, and
lifting that cap is an engine-side change to the frozen experiment
configuration, not a harness flag. The anchor-swap probe (constructing
with the 179.756 layout as orientation prior) lands in-band at
206.7-210.0 and is rejected without descents.

The contract recalibration is authorized and executed. By explicit product
decision (2026-08-16, session owner): a user-requested 5 mm pair spacing
and 5 mm border must not be inflated to 5.5 and 5.25 - the sag and safety
additions are dropped. Implementation is fixture-level and code-free: the
Mixed-61 pieces are pure segment loops (no arcs), so
flatteningSagToleranceMm and clearanceSafetyMarginMm collapse to the
0.001 mm validation minimum and the effective contract becomes
5.002 mm pair / 5.001 mm border - within two microns of the user's
intent, with every engine validator unchanged and the legacy 5.5-contract
request untouched alongside
(tests/fixtures/mixed-61/mixed61-request-exact-clearance.json,
sha 1bb90567...). The first native run under the exact contract lands at
175.112 mm against 182.196 on the old contract - the ~7 mm bound-level
overhead realized almost exactly - and the external references
(Sparrow 154.449 at 3 s, the recorded product run) are now directly
comparable for the first time. The deep 5.5-contract states are feasible
under the exact contract with half a millimetre of recoverable slack per
pair, and the alternation now runs from 168.275, 179.756, and the fresh
175.112 native.

First fixpoints on the exact contract. The imported-M4 line compresses
from 168.275 to 165.231 mm - the absolute record on the honest scale, and
the first number directly comparable to the external references
(Sparrow 3-second calibration 154.449). The from-scratch lines hold at
175.009 (legacy-native) and 179.009 (the old-contract constructed
endpoint recompressed): a layout structured for 5.5 mm contacts does not
re-pack by compression alone, so the from-scratch answer on the new
contract must be built inside it - the constructor's first exact-contract
band (200.6-208.8 over six salts) is descending now. The session owner's
acceptance criterion is recorded: the from-scratch line must reach the
168-family on its own for the program to count as complete; the imported
line serves as the measuring stick, not the destination.

First fixpoints on the true 5.0/5.0 contract, now that the engine honors
requested clearances exactly: the imported line lands at 166.832 mm, the
recompressed old endpoint at 179.006, a native single-run seed at
181.469. Trajectory chaos spans several millimetres across contract
micro-variants (the 5.002 track had compressed the same imported state to
165.231, and that layout is feasible under 5.0 too - its reseed runs
now), so state-seeded alternation remains the instrument and per-track
records are kept with their exact request hashes. The full from-scratch
pipeline on the true contract - eight constructions, band descents, then
alternation of the best three - runs alongside, and its fixpoint is the
number the session owner's acceptance criterion will be judged against.

The 165.231 reseed converges to 164.470 mm on the true 5.0/5.0 contract -
the new absolute record on the honest scale. The alternation chain from
the 5.002-track layout ran two productive cycles (separator 165.024 ->
descent 164.777 -> separator 164.598 -> descent 164.470) before the
joint fixpoint certified against both machines; the pinned parent
replays exact-valid at 164.470 with independent source-ring depth and
fingerprint ecabb250... under request sha ecfe126f.... Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-164.470/
(seed, full chain, pinned parent, revalidation replay). The gap to the
Sparrow 3-second calibration (154.449) is now 10.02 mm on an identical
clearance contract. The first from-scratch run of the same pipeline
stalled at the construction band: the band-descent driver still pointed
at the pre-contract-change isolated binary, which rejects a zero sag
tolerance, so every hop crashed and the eight constructions
(197.3-207.1) never descended - the resumed pipeline runs on the HEAD
binary now, and its fixpoint remains the acceptance-criterion number.

The from-scratch pipeline completes on the true contract: eight
constructions (197.3-207.1) descend to 185.9-202.5 (best yields 19.2 mm
on the 205.1 start), and the alternation hybrid takes the best three to
179.629 / 179.006 / 189.006 - the from-scratch fixpoint on the honest
contract is 179.006 mm. The striking finding: that number equals the
recompressed-old-endpoint fixpoint exactly, yet the two layouts share
0 of 61 placements - 179.006 is a structural depth plateau of the
instance, a common binding stack that disjoint basins hit
independently, not a coincidence of convergence. The from-scratch line
therefore stands 14.5 mm above the imported record (164.470) and the
acceptance criterion (168-family unaided) remains open; the binding
constraint is unchanged - constructor structure quality - and three
mechanism increments (pocket-refill coupled insertion, gap stations
under overhangs, mating orientation prior) are being implemented and
band-measured in parallel isolated worktrees. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/from-scratch-179.006/.

Salted alternation portfolios split the two lines. The imported record
is seed-locked: seeds 1-3 all return 164.470 unchanged - the fixpoint
is robust to chain salting, so advancing the absolute record now
requires new machinery, not more variance. The from-scratch plateau
BREAKS: seed 1 restructures 179.006 to 174.254 in a single separator
pass and the alternation refines to a 173.783 fixpoint (seed 2 stays
locked at 179.006 - tails again, exactly as the variance meta-lesson
predicts). The from-scratch line built entirely inside the honest
contract now stands at 173.783 mm, 5.5 mm from the 168-family
acceptance criterion; a second salted wave runs from the new state.
Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/from-scratch-173.783/.

The salted-wave cascade carries the from-scratch line to 172.861 mm and
then locks. Wave 2 from 173.783 improves on all four seeds (172.861 /
173.231 / 173.296 / 173.472 - an unusually productive state), wave 3
from 172.861 returns the incumbent on all four seeds: the from-scratch
line is now operator-locked at 172.861, 4.6 mm from the 168-family
criterion, mirroring the imported record's seed-lock at 164.470. Both
locked states now face the wide-rung experiment (descent targets
best+1.6 and best+3.2 instead of +0.8, admitting deeper non-monotonic
detours before reconvergence) while the three constructor mechanisms
build in parallel worktrees. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/from-scratch-172.861/.
