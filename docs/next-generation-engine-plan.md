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

The wide-rung experiment is a matched negative and the three
constructor mechanisms return their verdicts. Wide rungs (+1.6/+3.2)
leave both locked states untouched across six chains (164.470 x2,
172.861 x4): the locks are structural, not rung-width artifacts.
Constructor mechanisms, each implemented by an isolated-worktree agent
and measured band + descent-endpoint against the HEAD control:
gap-stations REJECTED (band worse on every salt, endpoints 1-3
against); mating-orientation-prior NEUTRAL and not adopted (band
slightly worse, endpoints 2-2 within noise) though its best endpoint
seeded useful basin diversity; pocket-refill NOT ADOPTED in its
current form but instructive - the coordinated two-piece insertion is
real (fires on 4/8 salts, up to 19 confirmed refills, honest
diagnostics counters) yet its filler-bearing beam children never
survive final selection, and the final constructions are verified
61/61 placement-identical to control on all 8 salts. The trapped
voids of this concave-heavy instance are simply too irregular for a
rigid bbox-anchored filler (median fill ratio 0.47). Meta-lesson
reconfirmed: band-level neutrality can still contribute (mating's
186.936 endpoint), and a mechanism can work exactly as designed and
still lose to the incumbent ranking. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/{wide-rung-negative,constructor-mechanisms}/.

The 179.006 plateau reveals itself as a five-fold structural attractor
and hands the from-scratch line its next break: 171.238 mm. The
16-salt widening (320.008-320.023) stays inside the known band and
its six alternations all funnel into the 179.0-180.6 shelf - but one
lands EXACTLY on 179.006, and both alternations from the mating
variant's 186.936 endpoint do the same, giving five independent
layouts (pairwise 0/61 shared placements) at the identical depth.
Exploiting the multiplicity: salted waves from the three new plateau
layouts (12 chains) produce exactly one escape - ex6alt2s1 + seed 3,
where the separator arm itself jumps 179.006 -> 171.568 in a single
warm-started pass (the first separator-driven plateau break; all
prior escapes came from salted descent targets) and the descent arm
settles a 171.238 fixpoint, independently replay-validated
(exactValid, 171.238). Full from-scratch lineage: salt 320.011 ->
202.275 -> descent 190.010 -> alternation 179.006 -> wave 171.238.
The from-scratch line is now 3.0 mm from the 168-family acceptance
criterion; the cascade continues from 171.238. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/{widening-ex6,plateau-179.006,from-scratch-171.238}/.

Wave 2 of the new cascade improves on all four seeds and the
from-scratch line breaks below 170: 169.731 mm. From the 171.238
state, seeds 0/1/2/4 return 170.317 / 170.221 / 169.731 / 170.010 -
a second all-seeds-productive state, mirroring 173.783. The winning
chain is genuine two-arm alternation over two full cycles (separator
171.238 -> 170.119, descent -> 170.010, separator -> 169.831,
descent -> 169.731). The from-scratch line now stands 1.5 mm from the
168-family acceptance criterion, entirely inside the honest contract
and entirely from HEAD code; wave 3 runs from 169.731. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/from-scratch-169.731/.

The deep shelf mirrors the plateau: sibling multiplicity yields
169.658 and both new locks certify, while the record absorbs four
more seeds. Wave 3 seed-locks 169.731; wide rungs confirm
(+1.6/+3.2 both incumbent). The sibling move - waves from the three
non-winning states of the 171.238 wave - drops the 170.010 sibling
into a DIFFERENT basin at 169.658, which then certifies its own lock
(4 seeds + both wide rungs). Two near-degenerate certified basins
0.073 mm apart, plus four more layouts within 0.7 mm: the deep shelf
has the same multiplicity structure as the 179.006 attractor, one
level down. The imported record meanwhile survives seeds 4-7 (7-seed
lock + wide rungs): the separator-escape lottery that broke the
plateau does not fire in that basin. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/{from-scratch-169.731,from-scratch-169.658,record-164.470}/.

Layout recombination (crossover) joins the mechanism roster: seam
legalization costs 5.7-11.6 mm, but the best hybrid enters BELOW the
plateau and mints new deep-shelf basins. Cutting the strip at the
short-axis midpoint and taking the 169.658 layout's left half with
the 169.731 layout's right half, the warm-started separator resolves
the seam to an exact-valid 175.363 - beneath the 179.006 entry
plateau - and alternations from it land at 170.515 / 170.840, two
mixed-gene basins the constructor pipeline has never produced. As a
basin-diversity generator this is two benchmark runs versus a
16-salt campaign. Crossover has not yet beaten 169.658; refined cuts
and waves from the hybrid basins run next, alongside a third
16-salt widening. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/crossover-recombination/.

THE FROM-SCRATCH LINE REACHES THE ACCEPTANCE CRITERION: 167.849 mm,
below the 168-family, with no imported basin anywhere in the lineage.
The break belongs to crossover descendants. The certified pure locks
(169.658/169.731) could not move, but recombining them minted basins
whose waves escape lower, and two recombined lineages descended in
parallel: A (170.840 -> 168.824 -> 168.459, now locked x4) and B
(170.515 -> 169.087 -> 168.531 -> 167.849). Replay-validated
(exactValid, 167.849). Full lineage: salt 320.011 -> 202.275 ->
descent 190.010 -> alternation 179.006 -> wave 171.238 -> 169.731 ->
sibling 169.658 -> crossover 175.363 -> 170.840/170.515 -> waves ->
167.849. Meanwhile the forced-compression class closes as matched
negatives: the separator target is passive (165/167/169 all return
incumbent), uniform squeeze (0.5-4%) and binding-stack nudges (1-4
mm, 3-10 pieces) all relegalize to the ~179 shelf, and alternations
from shelf-relaxed states are absorbed by the 179.006 attractor. The
distinguishing variable is whether a perturbed state legalizes BELOW
the plateau with inherited structure - crossover's midpoint cut is
the only perturbation measured to do so. Cascade continues from
167.849; next references are 164.470 and the 155 goal. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/{from-scratch-167.849,compression-negatives}/.

THE ENGINE GOES GENERAL: the 61-piece gate falls, and the proven
drivers become engine modes. The transfer test exposed the honest
truth behind the owner's overfitting concern: every persistent-vacancy
mode (1-21) was source-gated to pieces.len()==61, so nothing built
this cycle ran on shapes-17 or triangle-20. The generalization commit
converts every 61-derived aggregate quota into
VacancyQuotas::for_piece_count(n) (formula-verified for piece counts
1..400), removes the gate, and proves Mixed-61 behavior byte-identical
to the frozen binary (construction 206.869 / fingerprint 8a7737...,
descent hop fingerprint match, full diagnostic blocks equal) while the
constructor now produces exact-valid layouts on shapes-17 (200.651,
17 pieces) and triangle-20 (70.716, 20 pieces) with formula-scaled
work counters. Modes 22 (in-process separator/descent alternation to
joint fixpoint) and 23 (two-parent recombination with a scale-free
fractional cut) land as general engine modes with unit tests and
determinism gates - the alternation and crossover mechanisms that
carried the from-scratch line to 167.849 are now engine capabilities,
not python scripts. Meanwhile the three-lens critical extraction of
Sol's 26-commit branch graded every mechanism by evidence quality:
adopt-after-matched-arm = off-beam best-ever expansion parent
(-10.75% surrogate, the branch's only clean matched-arm), displaced-
first repair queue (-9.6%, cleanest experiment, no analogue on our
line), depth-two grandchild admission (-9.76%); adopt-as-guards =
geometry-settings equality on replayed parents and fail-closed on
unrun arms; rejected with numbers = the vacancy bridges (0 legal
candidates in 96 proposals), and Sol's 168.361/165 figures are
contract-incomparable (collision expansion 2.752 vs our 2.5,
aarch64). Next mechanism, synthesized from our compression negatives
plus Sol's repair-order rule: bounded-depth reinsertion (eject the
pieces protruding beyond a depth bound and reinsert them
displaced-first through the construction insertion machinery -
reconstruction under a hard bound instead of overlap legalization).

Bounded-depth reinsertion (mode 24) is a clean negative that closes
the second compression class and maps the binding front. The
mechanism is correct - the control bound reproduces the 167.849
parent byte-for-byte - but at every bound below the incumbent
(167.8 down to 165.0, ejecting 1 to 13 pieces) the largest ejected
piece finds zero candidate poses in the vacated space: genuine
geometric congestion, not budget. Together with the
overlap-legalization negatives this establishes that a converged
deep layout's binding structure is a dense front (7 pieces beyond
167.0, 13 beyond 165.0), which is why local compression cannot work:
the slack needed to re-place one piece is distributed across the
whole front. The mode itself is general (runtime bound, any piece
count - verified on triangle-20) and stays in the engine as
reusable machinery. Replay-integrity guards from the collaborator
extraction also landed (settings-equality on replayed parents,
fail-closed on unrun arms). Next matched arm in flight: the off-beam
best-ever expansion parent (mode 25) against the mode-20 control at
band + descent-endpoint level.

The off-beam elite parent (mode 25) is matched-arm rejected on
published depth. The collaborator's only clean matched-arm win
(-10.75% surrogate area on incomplete partials) does not transfer:
on the Mixed-61 band the treatment's min is 0.680 worse and its
median 0.026 worse, the only two salts that move split -1.889/+3.902
inside the ~10 mm reseeding spread, and the descent endpoints tie on
bit-identical constructions. The mechanism fires in only 0.6-7.3% of
layers and its children rarely win retention slots - the rank-
synchronous beam with frontier banding loses depth-optimal partials
far less often than the collaborator's lifecycle loses its elites.
Mode 25 stays as an opt-in variant for reproducibility; mode 20
remains the default. Sequencing consequence: the grandchild-admission
variant (same budget slot) is deprioritized; next arms are the
0.002 mm search-envelope allowance removal (enlarges the legal set
without touching publication - a direct probe at the dense binding
fronts of the locked states) and the gradual overlap-tolerant
compression operator.

The search-envelope allowance probe closes with a two-sided finding:
the 0.002 mm is one-quarter load-bearing, and the released slack is
noise-scale. Below ~0.0005 mm the search envelope stops being a
superset of the exact publication contract and even the constructor's
own output is rejected fail-closed; at the measured floor both
certified locks re-settle to contract-comparable exact-valid states
exactly one slack-release deeper (167.846 / 164.465, identical
orientations, max translation 0.023 mm, re-locked under 5 matched
arms each). The allowance is now an explicit runtime parameter with
the default unchanged. The micro-states are pinned as the numerically
best published layouts, honestly labeled slack-release rather than
search progress. With the local, ejective and envelope classes all
measured out, the remaining open class is gradual overlap-tolerant
compression under a clamped sheet - the separator constrained so it
cannot relax depth-ward - which goes to implementation next.

Clamped-sheet ladder compression (persistent-vacancy mode 26) is that
operator, and it is the first mechanism in this family to move a
certified lock by search rather than by slack release. A rung hands
the ordinary mode-0 pipeline a `sheet_long_axis_mm` equal to the
rung's bound and a warm-start incumbent carried at one separator
contraction above it, so depth-ward relaxation stops being expensive
and becomes geometrically impossible: `collision_fits_sheet` is the
only place the long axis bounds anything, and it gates every
acceptance. Bounds walk from the parent's own depth to the requested
final bound in at most eight uniform rungs, floored at the
separator's own single-target contraction ratio so the ladder is
scale-free.

The measured obstacle is not the clamp - out-of-sheet warm starts are
tolerated everywhere on the path in - but that an arm's
`final_placements` only ever reflect exact-accepted states, so a rung
whose single contraction target fails reports its own input back and
the ladder provably never moves (measured: every rung `stateChanged
= false` on both locks and on triangle-20). The arm's terminal
minimum-loss state is now recorded additively alongside it, which is
pure bookkeeping and leaves every existing arm's control flow,
acceptance and depths untouched. Each rung then runs two warm starts,
the deepest exact-valid state and the compression frontier, and keeps
the best of both.

Result on the from-scratch front: 167.846 -> 166.968 exact-valid at
seed 1 (rungs 1 and 2 of the ladder to 166.0), re-settling under mode
22 to 166.855, which is a joint mode-22 / mode-26 fixpoint - a
0.991 mm improvement on a state five prior mechanisms could not move,
and one mode 22 does not reach from the same parent at either seed.
The record state 164.465 does not move: its compression frontier
reaches 160.499 under the clamp but never legalizes. That is the
shape of the residual everywhere the mechanism fails - the frontier
compresses freely (A: 165.977 at bound 166.0; B: 160.499 at bound
160.558; triangle-20: 70.512) and is rejected by one to five
clearance-violating pairs, never by depth. Gradual overlap-tolerant
compression is therefore live but bottlenecked on legalizing a
near-feasible dense front, not on reaching one.

The compress-repair loop cascades to a certified 165.368 fixpoint.
After mode 27's 165.484 publication, alternating mode-22 waves and
mode-26/27 ladders trims 165.446 -> 165.407 -> 165.368, where all
eight arms (four alternation seeds, four ladder configurations)
return the incumbent. The from-scratch line now stands 0.903 mm from
the imported-line record - 13.6 mm below the entry plateau every
from-scratch lineage must cross, with the last 2.5 mm earned by the
deterministic compress-repair machinery rather than basin lottery.
Conflict-targeted re-placement (mode 28) is in flight against the
record's compressed frontier.

Anchor-local re-insertion lands as a real primitive and recharacterizes
the ladder residue; fine rungs are a negative. The seeded cloud is the
first machinery to repair an interior pocket (record-parent nudge
controls at 0.2/0.5 mm repaired exact-valid, all prior modes
bit-identical), but the deep-ladder residues turn out to be
over-compression - median incident violation mass 4.2 mm, projections
demanding up to 42 mm of travel - far beyond any single-piece repair.
Compressing at the pace repair can absorb does not work either: fine-
rung cascade loops (0.019-0.044 mm steps) fixpoint immediately on
both lines (165.368 / 164.465 hold). The remaining levers are
mechanical, not conceptual: the rollback-tracker false-positive abort
still kills 40-75% of rungs before they produce anything repairable
(an ulp-tolerance scoped to the clamped arms is in flight), and the
residue class above single-piece reach needs joint multi-piece
re-placement.

THE RECORD MOVES: 164.058 mm, and the from-scratch line follows to
165.203. The rollback-tolerance measurement matrix tried short
ladders at an intermediate rung scale (-0.27/-0.97) that no prior
cascade configuration had used, and HEAD's compress-repair machinery
published 164.112 from the record parent and 165.357 from the
from-scratch parent. Alternation and the autonomous cascade loop
settled certified fixpoints at 164.058 (8/8 arms) and 165.203 (8/8
arms). The imported-line basin that had absorbed seven seeds, wide
rungs and recombination without yielding a micron moved 0.407 mm
under a mechanism stack - clamp, two repair tiers, anchor-local
seeding, scoped rollback tolerance - in which every component is
general engine code. Remaining gap to the 155 goal: 9.06 mm. The
binding constraint is unchanged and precisely known: deep-frontier
states carry multi-millimetre clearance deficits that single-piece
repair refuses; joint multi-piece re-placement is the open build.

Joint multi-piece re-placement (mode 29, the third repair tier)
closes that open build and is a clean, well-instrumented NEGATIVE on
the residue class it was built for. The tier ejects every piece of
every pair-bearing violation component rather than the vertex cover
tier two lifts, so both sides of each conflict come out at once; it
then searches over insertion *order* - every permutation up to four
pieces, rotations of the canonical order above that - with each piece
drawing its own vacated pose, the single-piece separating
projection's trajectory, the aimed displacement cloud, the other
ejected pieces' vacated poses, and the skyline stations; and when no
order succeeds it runs one round of pairwise pose-swap seeding, which
exchanges two pieces' vacated poses in the anchor so each one's whole
cloud re-centres on the other's pocket. That last move is the
coordinated one no translation and no single-piece neighbourhood can
express at any magnitude, which is what made it worth building.

It fires everywhere and repairs nothing. Over ten ladders on the two
certified locks (record 164.058 to 163.8/163.3/162.5, from-scratch
165.203 to 165.0/164.5), tier three was reached on 173 of 176 rung
arms - tier one published 3 times, tier two zero - and admitted 110
of those, spending 1297 plain insertion orders and 914 pose-swap
attempts for zero exact-valid states. Both locks return their
incumbent on every seed. The residue is measured precisely: violation
components are 2 pieces in 108 of 110 admissions and 3 in the other
two; incident violation mass runs median 2.450 mm and max 5.028 mm;
and the best any order reached was 1 of 2, 3 of 4 or 5 of 6 ejected
pieces re-placed. The failure is therefore not order-dependence and
not the missing exchange - it is that at these depths the *vacated
space of the whole component* is already smaller than the component,
so no permutation of it can be packed back in. Of the 63 refusals, 35
were a kept sub-layout whose boundary residue would not micro-legalize
and 27 an ejection set of 8 against the local-repair limit of 7,
which is the one mechanical lever left in this tier: the pass ejects
every pair-bearing component in one set, so four independent 2-piece
conflicts refuse on a cap that neither of them individually trips.

The stack now has three tiers and a precise verdict on each. Tier one
repairs boundary-class and rounding-scale residues; tier two repairs
interior pockets to about half a millimetre; tier three reaches the
multi-millimetre class and confirms it is over-compression rather
than mis-arrangement. Every measured lever inside the compress-repair
loop is now spent, and the 164.058 / 165.203 fixpoints are unmoved.

AN ADVERSARIAL REVIEW REFUTED TWO OF OUR NEGATIVES, AND ON ONE OF THEM
IT WAS RIGHT: THE RECORD MOVES TO 164.042. The compression-negative
drivers were an instrument failure. pv34 and pv35 wrote
`reportedDepthMm` = `independentDepthMm` = 200.0 into the perturbed
warm-start fixture; the harness installs that field as the incumbent's
`used_long_axis_depth_mm` and every separator contraction target
derives from it, so the separator was handed roughly 36 mm of headroom
that did not exist and dutifully relaxed into it. The "~179 mm shelf"
those runs reported is a measurement of the fake headroom, not of the
geometry. pv35 additionally ranked "the k deepest pieces" by
`translateLongAxis`, a post-rotation anchor offset that ranges -25.2 to
+175.7 on a layout 164.058 deep and has nothing to do with the depth
frontier; the engine's own ordering, `high_frontier_blockers`, is by
transformed source max-Y.

Redone at true depth, the answer splits. The corrected setup declares
the parent's own depth at the mode-26 rung seed convention and clamps
the sheet long axis at the parent depth through the harness's existing
override - which is exactly the mode-26 rung clamp, since
`collision_fits_sheet` is the only place the long axis bounds anything
and it gates every acceptance - so the separator provably cannot relax
depth-ward. Under that clamp the separator accepts *zero* contraction
targets on every perturbed state at both seeds, clamped and unclamped
alike: the honest version of the original negative is not "179", it is
"nothing". The uniform squeeze stays closed and for a sharper reason:
even a 0.5% affine squeeze shatters the layout into eight or nine
violation components whose largest spans 16 to 36 pieces, which is
above every local-repair limit by design. But the binding-stack nudge
negative does not survive. Nudge the *true* depth frontier by one to
two millimetres and hand the result to conflict-targeted re-placement
(mode 28) with its bound set to the incumbent's own depth, and the
tier legalizes it below the record: 164.054 exact-valid at the first
try, 18 exact-valid publications across a 35-point (k, d) sweep, and a
cascade of 164.058 -> 164.054 -> 164.053 -> 164.043 -> 164.042. The new
state is replay-validated (zero violating pairs, zero boundary pieces,
`parentIndependentDepthMm` 164.042) and is a certified fixpoint on 8/8
established arms - mode-22 alternation at four seeds and mode-26
ladders at two rung scales and two seeds - plus both perturb-repair
cascades. Gap to the 155 goal: 9.04 mm. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/{compression-negatives-corrected,record-164.042}/.

The second refutation is upheld mechanically and rejected
scientifically. Tier three did pool every pair-bearing component into
one ejection set, so four independent two-piece conflicts refused on a
cap none of them individually trips - 27 of the 63 refusals were that
artifact - and it did commit the first in-bound finalist per piece with
no backtracking. Both are fixed. The tier now repairs one connected
component at a time, re-surveying the whole layout between components,
and for components of at most three pieces it enumerates all 64
combinations of the four finalist ranks per piece rather than the
single greedy one. The plan order is deliberate: plain insertion
orders, then the pose-swap round, then the beam, so every state the
tier used to publish it still publishes by exactly the route it used
to and the change can only ADD publications. Acceptance is still the
authoritative validator whenever it passes; only while other clusters
are outstanding may a pass accept partial progress. No new aggregate
quota term: the whole plan is charged against exactly the slot product
the single-set pass was already funded for, with the ceiling asserted
in the same quota test. On matched arms the pooled-cap refusal class
goes to zero and states that previously could not even be attempted
now get five and six component passes with a component actually
repaired in each. And on the ladders the original negative was
measured on, the verdict is unchanged: 104 tier-three invocations, 102
admitted, 256 component passes, 509 orders, 254 swaps, 3810 beam
combinations, one component repaired, zero publications, both locks
returning their incumbent at every seed. The residue at those depths
is over-compression whose components individually have no in-bound
pose - which the same fixed tier proves by being productive one class
up, on the nudge-scale residue that just moved the record. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/joint-replacement-per-component/.

The methodological lesson is the expensive one. Both negatives were
reported as "matched" and neither was: one measured its own harness
argument and the other refused on a cap its own ejection rule created.
A perturb-relax experiment must state, and a reviewer must check, what
the incumbent depth field was set to and what actually bounded the
relaxation - and a refusal is only a negative when a certificate sits
behind it.

THE RECORD MOVES 4.892 MM, TO 159.150, AND THE THING THAT MOVED IT IS
THE ONE MOVE NO REPAIR TIER ON THIS BRANCH COULD MAKE: ASKING A PIECE
THAT VIOLATES NOTHING TO GET OUT OF THE WAY. The per-component beam's
negative was correct and its certificate was the clue. Tier three
proved that the pieces of a deep-frontier violation component
individually have no in-bound pose - 256 component passes, 509 orders,
254 swaps, 3810 beam combinations, one component repaired, zero
publications. Read as a statement about the *layout* rather than about
the tier, that says the room the component needs is not inside the
component. Every repair tier the branch had built was local by
construction: it moves, ejects or re-places the pieces that are
themselves in conflict and holds the rest of the layout still. None of
them can reach room three pieces away, at any magnitude, in any
ordering, under any pose combination. That is not a search failure. It
is a missing degree of freedom.

Global pressure-balanced legalization (mode 30 unbounded, mode 31 under
a depth bound, and the fourth mode-26 repair tier) supplies it. Every
piece of the layout gets a translation variable - all 61, not the two
or three in the component. Each pair inside a guard band contributes
one linearized separation row `n_ij . (t_i - t_j) >= target - dist_ij`,
with the normal taken from the exact closest-approach witness this
repository already measures; a violating pair's row has a positive
right-hand side and asks for correction, and a *legal* pair's row has a
negative one and protects the clearance it already has, which is what
makes it safe to let the whole layout move. Sheet containment is four
exact rows per piece per gate - exact rather than linearized, because
the outer bounds of a translated outline are the translated outer
bounds - and the depth bound is simply the top row of every piece under
a clamped sheet, a hard constraint of the program rather than a filter
applied afterwards. The envelope gate keeps the branch's hard-won
discipline: it is a boolean, so an overlapping pair's magnitude is
recovered by bisecting against `polygons_overlap_exact` itself, and a
pair that has ever overlapped keeps its row for the rest of the run.

The solver is Hildreth's method - projected Gauss-Seidel on the dual of
`min ||t||^2 subject to A t >= b`. One multiplier per row, swept in a
fixed order, each moved by its own residual scaled by the row's squared
norm and clipped at zero, with the primal iterate `t = sum lambda_k
a_k` carried incrementally. No external solver, no factorization, no
floating-point order that depends on anything but the layout. The
non-negative multipliers are the whole mechanism: they *price the
pressure* a blocked piece exerts on its neighbours, so when the piece
that must move is wedged, the chain of rows behind it carries the
correction outward until it reaches slack. The round then applies the
step under a trust radius sized from the residue's own scale, snaps to
the canonical grid, re-measures the true geometry, regenerates every
row from that measurement and repeats - a trust-region SQP whose linear
model is only ever used to pick a step, never propagated.

It works, and the measurements say why rather than merely that. Over
438 tier-four invocations this session it published 105 exact-valid
states, and the shape of a winner is the finding: it moves a **median
of 57 of the 61 pieces**, with a median worst displacement of 5.5 mm
and a median mean displacement of 1.4 mm. That is not a repair of the
violation component. It is a redistribution of the layout, and it is
precisely the class of move the three local tiers cannot express. The
matched arms are unambiguous - every record ladder was run twice, once
on the dda427c binary and once with the tier armed. The base arm
returns 164.042 at every bound and seed, because 164.042 was a
certified 8/8 fixpoint; the treated arm publishes 163.411 at bound
163.5 seed 0, 163.497 at seed 1, and 162.488 at bound 162.5. The very
first run with the tier armed was already 0.631 mm below the standing
absolute record, against a previous best increment of 0.016 mm.

Cascaded and then jointly fixpointed against mode-22 alternation, the
line runs 164.042 -> 162.488 -> 161.486 -> 160.509 -> 159.958 ->
159.317 -> 159.155 -> **159.150**, and stops there on 12/12 arms: eight
mode-26 ladders at four bounds and two seeds, with the tier armed on
every one, plus four mode-22 alternation seeds. Modes 27 and 30 replay
the pinned state to zero violating pairs, zero boundary pieces,
`exactValid` and `contractValid`, raw source depth 159.1499863776172.
The gap to the 155 goal closes from 9.042 mm to 4.150 mm, and the gap
to Sparrow's 154.449 on this same contract from 9.593 to 4.701.

Two limits are worth stating as plainly as the win. Standalone on a
certified fixpoint the program is *infeasible* and says so with a
certificate: mode 31 on the 164.042 record at bounds 163.8, 163.5 and
163.0 drives all eight to ten boundary violations to zero - the depth
bound is fully satisfied - and then stalls on two envelope pairs whose
dual residual sits at exactly their own requirement, because the layout
is already packed to the contract from the sheet's bottom edge to its
frontier and there is no translation-only redistribution that reaches
the bound. The tier is productive only where the *ladder* puts it, on a
compressed frontier state where the separator has already spent the
slack and left a repair problem behind. And the from-scratch line moves
only 0.836 mm, to a 164.096 fixpoint: its 164.4 ladder is a clean
tier-four negative and only the 164.0 rung breaks through. This is, so
far, a mechanism for deepening an already deep basin far better than
one for reaching it, which leaves the owner's from-scratch acceptance
criterion exactly where it was. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/{global-legalization,record-159.150,from-scratch-164.096}/.

The lesson generalizes past this mechanism. Three tiers of increasingly
clever *local* search all bottomed out on the same residue, and each
one's negative was clean, instrumented and correct. What was missing
was never a better local search; it was a variable per piece and a dual
variable per constraint. When a sequence of local mechanisms all fail
on the same class, the next question to ask is not "what is a smarter
local move" but "what degree of freedom does every one of these
mechanisms lack".

## The perturb-solver combo: two matched controls, two opposite verdicts

The last untried mechanical combination was the corrected nudge
(pv36's convention: the k deepest pieces by true transformed max-Y,
moved d mm into the packed body) feeding the mode-31 global solver
instead of the local ladder, cascaded with modes 26 and 22 and run with
a k=0 matched control on every arm. It was run on both live lines at
once, and the two controls returned opposite verdicts - which is
exactly the kind of causal fact this ledger exists to keep.

On the record line the perturbation is a placebo. The unperturbed
control publishes 159.101999 at the same target as every perturbed
cell; over a widened 320-arm sweep (k up to 16, d from 0.05 to 6.0 mm)
the nudge contributes exactly one 0.0077 mm cell. What actually moved
the record from 159.150 to 159.092 was the *step size*: the standing
"infeasibility certificate" on this certified fixpoint had been
measured at bounds 0.24-1.04 mm below the parent, and at steps of
0.01-0.04 mm the same program on the same fixpoint simply publishes,
three rungs in a row, before mode 22 shaves the last 0.3 micron. A
certificate is a statement about the question asked, not about the
state - re-ask with a smaller step before believing it.

On the from-scratch line the perturbation is load-bearing, and the
diagnostics say why: the solver's cumulative displacement cap is 8x the
largest deficit asked of it, so an unperturbed run at a 0.006 mm bound
starves under `displacementCapped=true` with six pairs left, while a
2 mm nudge manufactures 2 mm deficits and buys a 16 mm cap. The control
publishes nothing at any bound; ten of twelve perturbed cells publish,
and the line moves 164.096 -> 164.040 with the perturbed cell as the
largest single link. The same arithmetic explains the aggressive-end
deaths (k=6, d=3.5 hands the solver more residue than the bound leaves
room for) and pins the productive band at k=2-3, d=1-2 mm, eps in
{0.006, 0.012, 0.025} - narrow, non-monotone, and now measured.

Two structural facts came out with the data. Perturb -> mode 26 and
perturb -> mode 22 are impossible by construction - both validate the
parent on entry and reject the first overlapping pair, so 48/48 such
arms are non-experiments, not negatives; infeasible states enter only
through modes 27/28/29/30/31, and the only runnable cascade shape is
perturb -> 31 -> legal state -> {26, 22} -> re-perturb. And both lines
are now hard fixpoints of everything built so far: 159.092 survives 90
arms plus the 320-arm sweep, 164.040 survives 238 runs. The dominant
residue on the record line is always the same - exactly one violating
pair, depth bound fully satisfied, zero boundary pieces - and it is a
pair that no translation can separate. Every mechanism in the current
cascade moves pieces without changing their poses. The degree of
freedom the whole stack lacks is now the pose itself: rotation, mirror,
re-placement *inside* the global solve rather than before or after it.
Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/{record-159.092,from-scratch-164.040}/.

## The ExplorationKernel seam is in place (production roadmap PR3)

The trait this plan has referred to since the collision-kernel decision
checkpoint now exists as code, in `crates/polygon-nesting-core/src/search/kernel/`.
`ExplorationKernel` declares the four geometric services the exploration
loop consumes - the proxy pair collision test, the proxy overlap
magnitude, the exact collision-polygon build, and the exact pair-overlap
verdict - and `LegacyKernel` is the current code path behind them,
forwarding to the functions that already implemented each one.

The seam has two tiers, and the distinction is the load-bearing part.
The proxy tier is bound generically: `LaneSearch` carries a
`K: ExplorationKernel` parameter that defaults to `LegacyKernel`, so a
kernel that accelerates the ~22.8M `pairCollide` calls PR1 measured is a
type substitution at the entry point, not a rewrite of the sweep. The
exact tier is bound by *name*, through a `LEGACY` constant, at every call
site that can reach a published placement. That asymmetry is how this
plan's refusal to put `f32`, a tolerance, or a foreign engine into
publication authority becomes a property of the type system rather than a
convention: no substitution can reroute an exact answer, because no exact
call site is generic. The independent source-ring validator is untouched
and remains the sole publisher.

The boundary is free. `LegacyKernel` is zero-sized, its methods are
`#[inline(always)]` forwarders, and the generic lane search has exactly
one instantiation, so monomorphisation reproduces the previous direct
calls; there is no `dyn` on any hot path. An interleaved six-round A/B of
the mode-20 stream against the pre-seam binary on the same machine gives
27.456 s median before and 27.425 s after (ratio 0.9989, ranges
27.28-27.75 and 27.36-27.69), with both arms producing
`independentDepthMm` 206.869 at fingerprint `8a7737381238fa4d...`. The
mode-22 record replay likewise reproduces `rawSourceDepthMm`
159.09233022733062 at fingerprint `fa01012af1d559ae...`.

`JaguaKernel` is the second implementation, built against the pinned
`jagua-rs` 0.7.2 behind the existing `jagua-experimental` feature. It is
a skeleton and it is wired into nothing: no production route, no CLI
mode, and no default path constructs one. Its exact tier forwards to
`LegacyKernel` verbatim, which is asserted rather than merely documented.
Its parity smoke test hands both kernels the same Mixed-61 source
geometry at the same poses and requires identical verdicts outside an
ambiguity band derived from the `f32` representation error at the
magnitudes the query works at - classified by growing and shrinking the
exact rings by that band, so the band never enters as slack in the
comparison. None of the dependency gates listed above are claimed by it;
it is the compiling target they will be measured against.

Two boundaries of PR3 are worth writing down, because they set PR4's
scope. First, the seam opens the *query*, not the shape: the oriented
representation the proxy tier consumes is still the legacy surrogate and
the catalogue that owns it is still concrete, so the lane search binds
`K::Shape = OrientedSurrogate`. A kernel that accelerates the query over
that representation is swappable today; a kernel with its own
representation additionally needs the catalogue, the pair-NFP builder,
and the pose-bounds helper moved behind `ExplorationKernel::Shape`.
Second, `build_collision_polygon` deliberately carries no instrumentation
of its own, because its constructor caller opens a
`CollisionPolygonBuild` span while its deep-operator caller uses
`profiling::deep`, which is compiled out by default; owning a span in the
shared primitive would force one of those two contracts onto the other.

## The pose-entry experiment: the missing degree of freedom, measured

The residue analysis said the stack lacked pose changes; modes 28 and
29 nominally supply them; the experiment asked whether they actually
do. The answer is no on the record basin, yes-by-one-quantum on the
from-scratch basin, and the difference is itself the finding.

On the record line, 309 runs produced 59 legal states and zero
publications below 159.092. Ten of those states sit at exactly the
incumbent depth with ten distinct fingerprints - which looked like
fixpoint-breaking material until the geometric diff against the parent:
zero rotation change and zero mirror flips on all ten, rotations
preserved to the last digit. The source explains it: anchor-local
re-insertion seeds the vacated pose under the anchor's own orientation
prior and searches translations on a micron grid, so a layout built
from continuous fine angles (179.977611, 350.661672, 9.190204 degrees)
can never see an alternative orientation at record density. The
nominal pose-changing modes are, on this basin, a translation-only
operator - and mode 31 accordingly sees the identical one-pair residue
from every one of their outputs. Worse, the pieces that control the
depth are exactly the frozen ones: the per-rank ejection sweep shows
the three depth-setting pieces admit zero re-placement finalists while
ranks 3-15 publish freely. The record fixpoint now spans 149 arms and
seven modes.

On the from-scratch line the same modes moved the line - by exactly
0.001 mm, to 164.038568, and the constants that cap the mechanism are
now measured. The productive perturbation is not the nudge (1-2 mm
displacements manufacture a six-piece component the re-placement
cannot house) but a frontier flatten of 0.002-0.004 mm, which
manufactures exactly one violating pair in a two-piece component that
mode 29 re-places on the first insertion order tried - under a LOOSE
bound, because the clamp gates acceptance rather than driving descent.
Every publication lands on the 0.001 mm pose-grid quantum, so this is
the minimum move the mechanism can express; and the ejection limit of
7 pieces meets a frontier whose ranks 1-8 sit within 0.0225 mm, so any
perturbation worth 0.1 mm or more trips the cap. Structural ceiling:
~0.02 mm per move. The one durable asset is diversity - 42 distinct
legal frontier states where translation-only modes ever produced one,
and mode 31 wakes up again on several of them.

The conclusion is no longer a hypothesis but a measurement: the next
mechanism must generate re-insertion candidates at DIFFERENT
orientations - continuous angle perturbation around the vacated pose,
at record density, inside the ejection-repair loop. Nothing in the
current stack can express it; everything else has been eliminated with
matched controls. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/{pose-entry-negative,from-scratch-164.038}/.

## Continuous-angle re-insertion breaks the translation fixpoint

The mechanism the previous entry called for now exists, as modes 32 and
33 - modes 28 and 29 with one added degree of freedom and nothing else
changed. Each ejected piece's candidate stream additionally carries
orientation-perturbed variants of its own vacated pose: a geometric
continuous-angle ladder in both signs and, where the request allows it,
the mirror flip; each variant re-centred on the vacated footprint's own
bounding-box centre so that a rung rotates the piece *in place*, and
each searched over the same local translation neighbourhood the vacated
pose gets. The ejection machinery, the component and ejection limits,
the insertion-order enumeration, the pose-swap round, the finalist beam,
the bound contract and the exact validator are reused verbatim.

It works, and the record moved for the first time since 159.092. The
record line's fixpoint - 149 arms across seven modes - fell on the first
perturbed arm tried: a frontier flatten of 0.004 mm handed to mode 33
published 159.089637, below the 159.092330 the whole previous campaign
could not beat. Cascaded to 159.082637 over three rounds, a 0.009693 mm
absolute improvement.

The attribution is the finding, not the number. The pose-entry negative
measured `maxdRot 0.000000` and `mirrorFlips 0` across every legal state
modes 28 and 29 produced; the geometric diff of the new record against
the old shows two pieces rotated by exactly -0.02 degrees - the ladder's
finest rung - and one of them is
`54345eb7-a37e-45eb-b0fd-eccffdfa14cc-copy-3`, the single piece that
*sets the depth* and that the per-rank ejection sweep had measured as
having zero re-placement freedom. On that piece the legacy anchor-local
stream returned 0 exact-valid finalists out of 122 candidates, exactly
as before; the orientation stream returned 2 out of 3509, and the pose
the piece committed to was one of them. Every sub-record publication on
the line carries `acceptedOrientation >= 1`. The frozen pieces are
frozen in *translation*, not in pose.

Three secondary measurements matter for what comes next. First, the
accepted rung is overwhelmingly the ladder's finest: of the accepted
orientation poses across the whole campaign the great majority are at
+/-0.02 degrees, with a minority at +/-0.125 degrees and a family of
pure mirror flips. The mechanism's per-move quantum is now the ladder's
finest rung, exactly as it used to be the 0.001 mm pose grid - a finer
rung is the obvious next lever, and the ladder is one constant. Second,
the row budget is load-bearing rather than incidental: at a budget that
truncated the stream to its leading ranks the depth-setting piece
produced no finalist at all, and at full coverage it produced two.
Third, mode 33 is the productive tier and mode 32 is not - the joint
pass ejects both endpoints of the conflict, and the orientation freedom
only pays when the partner can move out of the way at the same time.

Mode 33 is the productive tier and mode 32 is not: on the 64-arm record
grid mode 33 took 4 of the 4 sub-record publications and mode 32 took
none, though mode 32 did accept 2 orientation poses. The reason is the
vertex cover - mode 32 leaves the conflict's partner in place, so the
rotated piece still has to clear a neighbour that cannot move, and the
tie-break that ejects the innocent neighbour of a nudged piece is still
self-defeating. Mode 33 ejects both endpoints regardless.

The from-scratch line at 164.038568 did not move: 64 grid arms plus 224
launch-pad arms produced 15 orientation-accepted poses and 50
publications below their own launch pad, but none below the incumbent.
That is a real asymmetry and it is consistent with the earlier reading
of the two basins - the from-scratch frontier's ranks 1-8 sit within
0.0225 mm, so the pieces that would have to rotate are not the ones that
set the depth.

The new state is certified (modes 27, 30 and 22 seeds 0-3 all replay
`exactValid` and `contractValid` and reproduce raw 159.08263749731248 at
fingerprint `145d0ed4b2f53d3f...`, mode 30 reporting 0 violating pairs
and 0 boundary pieces) and is again a fixpoint of 120 probe arms -
though of everything *except* the mode-26 ladder tier, which adopted
nothing in four cascade rounds and consumed most of the wall clock; the
cascade was stopped part-way through round four for time rather than run
to a certified fixpoint.

Goal threshold 155.000 mm remains far off; the honest claim is narrower
and more useful: the translation fixpoint is not a fixpoint, and the
instrument that breaks it is continuous-angle re-insertion inside the
ejection-repair loop. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/orientation-entry/.

## Delta scoring, and what auditing it found (production roadmap PR4)

PR4's mandate was to make the relaxed sweep's score a delta: dense pair
state, a per-move moved row, incident updates, a one-hazard commit, and
no whole-layout rescore in the hot loop. Most of that list turned out to
describe code that already existed. `PairTracker` was already a dense
triangular array carrying per-piece incident sums, `score_placement`
already scored only the moved row against a broad-phase index, and
`commit_dynamic_hazard` already installed one pose. What did not exist
was any check that the delta was *right*, and what that check found is
the substantial result of this stage.

### The audit, and the finding

`search::shadow_rescore`, behind the `shadow-rescore` feature,
recomputes the complete layout score after every accepted move and
compares it against the incrementally maintained one. It classifies a
disagreement rather than merely counting it, because three different
claims are involved. **Structure** - which pairs collide, how many rows,
how many boundary violations - can never depend on evaluation order, so
a difference there is a delta that has lost the layout. **Row magnitude**
is one `f64` measurement of one pair. **Derived sums** are running `f64`
totals whose last bit depends on accumulation order. The weighted total
is recomputed in the delta's own summation order before comparing, so
what is under test is the delta and not which of two orders `+` was
applied in.

Run on the base commit, before any change, the audit says the
incremental score has never equalled a from-scratch one:

| stream | audited moves | structural | magnitude-only | derived-gap only | bit-identical |
|---|---:|---:|---:|---:|---:|
| mode 20 | 52313 | 0 | 25107 | 22859 | 4347 |
| mode 22 | 121463 | 534 | 80693 | 34572 | 5664 |

The magnitude class has a mechanical explanation. The proxy pressure
kernels accumulate a pole-pair series with the first operand outermost,
so reading a pair as `(moving, fixed)` and reading the same pair as
`(lower index, higher index)` are two summation orders over the same
terms. A candidate scorer always does the former and a complete score
always does the latter, so the two legitimately differ in the low bits
whenever the moved piece is the higher-indexed one - the asymmetry
`CoupledRollbackComparison` already documents for the coupled arms,
generalised to every accepted move. The widest gap seen is 4.83e9 `f64`
ulps.

The structural class has no rounding explanation, and 534 of mode 22's
moves are in it. Its signature is a collision row count off by one. Two
mechanisms in the sweep can produce it: when a dynamic move is rejected
and reverted, the row installed for the reverted piece is read back out
of the tracker by `tracked_piece_score` rather than re-measured, so a
row that has drifted stays drifted; and the candidate scorer's colliding
set comes from the hazard index while a complete score's comes from the
surrogate collider, which are two different oracles. This is
pre-existing, it is deterministic, and every published layout still
passes the exact validator - but it means the tracker is a *guidance*
structure rather than a measurement of the layout, and it must not be
read as one.

That is also the measured reason a sweep cannot simply carry its
predecessor's tracker forward instead of rescoring, which was the
obvious way to delete the per-sweep rescore outright. The two objects
are not the same object; deleting the rescore that way would change
outcomes, so it was not done.

### What was made a delta

Three quadratic costs were removed without changing a single answer.

The confirmation collider opened with a broad-phase reject against its
two operands' transformed surrogate extents and derived both of them
itself on every call - a walk over every cell vertex of both shapes. One
piece asked about all of its neighbours therefore re-derived its own
extent once per neighbour, and a whole-layout score re-derived every
piece's extent n-1 times; on the mode-22 stream, whose 121463 accepted
moves each confirm a full 60-pair row and whose 1951 rescores each visit
1830 pairs, that is of order 10^7 walks. `ProxyRowCache` holds one
extent per piece together with the pose it was taken at and re-derives
on a pose mismatch, so it is self-invalidating and a sweep that moves
one piece invalidates exactly one row.

The whole-layout score resolved both operands' oriented surrogates per
pair, asking the ordered catalogue n-1 times for each of the n answers
it needed; it now resolves one shape per piece behind a cloned catalogue
handle. The same score also ran a `pair_weight` sweep over every pair
whose results the pair loop immediately overwrote.

`update_score_after_move` answered "what is this pair's new loss?" with
a linear `find` over the moved row once per piece, and rebuilt the whole
collision list by retain-extend-sort on every accepted move. Both are
now linear merges over data that was already sorted, into a lane-owned
scratch that stops allocating after a lane's first move.

Deliberately *not* changed: the two running `f64` sums. The boundary and
weighted totals could be maintained by subtract-and-add, but their
accumulation order is observable in the last bit and the rollback
auditor compares them against a complete score.

### What it is worth, measured

The whole-layout rescore now costs about a third of what it did.
Normalised against untouched phases in the same profiled run it is
0.357x on mode 20 and 0.318x on mode 22, and its share of leaf time
falls from 2.41% to 0.86% (mode 20) and from 2.15% to 0.70% (mode 22).

Those percentages are the honest headline, and they are small. The
roadmap ranked "whole-layout rebuild/rescore around small moves" second
among the cost centres; the fixed-stream profile says it was worth
2.2-2.4% of leaf time, because sweeps are rare against candidates -
1101 rescores against 5.85M candidate queries on mode 20. The measured
centres are where PR1 said and where PR3 aimed: `pairCollide` at 45-48%
of leaf time and `pairPressure` at 19-21%, both inside `scorePlacement`.
A large factor out of this engine is kernel work, not accounting work.

End to end, on a ten-round interleaved A/B whose arms alternate order
every round and whose statistic is the per-round paired ratio (the box
is shared and its load drifts, so a ratio of medians would measure the
drift): mode 22 goes from a 6.483 s median to 6.047 s, paired ratio
median **0.935**, and all ten rounds are below 1.0 with the spread
0.876-0.949. Mode 20 goes from 27.673 s to 27.507 s, paired ratio median
**0.995**, nine of ten rounds at or below parity and the tenth a round in
which both arms were disturbed together. Mode 20 is the stream whose
sweep work is the smaller share, and 0.5% is at the edge of what this
machine can resolve; mode 22's 6.5% is not.

The equality evidence is stronger than the gates require. Every counter
in both fixed-stream profiles is identical across the change - candidate
queries, neighbour tests, SAT tests, cell probes, broad-phase probes,
accepted moves, publication attempts. Every non-timing field of the
mode-20 anchor, the mode-22 record replay, a mode-26 ladder and a
mode-31 solve is identical, failure-reason text included. And the audit
re-run on the changed engine reproduces the base tallies exactly -
121463 checks, 534 structural, 80693 magnitude-only, 34572 derived-gap,
the same worst-ulp figures and the same first rendered disagreement -
so the per-move tracker trajectory itself is unchanged, not merely the
published outcome.

### What is left

The sweep still opens with a complete rescore. It is now a cheap one,
but it is still `O(n^2)` in pair visits, and removing it entirely needs
the tracker to become a measurement of the layout rather than a
guidance structure - which is the structural disagreement above, and is
its own change with its own outcome risk. The natural order is: settle
which oracle owns a row (the hazard index or the surrogate collider),
stop `tracked_piece_score` reinstalling an unmeasured row on a reverted
move, and only then let a sweep inherit its predecessor's tracker.

> **Superseded.** Both mechanisms named above were inferred from
> reading, and measuring them refuted both: the reverted reinstall is an
> identity on every row it touches, and the hazard index is not in the
> loop on the streams that disagree. The single cause is that a pair
> question is not a function of the unordered pair - the proxy collider
> answers differently asked `(moving, fixed)` and `(lower, higher)`. See
> "Who owns a row: the pair question is not a function of the pair"
> below for the census, the decision, and its measured price.

## The ladder's floor was in the wrong place, and moving it moved the record

The previous entry ended with a lever and a reason to pull it: the
accepted orientation rungs were piling up on the ladder's *finest*
setting, and "a finer rung is the obvious next lever, and the ladder is
one constant". Pulling it produced a new absolute record on the true
5.0/5.0 exact-clearance contract - **159.078760 mm raw, from
159.082637** - and, more usefully, an attribution clean enough that the
mechanism is no longer in question.

### The floor's justification did not survive arithmetic

The floor was 0.02 degrees, defended on the ground that a finer rung
"cannot move a vertex of a hand-sized piece by even one placement-grid
quantum". That is checkable and it is wrong by a factor of thirty-five.
A rung `d` degrees moves a vertex at radius `r` from the rotation centre
by `r * d * pi/180`; on a hand-sized 100 mm radius the old floor travels
0.035 mm against a 0.001 mm pose grid. The rung that actually stops
being expressible on that radius is nearer 6e-4 degrees. The floor was
sitting an order of magnitude above the band it was meant to bound, and
the campaign's own accepted poses - overwhelmingly *at* the floor - were
the symptom.

So the ladder gained two rungs at the same 5/2 ratio, 0.008 and 0.0032
degrees, still spelled out rather than computed. Nothing else moved:
budgets, caps, ejection limits, insertion-order enumeration, the
pose-swap round, the finalist beam, the bound contract and the exact
validator are untouched, and the ordering rule is unchanged, so the new
finest rungs simply lead. The single knock-on is derived, not tuned:
`ORIENTATION_PERTURBATION_VARIANTS` goes 29 -> 37 and the charged-row
budget follows it by the rule it already followed, one anchor-local
budget per variant. Freezing the budget at the old length would truncate
the stream, which is the failure the constant exists to prevent. The
ladder test now states the expressibility claim as arithmetic instead of
prose.

### The A/B is decisive on the arm that matters

A frontier flatten of 0.001 mm on the 159.083624 lineage pin, handed to
mode 33 at a loose bound, run under both binaries:

| ladder | variants | candidates | orientation acceptances | rungs accepted | published |
|---|---:|---:|---:|---|---|
| old (7 rungs) | 1015 | 71050 | 12 | 4x mirror, 4x 0.125, 4x mirror-0.125 | **nothing** |
| new (9 rungs) | 74 | 5180 | 1 | 1x **0.0032** | **159.08262371460316** |

The old ladder exhausts its insertion orders and fails with "no
insertion order re-placed the 1 violation components inside the bound".
The new ladder accepts one pose at a rung that did not previously exist
and publishes an exact-valid, contract-valid state - on a *fourteenth*
of the candidate volume, because it succeeds on the first insertion
order instead of enumerating its way to a refusal. A finer rung is not a
bigger search; it is a search that does not need to be big.

### The descent, and where the improvement actually came from

Cascading from 159.082637 with modes 33 (flatten and nudge entries),
22 and 31, plus mode 33 on flattened states of the three lineage pins,
reached a fixpoint in four rounds and 110 arms:

| via | tier | raw | delta | accepted rung |
|---|---|---:|---:|---|
| flatten 0.001 of lineage pin 159.083624 | lineage | 159.08262371460316 | -0.000014 | -0.0032 |
| nudge rank 1 by 0.006 | nudge | 159.08176040364793 | -0.000863 | -0.008 |
| flatten 0.003 | flatten | 159.07876040364795 | -0.003000 | -0.0032 |

Every adoption is one orientation acceptance at one of the two new
rungs, with `acceptedAnchorLocal = 0` and `acceptedStation = 0` - the
orientation stream did all of the work on all three, and the legacy
streams none of it. Across all 110 arms the rotation family's accepted
rungs are 27 at 0.0032 and 13 at 0.008 and **zero at 0.02 or coarser**.
The distribution did not merely shift toward the new rungs; it vacated
the old floor completely, which says the previous campaign's
concentration at 0.02 was a boundary artefact rather than a preference.

The geometric diff against the old record closes the argument. Two
pieces rotate, both copies of `54345eb7-a37e-45eb-b0fd-eccffdfa14cc` -
the depth-setting family - by -0.0064 and -0.008 degrees, with zero
mirror flips and only two pieces translated. Both deltas are exact
integer multiples of rungs the old ladder could not express: -0.0064 is
two applications of the new floor, -0.008 is one application of the
rung above it. There is no reading of that layout on which the old
ladder could have produced it.

Basin diversity paid, once and cheaply: the first adoption came from a
*lineage* pin 0.001 mm worse than the incumbent, not from the incumbent
itself. Carrying the runner-up basins forward cost four arms a round and
supplied the entry that broke the round open.

### A certified fixpoint, which the previous run could not claim

The new state is pinned at
`finer-ladder/pinned-parent-159.079.json`, sha256 `1535067297279e46...`,
fingerprint `e28fba007f8031d4...`, and replays exactValid and
contractValid reproducing raw 159.07876040364795 on modes 27, 30 and 22
seeds 0-3. The full certification battery then ran 40 further probe
arms on it - mode 22 seeds 0-3, mode 26 ladders x6, mode 31 tiny steps
x4, mode 27, mode 30, and the whole frontier-flatten delta grid handed
to mode 33 under *both* ladder generations - and **nothing published
below the incumbent**. The previous entry had to qualify its fixpoint
("of everything except the mode-26 ladder tier... stopped part-way
through round four for time"); this one does not. Mode 26 was kept out
of the descent rounds, where it had adopted nothing in four rounds at
~48 s an arm, and spent only here, where its six arms all reproduce the
incumbent exactly.

### The from-scratch line moved, and not for the reason being tested

The from-scratch basin at 164.038568 did finally produce a
sub-incumbent publication, 164.037568, in 24 arms. It is not the
rungs. The A/B says so: the base-commit binary reproduces that arm
bit-identically, same raw and same fingerprint `49f094d7e59a9008`, and
the accepted pose is a pure mirror flip the old ladder already carried.
What unlocked it is the finer *flatten* delta grid - 0.001 mm, a
perturbation this line had never been given, since the old grid ran
0.002/0.004/0.01/0.02. That is worth recording precisely because it is
the wrong answer to the question asked: a negative for the ladder and a
positive for the entry grid, and conflating them would have credited the
mechanism with someone else's result.

### What is left

Goal threshold 155.000 mm remains far off; 159.078760 is 0.003877 mm of
absolute improvement and the honest framing is unchanged from the last
entry - the value is in the mechanism, not the millimetre. Two things
the evidence now points at. First, the ladder floor is still a floor:
the accepted rungs have simply re-piled on the *new* finest rung, 27 of
40 rotation acceptances, so the same argument that justified this change
justifies testing another two rungs down, and the arithmetic says there
is roughly a decade of headroom before 6e-4 degrees. Second, the entry
grid is a lever in its own right and a cheaper one - the from-scratch
result came from adding a single flatten delta, and the record line's
first adoption came from perturbing a *runner-up* basin rather than the
incumbent. Evidence:
docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/.

## The measured hot path, precomputed (production roadmap PR5)

PR4 ended by naming the centres it had *not* touched: `pairCollide` at
45-48% of leaf time and `pairPressure` at 19-21%, both inside
`scorePlacement`, and it said plainly that "a large factor out of this
engine is kernel work, not accounting work". This stage is that kernel
work. Nothing about what the engine computes changed; what changed is
that the collider stopped re-deriving, on every one of a mode-22
stream's 52.0M pair questions, quantities that are properties of a shape
or of a pose.

### Where the time actually was

The phase table says `pairCollide`; it does not say what inside it. A
counting build of the collider answered that, and three of its four
findings were not what the shape of the code suggested.

| quantity | mode 22 | mode 20 |
|---|---:|---:|
| `surrogate_pair_collides` calls | 51,962,803 | 22,821,625 |
| rejected on the shape extent | 21,539,780 | 9,775,438 |
| cell-index probes | 77,049,450 | 31,557,929 |
| ... whose cell misses the whole other shape | 30,404,905 | 12,068,282 |
| bin-membership iterations | 1,535,586,686 | 620,966,289 |
| distinct cell bits produced | 157,316,634 | 63,200,794 |
| separating-axis tests | 80,366,750 | 33,236,688 |
| ... axes evaluated on the *first* triangle | 182,368,721 | 75,991,281 |
| ... axes evaluated on the second | 83,375,467 | 35,816,953 |
| pole pairs quantified | 410,114,462 | 170,742,109 |

The bin index was the surprise. It was a `Vec<Vec<usize>>` of membership
lists and a query walked every list in the covered bin rectangle setting
one bit per entry - 1.54 *billion* pointer-chased iterations to produce
157M distinct bits, because a cell sits in every bin its extent touches
and a query covers 9.4 bins. Each bin now carries the bitmask of its own
cells and a query ORs 9.4 words out of one contiguous array. The bit set
is identical: `|` is idempotent and commutative, so neither the
duplication nor the visit order was ever observable.

The second was scale. `MAX_CELLS_PER_PIECE` is 512, so every probe
zeroed and scanned an eight-word mask - while a real Mixed-61 surrogate
carries 4.3 cells. The index now reports how many words are live.

The third was that 39.5% of cell probes are asking about a cell that
misses the *whole* other shape. A surrogate's extent is the fold of the
same ring points its cells are triangulated from, so every cell's extent
is inside it and translating both by the same amount preserves that; a
reported cell would have failed its own extent test anyway. The probe is
still charged, so `cell_index_probes` and `sat_tests` are untouched.

The fourth is the narrow phase. It is called with its first triangle at
a zero translation, which makes that triangle's three edge normals - and
its own extent along each of them - functions of the cell alone. They
are 68.6% of the stream's axis work, three `hypot` calls and six
divisions each, and they are now built once per oriented surrogate. The
magnitude the deriving function returned is dropped, because both of its
callers only ever asked `.is_some()`.

What was left after that was the work of finding out *which* shapes a
question is about. A pose's surrogate key runs `rem_euclid` three times
- three out-of-line `fmod`s - and the collider derives it for two poses
per question, while the poses barely change: a candidate scan holds one
candidate pose fixed across every neighbour, and a fixed piece's pose
changes only on an accepted move. One memo slot per piece, keyed on the
rotation's bit pattern for the reason `ProxyRowPose` is, answers almost
all of it. And the scan itself descended the ordered catalogue `2k + 2c`
times for `k` neighbours and `c` collisions - twice per pair in the
collision test and twice more in the pressure quantification, for the
same two shapes; it now descends `k + 1` times.

That last one carries a measured negative worth recording. Holding a
resolved shape across the scan keeps the catalogue borrowed, so the
first attempt took a second `Arc` handle the way PR4's whole-layout
score does. At 1,951 rescores that costs one refcount bump; at 13.6M
candidate scans it costs eight lanes contending on one cache line, and
it made the stream *slower* - 3.9 s to 4.0 s, with `scorePlacement`'s
own work up 2.8 s while the two named phases fell. Destructuring the
lane into disjoint field borrows buys the same freedom for nothing. The
phase table would have called the `Arc` version a win; the wall clock
called it a loss, and the wall clock is right.

### What it is worth, measured

Profiled, against the pre-PR binary on the same box, normalised against
the untouched leaf phases in the same run (`exactOverlapTest`,
`constructorProposals`, `pieceIndexBuild`, `boundaryPenalty`, which
together drifted 0.971x on mode 22 and 0.940x on mode 20):

| phase | mode 22 raw | mode 22 norm. | mode 20 raw | mode 20 norm. |
|---|---:|---:|---:|---:|
| `pairCollide` | 0.374 | 0.385 | 0.360 | 0.383 |
| `pairPressure` | 0.743 | 0.765 | 0.695 | 0.739 |
| `scorePlacement` | 0.666 | 0.686 | 0.635 | 0.676 |
| `moveSweep` | 0.682 | 0.703 | 0.654 | 0.696 |

`pairCollide`'s share of leaf time falls from 47.65% to 28.69% on mode
22 and from 45.46% to 25.83% on mode 20. `pairPressure`'s share *rises*,
from 19.29% to 23.07% and from 18.26% to 19.99%, and that is worth
saying plainly rather than quoting the ratio alone: its absolute cost
fell by a quarter, entirely because the double shape resolution it was
paying for sat inside its span, but the leaf total fell faster. The pole
loop itself is untouched.

End to end, on ten-round interleaved A/Bs whose arms alternate order
every round and whose statistic is the per-round paired ratio:

| stream | before | after | paired median | spread | rounds below 1.0 |
|---|---:|---:|---:|---|---:|
| mode 22 | 4.847 s | 3.597 s | **0.743** | 0.731-0.755 | 10/10 |
| mode 0 | 2.443 s | 1.895 s | **0.777** | 0.745-0.785 | 10/10 |
| mode 20 | 27.239 s | 26.562 s | **0.975** | 0.974-0.977 | 5/5 |

Mode 20 is the stream whose wall time is mostly deep-operator exact
geometry - `exactOverlapTest` alone is 19.8% of its leaf time after this
change - so a 2.5% end-to-end win against a 0.383x proxy collider is the
expected shape, not a disappointment. No stream regressed.

The equality evidence is the standard PR4 set. Every profile counter is
identical on both streams - candidate queries, neighbour tests, SAT
tests, cell probes, broad-phase probes, accepted moves, publication
attempts. All three pinned regression gates are bit-identical: mode 20
at `independentDepthMm` 206.869 and fingerprint `8a7737381238fa4d`, the
mode-22 record replays at raw 159.09233022733062 and 159.08263749731248
at `fa01012af1d559ae` and `145d0ed4b2f53d3f`. The mode-26 ladder and the
mode-31 arm agree field for field, failure-reason text included. And the
shadow-rescore audit rebuilt on this tree reproduces PR4's control
tallies exactly - 52313/0/25107/22859 on mode 20 and
121463/534/80693/34572 on mode 22, with the same worst-ulp figures and
the same first rendered disagreement - so the per-move tracker
trajectory is unchanged and not merely the published outcome.

### The one thing that could not be both

After all of the above, `hypot` *is* the proxy tier: 410M pole-pair
separations and 83M axis normalisations on the mode-22 stream, measured
at 7.7 ns/call on this box against 1.3 ns for `sqrt(x*x + y*y)`. It is
also the one optimisation here that cannot be taken bit-identically. The
platform's `hypot` is correctly rounded and the naive form is not: a
5M-sample probe at this engine's magnitudes puts them one unit in the
last place apart on 16.9% of inputs. The Rust `libm` port already in the
dependency tree is no way out either - 3.7x faster than the platform
call, and disagreeing with it on 13.9% of the same inputs.

So it is `fast-proxy-hypot`, off by default, confined to two
pole-pressure kernels and two axis normalisations, none of which can
reach a published placement. Its measured outcome is better than the
arithmetic predicted: on all three regression streams *and* on the
mode-26 and mode-31 arms the flagged build reproduces the unflagged one
exactly, every arm `exactValid` and `contractValid`, while mode 22 runs
3.2 s against 3.7 s. Five agreeing streams is evidence and not a
guarantee, and no published fingerprint in this repository was measured
under it, so it stays off - a candidate for the anytime coordinator to
evaluate on a corpus rather than an optimisation to adopt on a fixture.

### What is left

The proxy collider is now bounded below by two things it cannot shed
without changing an answer: `hypot`, above, and the four divisions
`bin_range` performs per cell-index query, which cannot become a
reciprocal multiply because a differently-rounded bin can drop a cell
from the mask. Beyond those, the next measured centre is no longer in
this file at all - on mode 20 it is `exactOverlapTest` at 19.8% of leaf,
which is Clipper inside the deep operators, and that is PR6's port of
the constructor and vacancy operators onto the shared proxy kernel.

## Sol review 3: the substrate is at parity; the gap moved

The third adversarial review (docs/sol-review-3-production-convergence.md,
verbatim) opens with a measured correction that retires this ledger's
oldest premise. Replaying the m22 stream against the profiling counters:
3.775M candidate evaluations/s at ~265ns each against Sparrow's 3.742M
at ~270ns, and 33.9K effective moves/s against Sparrow's 14.2K. The
"3-4 orders of magnitude per move" statement is now false for m22. What
remains slow is everything around the loop: mode 20 needs ~13x even
with its dominant phase free, mode-26 certification rungs are 12-95
seconds of operator work against a 0.5-1.0 second production slice, and
no single process reproduces the from-scratch causal chain (worse
constructor basin -> waves -> sibling retention -> crossover ->
compression) at any speed. Quality per unit work, not work per second,
is now the deficit.

Seven findings, all accepted. The two critical ones reshape PR6/PR7:
there is still no evidenced 160-in-10 path (orchestrating existing
mode schedules cannot honestly claim it), and PR5's adoption logic -
correct as publication policy - is wrong as the coordinator's only
state model, because it discards exactly the worse-but-structurally-
different basins the from-scratch lineage was built from. PR7 needs a
PublishedIncumbent and a typed SearchArchive as separate objects. The
high findings: the certification comparator hid ~35 ULPs (fixed, below);
the kernel seam is not yet the production seam (exact methods must
leave the proxy trait; a lane-owned query_moved_into is the PR6 shape);
the row tracker's 534 structural disagreements need causal attribution
and one canonical measure_row before any tracker inheritance; the
orientation ladder is not scale-free (angular rungs must derive from
displacement rungs over piece radius, delta-theta = delta-x / r) and
modes 28/29/32 leave the production portfolio, with m33 demoted to a
triggered tail repair. The hypot flag's blast-radius claim was wrong
(proxy scores steer which layout reaches publication); the stronger
shape is squared-distance comparison with sqrt only on overlapping
pairs.

The comparator finding was acted on immediately. certify_full.py's
`RAW - 1e-12` concealed five exact-valid publications one ULP below the
declared record, one with a distinct fingerprint - and exposed that the
parent-measure and publication-measure paths round one ULP apart on
identical placements. Policy now: the declared record raw is the
publication-authority measure (159.07876040364792), replay reproduction
is identical fingerprint plus raw within one ULP, and below is strict
raw < with no decimal epsilon. Recertified under that policy: 40 arms,
replayPass true, zero below, fixpoint true (certulp.json). The distinct
co-state is pinned (lineage/pin-159.078760-alt-5ddb62de.json) - the
frontier now provably holds at least two layouts at the record depth.

The reordered board, per the review: the quality frontier trace first
(one process, from request only, every exact-valid candidate logged
with elapsed time, work ordinal, operator, parent fingerprint, archive
membership - the depth-versus-time curve nobody has measured), then the
row-ownership oracle as a correctness gate for PR6, then PR6 as seam v2
plus deep-operator port starting with mode-20 basin generation (the
vacancyProxyRank redesign is part of PR6, incremental occupancy and a
bit-grid flood fill, not loop tuning) and a fused compression->m31
legalizer, then a thin PR7 harness before the full coordinator. Cut
from the production path: full mode-26 ladders, modes 28/29/32 and
standalone 30, unconditional m33, fixed-angle floor tuning, the Jagua
pair-layout skeleton as a production target, and bit-identity as a
requirement on the NEW profile - protected legacy stays bit-identical,
the new profile owes per-seed determinism and exact-valid publication.

## Who owns a row: the pair question is not a function of the pair

PR4's audit left the roadmap a three-step order: settle which oracle
owns a tracker row, stop the reverted move reinstalling an unmeasured
one, and only then let a sweep inherit its predecessor's tracker instead
of rescoring. Step one is answered here, and answering it dissolved
steps two and three into a single, different, and much sharper fact.

### The two named causes were both wrong

PR4 named two mechanisms for the structural disagreements and named them
from reading rather than from measurement. Both are refuted.

*A reverted dynamic move reinstalls its row from `tracked_piece_score`.*
It does, and it is harmless. The reinstall writes back exactly the rows
it read, so every row it touches is unchanged; the only quantities it
perturbs are the running `f64` sums, through the `(x - a) + a` in the
boundary total and the `.max(0.0)` clamp in `replace_pair`, and those
are the audit's derived class by construction. The census confirms it
from the other end: every structural disagreement on both affected
streams is flagged `revert=false`.

*The candidate scorer's colliding set comes from the hazard index while
a complete score's comes from the surrogate collider.* On the streams
that disagree, the hazard index is not in the loop at all. The census
reports `confirmedRowLen -1` on every structural disagreement - no
confirmation ran, because those lanes are not on the dynamic-hazard
backend. Both the candidate scorer (`score_placement`) and the complete
score (`score_state`) reach the *same* function, `resolved_pair_penalty`,
over the same catalogue shapes.

### What is actually happening

They reach the same function with the operands the other way round, and
the function is not symmetric in them.

`surrogate_pair_collides` tests the first operand's *precomputed* cell
axes (`CellAxes`, with its projections taken at catalogue-build time in
local coordinates) against the second operand's points in a frame
relative to the first. Swapping the operands re-derives the same six
separating axes through different subtractions and projects them at a
negated offset. The two answers agree wherever a contact is decisive and
can differ outright where it is marginal - and the verdict is the whole
of the question, because `pole_overlap_pressure` is strictly positive
for *any* two shapes, so a row exists exactly when the collider says
`true`. The pressure itself is order-dependent too, for the reason PR4
already documented: the pole series accumulates with the first operand
outermost.

A candidate scan asks `(moving, fixed)`. A whole-layout score asks
`(lower index, higher index)`. So the row a sweep installs depends on
*which piece moved last*, and the row a rescore computes depends only on
the layout. That is the entire structural class, and the census renders
it in one line - here the first disagreement on the
`pinned-fs-parent-164.0376` stream:

```
moved piece 51, revert=false, shadow rows 32, tracker rows 31,
confirmedRowLen -1; shadow-only pair (33, 51) touchesMoved=true
partnerInMovedBroadPhase=true
proxyCollides(lower,higher)=true proxyCollides(higher,lower)=false
```

The broad-phase index did offer piece 33 to piece 51's scan
(`partnerInMovedBroadPhase=true`), so no spatial index is at fault; the
collider simply answered `false` asked as `(51, 33)` and `true` asked as
`(33, 51)`. The row is lost at that move and stays lost until one of the
two pieces moves again, which is why one dropped pair accounts for
dozens of consecutive disagreements.

### The decision

**The index-ordered pair owns the row.** This is forced rather than
chosen: a tracker row has to be a measurement of the layout if a sweep
is ever to inherit one instead of rescoring, and `(moving, fixed)` is
not a function of the layout - it is a function of the path taken to it.
`(lower, higher)` is the only order a rescore can reproduce without
knowing the move history.

One tier cannot implement the decision at all, and that is worth stating
plainly. The dynamic-pole tier answers
`collision_pressure(piece, pose, other)` - one *explicit* pose against
the committed layout - so its first operand must be the piece whose pose
is being proposed. It cannot be asked an index-ordered question, and
swapping its operands substitutes a piece's committed pose for its
candidate one. It is therefore excluded from the rule, and it is the
measured residue below.

`canonical-pair-order` implements the decision, off by default, at the
sites where the rule can be enforced: the kernel's proxy verdict
(`kernel_pair_collides`) and both pole-pressure sites. The confirmation
tier is deliberately left alone - the audit measured its verdict
agreeing bit for bit in both orders, so there is nothing there to fix,
and its magnitude reaches the rule through `rollback_pair_pressure`.

### What the decision is worth, and what it costs

Audited on the four regression streams, base against
`canonical-pair-order` (both `shadow-rescore` builds, both deterministic
- the base tallies reproduce exactly across repeat runs):

| stream | audited moves | structural | magnitude-only | derived-gap only |
|---|---:|---:|---:|---:|
| mode 20 anchor, base | 36382 | 0 | 16807 | 15804 |
| mode 20 anchor, canonical | 78094 | **0** | 113 | 75699 |
| mode 22 record-159.092, base | 97424 | 0 | 63885 | 28750 |
| mode 22 record-159.092, canonical | 139911 | **0** | 6438 | 125990 |
| mode 22 finer-ladder 159.079, base | 103150 | **14** | 68215 | 27883 |
| mode 22 finer-ladder 159.079, canonical | 145610 | **0** | 9588 | 126294 |
| mode 22 from-scratch 164.0376, base | 95689 | **121** | 61207 | 28657 |
| mode 22 from-scratch 164.0376, canonical | 138270 | **0** | 6745 | 123142 |

Two things to read out of it. The structural class goes to zero on the
two streams that had it, which is the diagnosis confirmed. And the
magnitude class collapses with it - from 46-66% of audited moves to
0.1-6.6% - because the same operand order governs the pole series. The
residue is the dynamic-pole tier: its worst gaps are `f32`-representable
values a relative 3e-7 apart, `4.12769409179687500e2` against
`4.12769317626953125e2`, which is what an `f32` engine asked
`(moving, fixed)` and `(lower, higher)` produces.

The cost is that this is a **trajectory change, not a refactor**, and
the numbers say so unambiguously. Audited move counts rise by 41-115%
on every stream: enforcing the order makes the search *see* the marginal
contacts it was previously dropping, so more pieces stay active and
sweeps run longer. All four regression targets still reproduce - 206.869
with fingerprint `8a773738…`, 159.09233022733062, 159.07876040364795,
164.0375677990678, every arm `exactValid` and `contractValid` - but the
mode-20 anchor's run reports a *different set* of lane fingerprints
(three rather than two, `20f1eba0…` and `712733a7…` alongside the
anchor's), so the runs are not the same search and must not be quoted as
one.

It costs time, and the cost is not marginal. On a ten-round interleaved
A/B of the mode-22 record stream whose arms alternate order every round
and whose statistic is the per-round paired ratio: 3.334 s median
against 4.246 s, paired ratio median **1.286**, nine of ten rounds above
parity, spread 0.860-1.329. The one round below parity is a round in
which the default arm was disturbed (4.784 s against its own 3.2 s
median). Extra work is exactly what the audited move counts predicted.

### Why steps two and three were not delivered

Step two's gate was "structural count to zero **with outcomes
bit-identical**". Structural-zero is reachable and is demonstrated above;
bit-identity is not, and the two are mutually exclusive rather than
merely hard to achieve together. The disagreement *is* a value
difference in the candidate scan - the pair `(33, 51)` either collides
or does not - so any change that makes the tracker agree with a rescore
necessarily changes what the search compared, and any change that
preserves what the search compared necessarily preserves the
disagreement. There is no third option to look for. Per the standing
rule that byte-identity outranks gate numbers, the default path is
unchanged and the rule ships off by default with its price measured.

Step three - a sweep inheriting its predecessor's tracker - stays
blocked, but the reason is now specific rather than general. It needs
the tracker to equal a rescore bit for bit, and that needs the magnitude
class at zero as well as the structural class. Under
`canonical-pair-order` the magnitude class is no longer the engine's
scoring being order-dependent; it is one tier, the dynamic-pole tier,
which cannot express an index-ordered pair question at its interface.
Closing it is a change to that tier's API, not to the tracker, and it
should be priced as such.

### What is left

* The dynamic-pole tier's interface, if the inherited-tracker
  optimisation is still wanted. Its upside is bounded by PR4's own
  measurement - the whole-layout rescore is 0.86% of leaf time on mode
  20 and 0.70% on mode 22 - so it should be ranked against that, and
  the trajectory change above is not obviously worth 0.7%.
* `canonical-pair-order` evaluated on a corpus rather than on four
  fixtures. The honest claim today is that it makes the tracker a
  measurement of the layout and that it costs time on these streams;
  whether a search that stops dropping marginal contacts packs *better*
  is a question four pinned regressions cannot answer.

## The pole loop has no bit-identical headroom, and the flag that would move it moves the search

PR5 handed this stage two things: `pairPressure`, which its own work had
pushed from 19.29% of mode-22 leaf time to 23.07% by making everything
around it cheaper, and `fast-proxy-hypot`, an opt-in whose entire
evidence was five agreeing Mixed-61 streams. Both were named as
unfinished. This entry finishes them, and the answer on both is
negative - the first because the loop has nothing left to give, the
second because what it gives has a price that was not being measured.

### What the pole loop actually does, counted

A counting build of `pole_overlap_pressure`, on the two pinned gate
streams - the mode-22 record replay at gate 2 and the mode-20 anchor at
gate 1, which is a narrower pair of streams than PR5's table and so
reports smaller totals for the same work:

| quantity | mode 22 gate | mode 20 gate |
|---|---:|---:|
| `pole_overlap_pressure` calls | 17,139,907 | 6,363,087 |
| pole pairs quantified | 330,270,986 | 106,492,468 |
| ... penetrating (`penetration >= epsilon`) | 12,395,921 | 4,063,850 |
| ... on the decay branch | 317,875,065 | 102,428,618 |
| ... whose circles are disjoint | 315,450,166 | 101,655,245 |
| ... whose gap exceeds 10 `epsilon` | 280,606,859 | 90,710,761 |
| ... whose gap exceeds the shape's own diameter | 7,691,023 | 2,652,539 |
| ... whose gap exceeds 1000 `epsilon` | 0 | 0 |
| **... whose contribution rounds to nothing** | **0** | **0** |
| poles per first operand, mean | 4.21 | 3.67 |
| poles per second operand, mean | 4.63 | 4.40 |
| poles, maximum | 13 | 13 |

The last row of the pair block decides the stage. The obvious
optimisation for a quadratic loop over circles is to bound the pair
cheaply and skip the ones that cannot matter, and 95.5% of these pairs
are disjoint circles - the shape of the data says the skip should pay.
It cannot, and not because it is unprofitable: because there is nothing
to skip. The decay branch contributes
`pi * eps^2 * min(r) / (gap + 2 eps)`, which is positive for every gap,
and the accumulator it lands in only ever grows. An addition is a no-op
in binary64 only when the addend is below half an ulp of the
accumulator, and since the accumulator starts at `eps^2` that needs
`gap + 2 eps > pi min(r) 2^53` - at the smallest pole radius this engine
generates, a separation upwards of `10^15` millimetres, and it only gets
further away as the sum grows. The counting build agrees empirically: of
436,763,454 pole pairs across the two streams, **zero** have a
contribution the running sum would discard. Every pair is load-bearing
in the last bit, so a bound that skips one changes the answer. That is
not a tuning question, and no amount of instrumentation was going to
turn it into one.

What the counts *do* show is redundant arithmetic. The second operand's
translated centre is derived inside the inner loop, so it is
re-evaluated once per *first* pole - 4.21 times per call on mode 22.
The stream performs 660.5 million of those additions where 158.8
million would do, so 501.8 million are redundant. The operand-reuse
probes say the same from the other side: the second operand's
`(shape, translate)` is found in an 8192-entry direct-mapped cache on
99.7% of calls, because it is a *fixed* neighbour and there are only
sixty-one of them, while the whole-call argument tuple hits on 2.0%. So
the pair result is not memoisable and the operand row is.

### Removing the redundant additions buys nothing

The hoist is arithmetically free - the same two additions produce the
same two values, once instead of `n` times - and it was measured before
it was believed. It was also *built*: an experiment binary that hoists
the row for any operand with at most sixteen poles reproduces all four
regression gates as whole documents, so what follows compares two
engines that are known to compute the same thing.

On a faithful reconstruction of the loop, at 21.69 pairs per call over
pole-count and separation distributions calibrated to the counts above,
best of fifteen interleaved rounds, with every variant asserted to
produce a bit-identical sum:

| variant | ns/pair | ratio |
|---|---:|---:|
| current, centre re-derived per pair | 8.985 | 1.0000 |
| hoisted into a zero-initialised row | 9.271 | 1.0318 |
| hoisted into an uninitialised row | 9.166 | 1.0201 |
| hoisted, with `sqrt(x*x + y*y)` | 2.949 | 0.3283 |

The hoist loses by 2.0% even when the row costs nothing to create. At
4.63 inner iterations the buffer is a dependency the additions were
not: the adds issue into slack the `hypot` call leaves behind, while
the stores and reloads are real traffic on a loop that runs four times.
The measurement that makes this legible is on the same real inputs: of
the 8.58 ns a pair costs, 7.49 ns is `hypot` alone and 2.80 ns is
everything else, and the two overlap. Arithmetic the machine was
already hiding is not arithmetic worth removing - the same lesson PR5
learned from its `Arc` handle, in a different register.

Two *extra* additions in the same body cost 4.5%, so the effect is real
and the sign is simply the other way: the loop is short enough that
hoisting costs more than it saves.

End to end, the engine agrees by declining to notice. Fifteen
interleaved rounds against the gate-bit-identical experiment binary,
arms alternating order, per-round paired ratios:

| stream | base | hoisted | paired median | spread | rounds below 1.0 |
|---|---:|---:|---:|---|---:|
| mode 22 | 3.167 s | 3.157 s | 1.001 | 0.967-1.053 | 7/15 |
| mode 0 | 1.562 s | 1.562 s | 1.004 | 0.984-1.030 | 5/15 |

Half a billion additions removed from a mode-22 stream, and the paired
median moves by one part in a thousand in the wrong direction. So the
change is not taken. The honest statement of the result is that the
loop's arithmetic is not its cost, and a count of removed operations was
never going to establish that it was.

### `hypot` is the loop, and it is already the right `hypot`

Sampling one in 128 of the mode-22 gate stream's pole pairs gives two
million real arguments, and on them:

| what | ns/pair |
|---|---:|
| whole inner body, platform `hypot` | 8.58 |
| whole inner body, `sqrt(x*x + y*y)` | 3.47 |
| `hypot` alone | 7.49 |
| `sqrt(x*x + y*y)` alone | 1.93 |
| everything except the length | 2.80 |

`pairPressure` costs 168.9 ns per call in the profiled mode-22 gate
stream (2893.88 ms over 17,139,907 calls, 21.16% of leaf), and 19.27
pole pairs at 8.58 ns is 165.3 ns of it. The span *is* the loop, to
within 2%, and the loop is `hypot` to within 87%.

So the question is whether the platform call can be beaten without
changing an answer, and that turns on whether it is correctly rounded.
It is, and this is now checked rather than assumed: for 200,000 of the
sampled arguments the exact value of `dx^2 + dy^2` was formed as a
rational and compared against the midpoints bounding each candidate's
rounding interval, in exact arithmetic. glibc 2.42's `hypot` is the
correctly rounded result on **0 failures out of 200,000**. The naive
form is wrong on 33,310 of them - 16.655%, always by exactly one ulp,
which is the 16.7% PR5 measured, now attributed to a specification
rather than to a coincidence.

Correct rounding is a *specification*, so "bit-identical" and "correctly
rounded" are the same requirement here, and the platform already meets
it. There is no faster answer to be had by being cleverer about the
answer; only by being cleverer about reaching it.

### What is left, sized

There is exactly one bit-identical route left and it is a
numerical-methods change, not a loop change: a fast path that computes
the length in double-double with one Newton correction, emits a
certificate that its result is the correctly rounded one, and falls back
to the platform call when the certificate fails. Sized on the same two
million real arguments:

| length | ns/call | ratio | disagrees with platform |
|---|---:|---:|---:|
| platform `hypot` | 8.04 | 1.000 | - |
| double-double + Newton, FMA enabled | 3.08 | 0.383 | 0.558% |
| double-double + Newton, baseline target | 5.85 | 0.726 | 0.558% |
| `sqrt(x*x + y*y)` | 1.82 | 0.226 | 16.729% |

Two things that table says plainly. The prize is real - 2.6x on the call
that is 87% of the loop that is 21% of mode-22 leaf - and it is entirely
conditional on FMA, which the release profile does not enable, so it
needs a runtime-dispatched `target_feature` path rather than a build
flag. And the probe is a *sizing* probe, not a candidate: it disagrees
with the platform on 0.558% of real inputs, which is exactly the
population a certificate has to catch and hand to the fallback.
Shipping it without a soundness argument for that certificate would be
shipping a 0.558% chance of a different search, which is the mistake the
next section is about.

## The proxy tier's fast length changes outcomes, and the corpus says so

`fast-proxy-hypot` shipped off by default with an honest caveat - "five
agreeing streams is evidence and not a guarantee" - and an explicit
instruction to evaluate it on a corpus rather than on a fixture.
Evaluated on a corpus, it fails, and it fails in the specific way the
caveat predicted.

### The control first

Every comparison below is against a same-binary control, because "the
trajectories differ" is only a statement about the flag if the engine is
deterministic. It is: the base binary run twice over the eight corpus
streams is identical field for field, and run twice over the fourteen
Mixed-61 arms is identical field for field. 22 of 22 controls clean, so
every difference reported below is attributable to the flag.

### What diverges

Two corpora, compared as whole documents rather than as published
fields:

| streams | published outcome identical | whole document identical |
|---|---:|---:|
| shapes-17 / triangle-20, mode 20, 4 fixtures x 2 seeds | 8/8 | 2/8 |
| Mixed-61: 4 gates, 4 mode-31 arms, 6 mode-26 ladder arms | 14/14 | **0/14** |

The published outcome reproduces everywhere. Mode 20 replays at
`independentDepthMm` 206.869 and fingerprint `8a7737381238fa4d`, the
three mode-22 records at 159.09233022733062, 159.07876040364795 and
164.0375677990678 at `fa01012af1d559ae`, `e28fba007f8031d4` and
`49f094d7e59a9008`, every arm `exactValid` and `contractValid`, the
mode-26 ladder and the mode-31 arms agreeing field for field including
failure-reason text. That is the evidence PR5 had, reproduced and
extended to a fourth gate and to a second family of requests.

It is also not the whole document. Under the flag the relaxed search
takes a different path on 20 of the 22 streams: last-place differences
in `rawPenalty`, `weightedPressure` and `weightedLoss` propagate into
different accepted moves (32,317 against 32,288 on one shapes-17
stream), different rotation and translation evaluation counts, and
different probe totals. The mechanism is exactly the documented one - a
last-place difference in a ranking signal moves a tie-break, a moved
tie-break is a different accepted move, and from there the two searches
are different searches.

### And on three arms it changes a reported layout

On the three mode-26 arms at relaxed seed 1 the divergence is not
confined to counters. The coupled dynamic separator's boundary
projection treatment lands somewhere else:

| | flag off | flag on |
|---|---:|---:|
| `boundaryProjectionTreatment.finalDepthMm` | 179.810 | **179.931** |
| `coupledTreatmentIndependentUsedLongAxisDepthMm` | 179.809 | **179.930** |
| `finalPlacementFingerprint` | `49516ab3d5f7013d` | `ea7e63871babd135` |

A different layout, 0.121 mm deeper, on a reported depth field. The
mode-26 publication above it still reproduces the incumbent, because a
ladder arm replays a pinned parent and the parent is not what moved -
but the engine's own from-scratch treatment in the same process did
move, and it moved the wrong way. One arm is not a quality claim; it is
a disproof of the claim that the flag is outcome-neutral, and that claim
was the only thing keeping promotion open.

### Verdict, and what would reopen it

**Do not promote.** The default stays `hypot`, and the recommendation is
stronger than "not yet": the evidence that would have justified
promotion has now been gathered and it points the other way.

The end-to-end win is real and was measured the same way every other
number in this file is - ten interleaved rounds, arms alternating order,
the statistic the per-round paired ratio:

| stream | flag off | flag on | paired median | spread | rounds below 1.0 |
|---|---:|---:|---:|---|---:|
| mode 22 | 3.427 s | 3.095 s | **0.900** | 0.767-0.934 | 10/10 |
| mode 0 | 1.653 s | 1.553 s | **0.913** | 0.869-1.001 | 9/10 |

Ten percent, consistently. It is not worth a 0.121 mm regression on a
reported depth, and it is certainly not worth a proxy tier whose answers
are a function of which machine's `libm` compiled it.

The flag should stay in the tree, off, for one reason: it is the
cheapest available *measurement* of what the proxy tier's length costs,
and this stage used it as exactly that. It should not acquire a default,
a settings knob, or a coordinator that can reach it.

What would reopen the question is not more streams of the same kind. It
is a different argument: a fast length that is *certified* correctly
rounded, per the sizing table above, so that the tie-break it feeds is
the same tie-break. Such a change carries no corpus burden at all,
because it does not change an answer - it only has to prove that it does
not, and the four gates plus the shadow-rescore audit already exist to
check that. Corpus evidence is the wrong instrument for a bit-identical
change and an insufficient one for a non-bit-identical one; that is the
general lesson here, and it is worth more than the flag was.

Evidence: `docs/experiments/fast-proxy-hypot-corpus-evidence.json`.

### What this stage shipped

No engine behaviour. Every `.rs` line this stage changed is a doc
comment, and the worktree binary reproduces the base binary as a whole
document on all four gates, the four mode-31 arms and the six mode-26
ladder arms - 14 of 14, which is the same comparison that finds 14 of 14
*differences* under the flag. That is the point of the entry: two
plausible optimisations were built and measured, one is arithmetically
impossible and one is empirically worthless, and the third was already
in the tree and has now been disqualified rather than deferred. The
cheapest thing an engine can be given is a reason not to change it.

## The quality curve exists now, and it is over in 1.4 seconds

The third review's priority 0 was a *measurement*, not an optimisation:
"build one event-driven, one-process, from-request-only quality frontier
trace before doing more optimization", because "current evidence gives
whole-arm totals from selected parents, not time-to-first-value, marginal
delta-mm/ms, or the cost of the actual from-scratch ancestry inside one
process. Until that curve exists, a ten-second portfolio allocation is
informed engineering - but still storytelling."

The curve exists. It is `docs/experiments/quality-frontier-trace/`, and
what it says is not what the ten-second schedule assumed.

### One choke point, not fifty-five

The trace's unit is one **exact-valid candidate** - every layout the
search proves legal, published or not - and there is exactly one place
in this engine where that event happens: `validate_and_measure_placements`,
the composite exact validator. Every mode's acceptance, every lane's
publication check, every deep-operator dual audit passes through it. So
the instrument is one `#[cfg]` block at one line, plus operator *scopes*
that name the work an event belongs to, plus disposition events at the
three sites that decide one. Fifty-five call sites needed no edits.

The scopes are the second half of the design. A thread-local stack of
`(operator, seed, parent fingerprint)` frames is pushed by the
constructor, by each relaxed epoch, by each coupled arm, by the
persistent-vacancy mode dispatch, and by each of mode 20's eight
construction restarts; every event carries the innermost frame and a
snapshot of the `profiling` counters, so the difference between a
scope's enter and exit *is* its work attribution. On the mode-20 stream
the leaf scopes account for 26.851 of the mode-20 run's 26.882
seconds.

`quality-trace` is off by default and empty when off - the `profiling::deep`
pattern, for the same reason. Ten interleaved rounds on the mode-22 gate
stream, arms alternating order, per-round paired ratio: **1.014**, spread
0.980-1.044, four of ten rounds below parity, every outcome identical. An
independent earlier ten-round set of the same pair read 0.995. Compiling it
in is not distinguishable from not compiling it in.

### What it costs when it is on, and what that cost actually is

Arming the sink is not free, and the decomposition is the interesting
part. Same A/B protocol, same stream:

| comparison | paired median | spread | rounds below 1.0 | outcomes identical |
|---|---:|---|---:|:--:|
| base -> feature in, sink closed | **1.014** | 0.980-1.044 | 4/10 | yes |
| sink closed -> sink open | 1.165 | 1.157-1.184 | 0/10 | yes |
| sink closed -> counters only, no sink | 1.168 | 1.156-1.195 | 0/10 | yes |

The third row is the whole of the second. The trace's own work -
formatting a JSON line and writing it into a 1 MiB buffer, a few dozen
times per run - is not separable from noise once the *counters* are
armed; what costs 16.8% is `profiling::set_enabled(true)`, i.e. one
thread-local add on each of 10.9M candidate queries. So the trace ships
with `POLYGON_NESTING_QUALITY_TRACE_COUNTERS=0`, which leaves the
counters alone and gives a run the clock the production build runs on
with zero work ordinals. Every run header says which it was. A
time-to-quality curve is drawn on the undistorted clock; a work
attribution is read off the other run; no single artifact claims both.

### The curve

Four processes, Mixed-61 exact-clearance request, **from request only** -
no pinned parent, no warm start, production default allowance, seeds 0
and 1. Undistorted clock. Depth is raw source depth, joined to the
public incumbent by placement fingerprint.

| | m0+coupled s0 | m0+coupled s1 | mode 20 s0 | mode 20 s1 |
|---|---:|---:|---:|---:|
| first complete exact-valid layout | 0.535 s @ 231.570 | 0.546 s @ 231.570 | 0.541 s @ 231.570 | 0.546 s @ 231.570 |
| <= 200 / 190 / 185 mm | 0.666 s @ 182.976 | 0.672 s @ 182.976 | 0.669 s @ 182.976 | 0.655 s @ 182.976 |
| <= 182 mm | 0.980 s | 0.917 s | 0.977 s | 0.883 s |
| <= 181.6 mm | 1.249 s | 1.148 s | 1.254 s | 1.121 s |
| <= 180 mm | never | 1.350 s | never | 1.326 s |
| final engine depth | 181.589 | 179.690 | 181.589 | 179.690 |
| run end | 1.956 s | 1.981 s | 26.617 s | 26.960 s |
| tail with zero incumbent gain | 0.707 s | 0.630 s | **25.363 s** | **25.635 s** |

Read the first two rows together: the constructor produces a complete,
exact-valid, contract-valid 231.570 mm layout in **0.535-0.546 seconds**,
and its own portfolio has already reached 182.976 mm at 0.655-0.672 seconds - so
three of the five depth milestones the review named are cleared before
the relaxed loop's first accepted move. The relaxed loop then spends
5.3M candidate queries and 48.2K effective moves to buy 1.387 mm (seed
0) or 3.286 mm (seed 1), and it is finished by 1.35 seconds.

Marginal delta-mm per second, first incumbent to last, then over the
whole run:

| run | gain | window | inside window | over whole run |
|---|---:|---:|---:|---:|
| m0+coupled s0 | 1.387 mm | 0.583 s | 2.378 mm/s | 0.709 mm/s |
| m0+coupled s1 | 3.286 mm | 0.678 s | 4.845 mm/s | 1.659 mm/s |
| mode 20 s0 | 1.387 mm | 0.585 s | 2.372 mm/s | **0.052 mm/s** |
| mode 20 s1 | 3.286 mm | 0.671 s | 4.897 mm/s | **0.122 mm/s** |

The mode-20 rows are the *same search* as the m0 rows - the two runs
agree counter for counter through the m0 phase, 5,851,533 candidate
queries and 52,313 effective moves on seed 0 - plus a 25-second tail
whose marginal contribution to the published result is 0.000 mm.

### Where the work went, and the number that reorders the board

Per-scope, mode-20 seed 0, work-ordinal run:

| scope | wall | candidate queries | effective moves | exact pair tests | collision builds | exact-valid candidates |
|---|---:|---:|---:|---:|---:|---:|
| `constructor` | 0.648 s | 0 | 0 | **584,671** | 2,913 | 1 |
| 16x `m0.epoch*` | 1.193 s | 5,332,423 | 48,153 | 207 | 0 | 3 |
| 3x `coupled.*` | 0.334 s | 519,110 | 4,160 | 63 | 0 | 0 |
| 8x `mode20.restart*` | 24.676 s | 0 | 0 | 458 (non-deep only) | 0 | 8 |

**The short-side-first constructor performs 584,671 of the run's 585,460
exact Clipper pair tests - 99.87% - and all 2,913 collision polygon
builds, inside its first 0.648 seconds.** The review's iteration target
says "keep all optimizer-internal exact geometry below roughly 5% of the
ten-second budget"; on this stream the optimizer's exact geometry is
already 0.13% of the run's, and the constructor's is everything. That is
a different target than the one the board was written against.

Mode 20's eight restarts cost 3.0-3.2 s each, 24.676 s together, and produce exactly one
exact-valid complete layout each: 204.070-217.202 mm on seed 0,
204.272-228.112 mm on seed 1. Every one is deeper than the incumbent it
was built alongside, and the adoption rule refuses all eight - now with
a named reason, `notStrictlyBetterThanLegacy`, rather than a silent
return of legacy.

This is the review's own finding measured rather than argued: mode 20
"is required in mechanism" and its "worse immediate depth must not
disqualify its basin", but on this evidence the mechanism costs 93% of a
27-second process and returns, as a published result, nothing. Whether
those eight basins are worth their 24.7 seconds is a question about
their *descendants*, and that is the second plot the review asked for -
"structurally diverse archived basins versus time, with their eventual
descendant depth under a fixed downstream work budget". This trace
supplies its input (eight fingerprinted, depth-measured basins per seed,
with creation timestamps) and deliberately not its answer.

### Two things that had to be built to measure anything at all

**Mode 20 had no from-request path.** `run_population` refuses an
unpinned parent for the whole 9-21/25 band before it does any work, so
no single process could measure a from-request mode-20 basin - the exact
slice the ten-second schedule allocates 1.9-4.0 s to. It is now
reachable behind `persistent_vacancy_allow_unpinned_parent`, off by
default, reported in the result document when armed
(`unpinnedVacancyParent: true`), and explicitly not quotable against any
pinned number: a fixture carries a frozen fingerprint and depth that the
arm re-derives on load, and an in-process parent carries neither.

**Adoption refusals are named.** The review's second finding was that
"every adoption rejection silently returns legacy; production telemetry
cannot distinguish incomplete, invalid, envelope-only rejection, or
non-improvement". Under the trace they are four distinct `publication`
events: `incompleteCardinality`, `publishedDepthUnmeasurable`,
`notStrictlyBetterThanLegacy`, `compositeValidatorRejected`.

The mode-20 construction clamp this stage runs under is derived rather
than pinned - twice the request's own area lower-bound depth, 130.399 mm
-> 260.797 mm - per the review's instruction that scale-dependent
thresholds come from geometry. No Mixed-61 constant enters the driver.

### Evidence

All four pinned regression gates reproduce on the worktree binary and
again on the `quality-trace` binary **with the sink armed on every
gate**, which is the harder half: recording the stream does not change
the stream. Mode 20 at `independentDepthMm` 206.869 /
`8a7737381238fa4d`, the mode-22 records at 159.09233022733062 /
`fa01012af1d559ae`, 159.07876040364795 / `e28fba007f8031d4`, and
164.0375677990678 / `49f094d7e59a9008`, every arm `exactValid` and
`contractValid`. Artifacts, drivers and raw event streams:
`docs/experiments/quality-frontier-trace/`.

## The constructor's void raster was 66.6% of mode 20, and the cell size it used is a lottery ticket

Sol's portfolio gives mode-20 basin generation a two-second slice and
prices the current implementation at 26.562 s, which needs roughly 13x.
It also names the centre: `vacancyProxyRank` rasterises the whole strip,
allocates three buffers, and runs a cell-by-cell point-in-polygon scan
against the active collisions, and the instruction is to replace it -
incremental occupancy, a bit-grid flood fill, reusable buffers,
scale-derived resolution - rather than tune its loops. This entry does
that, and the redesign turned out to be the smaller of its two findings.

### What the phase was actually doing, counted

A counting build of the legacy evaluator on the mode-20 gate-1 stream:

| quantity | mode 20 gate |
|---|---:|
| `trapped_void_cells` calls | 11,281 |
| grid columns (fixed 2.0 mm cell, 2000 mm strip) | 1,000 |
| grid rows | 58-105 |
| cells rasterised per call, mean | 83,613 |
| active collisions per call, mean | 31.45 |
| collision vertices per call, mean | 212.2 |
| cells inside some collision's bounding box, mean | 65,896 |

Profiled, the phase is 20,115.1 ms over those 11,281 calls - **1,783.10
us each and 66.60% of leaf time**, against Sol's 57.5%. The cost is in
the last row: the bounds prefilter still leaves about 65,896 cells per
call that have to ask Clipper a point-in-polygon question, and the whole
grid is rebuilt from nothing every time.

None of that work is necessary, and each of four changes removes a
different part of it:

* **Incremental occupancy.** A constructor child is its parent plus
  exactly one placed piece, and a piece's occupancy does not depend on
  the pieces around it, so a child's grid is its parent's grid with one
  raster OR-ed in. A 64-slot FIFO keyed by state identity retains the
  grids a rank's children produce, which is more than the next rank's
  six-or-seven parents need, so a slot expansion normally starts from a
  hit. The rasteriser sees one piece per call instead of 31.
* **Scanline rasterisation.** `O(rows x edges)` instead of
  `O(cells x edges)`, by even-odd fill between sorted crossings of the
  row's centre line. Spans are filled *closed*, and edges and vertices
  lying exactly on the scanline are filled directly, because the legacy
  rule is that Clipper's `IsOn` is occupied and this engine's
  axis-aligned parts at integral translations put edges exactly on cell
  centres constantly. Getting that rule wrong is worth about 3% of the
  count on a complete 61-piece layout - measured, because the first
  version got it wrong.
* **Bit-grid flood fill.** `u64` words, one word-aligned stride per row.
  Horizontal closure is a Kogge-Stone occluded fill with a cross-word
  carry, vertical propagation is one `free & reach` per word, and the
  per-cell `Vec<bool>` stack is gone. It agrees with a reference stack
  walk on a 200-case pseudorandom corpus, which is the unit test.
* **Scale-derived resolution**, below - the finding.

### What it is worth, measured

Ten interleaved rounds, arms alternating order every round, statistic
the per-round paired ratio, on a box shared with two other benchmarking
agents:

| stream | flag off | flag on | paired median | spread | rounds below 1.0 |
|---|---:|---:|---:|---|---:|
| mode 20 gate 1, engine clock | 26.245 s | 6.223 s | **0.2375** | 0.2361-0.2389 | 10/10 |
| mode 20 gate 1, process wall | 26.278 s | 6.255 s | **0.2384** | 0.2370-0.2399 | 10/10 |

**4.21x on the whole stream.** The phase itself:

| | flag off | flag on | ratio |
|---|---:|---:|---:|
| `vacancyProxyRank` | 20,115.1 ms | 225.9 ms | 0.0112 |
| ... per call | 1783.10 us | 20.03 us | **89.0x faster** |
| ... share of leaf | 66.60% | 2.24% | |

Both arms make the same 11,281 calls, and every other leaf phase's call
count is identical to the digit - `exactOverlapTest` 1,266,102,
`collisionPolygonBuild` 750,434, `pairCollide` 15,562,760 - because the
two arms are the same search. Those untouched phases drift 0.976x
between the runs, so the normalised phase ratio is 0.0115.

The honest multiplier against Sol's 13x is therefore **4.21x, and that
is 96.6% of the entire headroom this phase can ever offer**: with
`vacancyProxyRank` at exactly zero the stream would run 6.02 s, a 4.36x
ceiling. The remaining 3.1x is not in this phase and never was. It is
now `exactOverlapTest` at 33.1% of leaf plus `collisionPolygonBuild` at
20.1% - the constructor's exact confirmation, 1.27M Clipper queries and
750K offset builds - which is precisely PR6's port. Mode 20 goes from
26.2 s to 6.2 s; a two-second slice needs that port.

### The equivalence evidence

The profile is opt-in and its contract is per-seed determinism plus
unchanged exact-valid publication, not bit-identity. It delivered more
than its contract:

* **Flag off**, all four regression gates reproduce the pristine
  `c9bfbd8` binary as **whole documents** - 206.869 at
  `8a7737381238fa4d`, 159.09233022733062 at `fa01012af1d559ae`,
  159.07876040364795 at `e28fba007f8031d4`, 164.0375677990678 at
  `49f094d7e59a9008` - every counter, every restart row, every
  diagnostic field.
* **Flag on**, all four gates reproduce the same documents, with one
  field different: `work.totalRetainedPeakBytes` rises from 468,898 to
  1,977,858, which is the evaluator's own grid cache being honestly
  charged against the 64 MiB ceiling it now shares.
* Two flag-on runs of the mode-20 stream are identical field for field
  apart from the elapsed clock.

So the quality gate - descendant depth under a fixed downstream work
budget, mode-20 endpoints for CLI seeds 0-3 given the identical short
descent (mode 22, relaxed seeds 0 and 1, target endpoint + 0.8 mm) by
the *default* binary so that only the parent differs - passes with a
paired delta of exactly zero on all eight pairs, at identical descendant
fingerprints. One caveat is worth recording rather than hiding: the
mode-20 endpoint is invariant to the relaxed-seed argument, because
`construction_seed` derives from the anchor fixture, the seed domain and
the target depth. Seeds 0-3 are four replicas, not four samples. That
was verified, not assumed.

### The finding: the cell size is a lottery ticket

The first version of this evaluator derived its cell as one sixteenth of
the narrowest piece - 1.875 mm on Mixed-61 against the shipped 2.0 mm -
and it **failed the quality gate by 7.44 and 8.62 mm**. That failure is
the useful part of this entry, because chasing it produced a fact about
the mechanism rather than about the rewrite.

First, the rewrite was cleared. Run at a *matched* cell size, the
bit-grid evaluator and the legacy raster produce the same constructor
endpoint fingerprint and the same descended depths, at every one of five
sizes:

| cell | endpoint | fingerprint (both) | descend seed 0 | descend seed 1 |
|---:|---:|---|---:|---:|
| 1.875 mm | 205.096 | `c0174b0ce84667c3` | 180.491 | 179.272 |
| 1.900 mm | 204.519 | `dbfcc7135049fbc8` | 180.632 | 179.486 |
| 2.000 mm | 206.869 | `8a7737381238fa4d` | 173.047 | 170.648 |
| 2.100 mm | 208.994 | `fb5436824e73e9c4` | 179.409 | 181.465 |
| 2.500 mm | 204.914 | `73a7daf0bbae66b7` | 179.003 | 179.003 |

The rewrite is not the variable. The cell size is - and the redesign is
what made asking affordable, at 6.3 s an arm instead of 26.5 s. Eighteen
cell sizes from 1.2 mm to 5.0 mm, every endpoint given the identical
descent:

| best of two descents | cell sizes |
|---|---|
| 169.5-174.3 mm | 1.4, 1.5, 1.6, **2.0**, 2.4, 5.0 |
| 179.0-180.6 mm | 1.2, 1.7, 1.8, 1.875, 1.9, 1.95, 2.05, 2.1, 2.2, 2.5, 3.0, 3.75 |

Eighteen distinct endpoint fingerprints; twelve land on a 179-181 mm
plateau and six find a basin; the shipped 2.0 mm is the *second*-luckiest
ticket at 170.648 while 1.4 mm reaches 169.501; and its two immediate
neighbours, 1.95 mm and 2.05 mm, both land on the plateau. There is no
region of good cell sizes. There is a coin.

The same table prices Sol's other rule. Immediate constructor depth
ranges over 202.615-208.994 across the eighteen arms, and its Pearson
correlation with the descended depth is **-0.212**: the *deepest*
immediate result, 2.0 mm's 206.869, produces the second-best descendant,
while the two shallowest, 202.994 at 1.95 and 2.05 mm, produce 179.6 and
180.0. "Worse constructor basins produce better descendants" is no
longer a remembered anecdote in this ledger; it is a measured
anti-correlation on eighteen paired samples.

### What shipped, and the calibration

Given that, `VOID_CELLS_PER_MIN_PIECE_EXTENT` is **15**, and the
documentation says plainly that this is a calibration and not an
optimisation. What the dimensionless divisor buys is scale covariance -
scale a request by `k` and every cell, count and ranking is unchanged,
which a fixed 2 mm cell cannot claim, being a fifteenth of a 30 mm part
and a thousandth of a 2 m one. What the *value* 15 buys is that the
derived cell is exactly the shipped 2.0 mm on the one stream whose
quality is pinned, so this profile's first delivery is a pure speed
change with an unchanged endpoint. The sweep is the argument that this
costs nothing: there is no better value to have chosen. A grid budget
coarsens the cell, and only coarsens it, when the derived resolution
would exceed 2^21 cells.

### What is left

* The 169.5 mm basin at 1.4 mm is not a tuning opportunity - it is one
  draw of the same coin, and treating it as a discovery would be exactly
  the error this entry documents. What it *is* is an argument for the
  archive: a coordinator that can afford several constructor arms at
  6.2 s each should draw several tickets and keep the structurally
  distinct ones, which is the `SearchArchive` Sol's review 3 asks for.
* Mode 17's vacancy-transport call sites still use the legacy evaluator.
  They are not on an incremental lineage - that mode ejects pieces as
  well as inserting them - so they need the full-rebuild entry point,
  and they are not on the measured critical path.
* PR6's Clipper port owns the rest of mode 20. After this stage the
  constructor's exact confirmation is 53% of its leaf time and 100% of
  the remaining gap to a two-second slice.

Evidence: `docs/experiments/fast-constructor-profile-evidence.json`.
