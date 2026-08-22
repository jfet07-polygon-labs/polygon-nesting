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

> **SUPERSEDED, 2026-08-22.** Every number in the paragraph above belongs to
> the retired 5.5 mm pair / 5.25 mm boundary contract. Under this branch's
> exact-clearance 5.0/5.0 contract the same construction gives
> **130.19990218310795 mm** strengthened and 125.19990218310794 mm plain, and
> **130.2140326353513 mm** for the composite envelope the engine actually
> publishes under. The 7.09 mm of "contract overhead" is gone with the
> contract that caused it - Sparrow's separation and this branch's are now the
> same 5.0 mm, so there is one bound and not two, and the residual asymmetry at
> the bound level is the 0.0141 mm the search allowance adds. Re-pin, derivation
> and the identity check against the retired file (the r = 2.5 inflated area
> agrees to 0.0 mm^2, which is what says only the constants moved):
> `docs/experiments/depth-lower-bound/depth-lower-bound-exact-clearance-evidence.json`.

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

## Seam v2: the exact tier leaves the trait, the fused pair query is priced at zero, and the moved row gets a name

Sol's fourth finding is that the kernel seam is not the production seam,
and it lists seven defects. This stage takes the three that are seam
shape rather than backend work - the trait declaring both tiers, the
split pair query, and the untyped tracker delta - and leaves the four
that are not. It is a pure-seam stage: every gate and every arm is
bit-identical to the base commit, on both arms of the one flag it adds,
and the one thing measured came back zero.

### The exact tier is off the generic parameter, mechanically

`ExplorationKernel` declared both tiers, and the convention that kept
exact answers off a generic parameter was "exact call sites name
`LEGACY`". A convention is not a seam: nothing stopped a future generic
function from writing `K::exact_pair_overlaps` and routing a
publication-authority answer through whatever kernel had been
substituted.

The trait is now the proxy tier and nothing else. The `f64` Clipper
collision-polygon build and pair-overlap verdict moved to
`search::kernel::exact`, a module with no type parameter anywhere in it,
behind an `ExactAuthority` token whose one field has a type private to
that module - so the token cannot be constructed outside it, only
received. Both service functions are private there, so the token is the
only door, and the crate's one grant is `LegacyKernel::exact_authority`,
an *inherent* method on the concrete kernel.

That turns the misuse into a compile error. A function generic over
`K: ExplorationKernel` has no `K::collision_polygon` and no
`K::exact_pair_overlaps` to call; it cannot reach the grant, because `K`
is not `LegacyKernel`; and it cannot forge a token to hand to a helper.
Grepping `exact_authority` now enumerates every exact call site in the
crate - two in `general_fast`, one in the deep-operator constructor, one
in the jagua shape conversion. `JaguaKernel` loses its exact tier
entirely rather than forwarding it, so the refusal to put `f32`, a
tolerance, or jagua into publication authority stopped being a runtime
property checked by asking two kernels the same question and became a
type property.

The honest limit of the mechanism, stated because the previous version of
this claim overreached: `ExactAuthority::grant` is `pub(super)`, so any
module *inside* `kernel/` could mint one. What it buys is that no code
outside that directory can, and that the one grant handed to the rest of
the crate is a named legacy service.

### The fused pair query: built, measured, declined as a default

The split pair query is a real defect of the seam. Every call site that
matters asks both questions about the same two operands, so two trait
entries make every caller present the operands twice and forbid a kernel
whose two answers share a traversal from exploiting that.
`ExplorationKernel::pair_row` is the fused entry and it is
unconditional: it returns a `PairRow` carrying verdict and magnitude,
and its provided body is today's split in today's order, so a kernel
that cannot share a traversal inherits today's arithmetic exactly.

Whether the *lane* should call it is a different question, and it is a
measurement, so it shipped as one. `fused-pair-query` selects between
two bodies of `resolved_pair_row` that compute the same `f64` from the
same operands in the same order. Ten-round interleaved A/Bs, arms
alternating order every round, statistic the per-round paired ratio
fused/split, two independent samples:

| stream | sample 1 | rounds below 1.0 | sample 2 | rounds below 1.0 |
|---|---:|---:|---:|---:|
| mode 22 | 1.007 | 2/10 | **0.998** | 7/10 |
| mode 0 | 0.994 | 7/10 | **0.999** | 6/10 |

Every median is within 0.7% of parity, every one sits inside its own
sample's spread, and mode 22 changes sign between the samples. That is
what a real zero looks like, and it is the reason two samples were taken:
sample 1 alone would have been written up as a 0.7% regression and
sample 2 alone as a 0.2% win, and neither statement would have been
true.

The result is the one the structure predicts. The legacy kernel's
verdict walks a cell index and its magnitude walks a pole series - two
disjoint structures built at catalogue time, with nothing between them
to share - so fusing the call saves one operand presentation and one
index-ordered swap, and PR5 already established that the arithmetic
around this loop is not its cost. **The default stays split.** The flag
stays in the tree, off, as the instrument that priced the entry; the
entry stays unconditional as the seam a sharing kernel needs.

A measured no is the delivery here, not a consolation for one. The thing
that would change the answer is not a better fusion but a kernel whose
two answers come from one traversal, and that kernel is exactly what the
entry now makes expressible.

### The moved row has a name and a completeness

`PlacementScore` named neither of the two things that matter about it.
It is `MovedRowDelta`: the delta for one moved piece, which
`update_score_after_move` consumes and nothing else, which is what makes
an accepted move cost the moved row rather than the layout.

Two contracts now live on the type rather than being rediscovered at each
of its eleven producers. Rows are keyed by the index-ordered pair, per
the row-ownership decision - and the sharp distinction the doc now draws
is that the row *key* has always been index-ordered unconditionally,
while the order the operands are *asked* in is not, and only
`canonical-pair-order` makes the two agree. The type carries the free
half of that decision; the flag carries the half that costs a
trajectory.

And completeness is explicit, per the roadmap bullet "only a complete
result may produce a tracker delta". `MovedRows` distinguishes
`Complete`, `PrunedAtBound` and `Unscanned`, and the two incomplete cases
are deliberately not merged because the arguments that keep them out of
the tracker differ in strength. `PrunedAtBound` is excluded by an
ordering identity - the bound a scan prunes against is exactly the
`weighted_loss` of the candidate it must beat, and both comparators order
on `weighted_loss` first - so it is asserted. `Unscanned` is excluded
only when the incumbent it is compared against has a finite loss, which
is weaker, so it is documented and not asserted. A test constructs a
`PrunedAtBound` delta on purpose, so the assertion cannot pass because
the state is never reached.

The marker is free: it is written by every producer, read only by a
`debug_assert` that the release profile compiles out, and branched on
nowhere. A ten-round interleaved A/B of the mode-22 gate stream against
the previous commit measured a paired median of 0.992, spread
0.966-1.018.

### What this stage shipped, and what it did not

Zero engine behaviour. Both arms of the flag reproduce the base-commit
binary as whole documents on all four gates, the four mode-31 arms and
the six mode-26 ladder arms - 14 of 14 each, wall-clock and
build-identity fields removed - with mode 20 at `independentDepthMm`
206.869 and fingerprint `8a7737381238fa4d`, the three mode-22 records at
raw 159.09233022733062, 159.07876040364795 and 164.0375677990678 at
`fa01012af1d559ae`, `e28fba007f8031d4` and `49f094d7e59a9008`, every arm
`exactValid` and `contractValid`, and failure-reason text agreeing field
for field. Full release suite green at 1222 tests, plus the 132
`general_relaxed` tests re-run in a debug build so the new
`debug_assert` is live rather than compiled out.

Four of Sol's seven bullets are untouched and stay open, because they are
backend work rather than seam shape:

* `LaneSearch` still fixes `K::Shape = OrientedSurrogate`, so
  `JaguaKernel` still cannot be substituted into it. That needs the
  catalogue, the NFP builder and the pose-bounds helper moved behind
  `ExplorationKernel::Shape`.
* `PosedShape` still only translates a pre-oriented shape, so
  continuous-angle operators still require prebuilding every angle
  variant.
* The jagua skeleton still builds a `Layout`, container, placed item and
  moving scratch per pair question. It is a parity scaffold and is still
  not a candidate engine.
* `KernelProbes` are still backend-defined, so a jagua "one SAT" charge
  is still not economically comparable with the legacy collider's, and
  portfolio work quotas are still not backend-neutral under that
  definition.

Tracker inheritance remains blocked for the reason the row-ownership
chapter established rather than for a new one: it needs the magnitude
class at zero as well as the structural class, and the residue is the
`f32` dynamic-pole tier, which cannot express an index-ordered pair
question at its interface. `MovedRowDelta` is the type that inheritance
will hand around when that tier is fixed; it does not unblock it.

## PR7: the coordinator has two state objects, and the schedule it was given is not the schedule the measurements support

Sol's review 3 asks for a thin coordinator - "Sol's portfolio, sized by
the measured economics, as one single-process anytime driver" - and its
finding 2 says why the engine cannot get there with the state model it
has: adoption "only retains an immediately better public result", while
"the documented from-scratch lineage begins from precisely this class of
worse, structurally different constructor basin". This stage builds that
coordinator, runs the review's own ten-second schedule from the bare
request, and measures it against the m0+coupled baseline. It beats the
baseline on every paired round. It also disagrees with the schedule
about where three of the ten seconds should go, and the disagreement is
the more useful half.

### The two objects, and the one door

`search::portfolio` is the module. `PublishedIncumbent` is the engine's
answer - always dual-gate valid, best raw depth - and it moves through
exactly one function: `adopt_published_placements`, which is steps 2-4 of
the adoption rule extracted verbatim so that the coordinator and the
coupled separator's own mode slot publish through the *same* completeness
check, the *same* strict raw comparator and the *same* composite exact
validator. The coordinator detects adoption by the fingerprint moving,
because that is the only way that function can have said yes. It has no
validity opinion of its own and no way to acquire one.

`SearchArchive` is the search's memory: basins keyed by placement
fingerprint, each carrying raw depth, birth time in both seconds and work
units, operator provenance and parent fingerprint. Under capacity it
admits **everything**, including a basin deeper than every member,
because this ledger's own eighteen-sample sweep measured
Pearson(immediate, descended) = -0.212 and immediate depth is therefore
not evidence about future value. At capacity it evicts only a member that
is both *dominated by* and *similar to* some other layout - no deeper,
and piece-assignment overlap at or above the threshold - and a full
archive of mutually distinct basins refuses the newcomer rather than
dropping a distinct member. Structural distance is the fingerprint as the
cheap first cut and the fraction of pieces at an exactly identical pose
as the better one, with no tolerance in it: a tolerance would be a
length, and a length would have to come from somewhere.

### The number

Nine paired rounds, three seeds, arms interleaved with the order rotating
every round, from the bare request at a ten-second wall budget, on a box
shared with another benchmarking agent:

| seed | m0+coupled baseline | coordinator, review schedule | coordinator, focused |
|---:|---:|---:|---:|
| 0 | 181.589 | **179.587** | **179.587** |
| 1 | **179.690** | 176.753 (176.056/179.633/176.753) | **176.056** (3/3) |
| 2 | 179.662 | **179.006** | **179.006** |

**Paired delta median -2.002 mm, 9 of 9 rounds strictly better**, on both
coordinator arms. Against the bar - the baseline's 179.690 flatline - the
focused arm publishes **176.056 mm in all three rounds, a 3.634 mm
margin**. The stretch goal of 175 mm is not reached; the best layout any
arm published in ten seconds is 176.056.

The first 1.8 seconds of all three curves are the same curve, because
they are the same search: the coordinator's phase 0 *is* the protected
mode-0 run. Under the trace, seed 1's focused arm crosses 179 mm at
6.800 s and 177 mm at 7.526 s; the baseline crosses neither, ever.

### Where the ten seconds went, per operator

| operator | calls | mean wall | exact-valid | published |
|---|---:|---:|---:|---:|
| `basins/mode20` | 19 | 0.613 s | 19 | **0** |
| `descent/mode22` | 27 | 1.168 s | 27 | 9 |
| `crossover/mode23` | 4 | 2.716 s | 4 | 2 |
| `compression/mode31` | 6 | 0.096 s | **0** | 0 |
| `compression/mode22` | 6 | 1.036 s | 6 | 0 |

**The constructor slice does not pay at this budget.** The review gives
mode-20 basin generation 1.9-4.0 s and this stage gave it four salted
arms - salted on the derived cell divisor and on the clamp target, per
the cell-lottery finding, never tuned. Nineteen arms across nine runs,
every one exact-valid, every one refused by the adoption rule, and not
one descendant caught the incumbent inside ten seconds. At 0.613 s each
they are running at the `fast-constructor-profile` price, 4.2x cheaper
than the shipped evaluator, and it is still not cheap enough. The
`focused` arm prices the slice directly by setting it to zero: it is
never worse, and on seed 1 it is better and more consistent, because the
1.24 s goes to the crossover phase instead. That is a verdict on the
*allocation* at this budget, not on mode 20's mechanism, and the archive
is exactly the structure that will collect the payoff when the mechanism
gets cheaper.

**Mode 31 legalised nothing.** Six calls, zero exact-valid results, every
one "global legalization did not reach a feasible fixpoint". The review
already said m31 is production-worthy "only as the legalizer for a
compressed/perturbed frontier"; on a clean mode-22 fixpoint it has
nothing to legalise, and this is that sentence measured.

**Mode 23 is the second most productive operator here.** Two
publications in four calls under the review's schedule and three in nine
under the focused one, carrying the largest single gains in the run. The
review called crossover "conditional but currently evidence-required"; it
is now evidence-*producing*.

The archive ends at 8 of 16 under the review's schedule and 4-5 of 16
focused, so **its eviction rule never fired on this stream** - a caveat,
not a claim. One small finding fell out of it: the coupled separator's
control, treatment and boundary-projection arms are all offered and all
three come back `Duplicate`. The mode-0 result *is* the
boundary-projection arm; the separator's three arms are one layout.

### Two defects the coordinator's own ledger found, and one experiment it declined

**Charging a constructor's pose prior as a descent.** The frontier is
ordered partly by how often a basin has been descended from, and mode 20
does not descend from its parent - it builds from scratch and reads the
parent as a pose prior. Charging it a descent pushed the incumbent to the
back of a queue it should have led, and the first schedule spent its
whole alternation phase on 194-214 mm constructor basins while the one
parent whose quantum published waited. `ParentRole` is the fix.

**Ordering the frontier by fairness before quality.** The review's phrase
is "m22 work quanta across the **best** structurally distinct archive
states", and the word is load-bearing: sorting by descent count first is
fairer and measured worse. That schedule's nine rounds are retained -
also -2.002 mm median, but its best layout is 179.006 mm rather than
176.056 and seed 1 lands at 179.545-179.633.

**Iterated deepening of the alternation quantum**, built and declined.
When the frontier is a fixpoint at the current quantum size the obvious
move is to double the quantum rather than end the phase with budget left.
It keeps the phase busy by spending the *crossover* phase's budget, and
the crossover phase is the second most productive operator here: seed 1
goes from 176.056 to 179.633 under the review's schedule and to 176.753
under the focused one. `descent_iterated_deepening` stays in the tree,
off, as the instrument that priced it.

### Determinism is denominated in work, not seconds

A wall-clock schedule branches on a clock, so two of its runs are two
different searches on a shared box - which seed 1's own spread shows. The
work-budget mode branches only on the engine's counters: one unit is one
proxy candidate query and an exact Clipper pair test is charged five, a
ratio read off the quality-frontier trace's scope ledger (1.108 us
against 0.224 us). Two independent processes at a 40M-unit budget are
identical **as whole documents** - every phase boundary, every operator
call, every archive member, every work-unit reading - at depth 176.056
and 33,286,633 units spent, and so are two at a *binding* 20M budget,
where the schedule is genuinely different: `basins` is skipped because
mode 0 alone spends 9.63M past the 8M deadline, `descent` gets 4.12M
instead of 17.04M, `drain` is skipped, and both processes take that
different schedule identically.

One honest limit: the deep operators' Clipper counters are behind
`search-profiling`, which is off, so the `basins` phase charges 2,310
work units for 2.44 s of real work. The phase is still bounded, by its
slot count, so the schedule terminates - but the work currency is not yet
a faithful proxy for wall time across all operators, and nothing here
claims it is.

### What the coordinator costs on the work it did not add

Paired A/B on the phase both arms share: the plain engine run against the
same search as the coordinator's phase 0 with a zero budget, so every
later phase is entered, finds no room and is skipped. The difference is
five archive offers - each re-measuring a raw depth and re-running the
composite validator on a 61-piece layout - plus the incumbent's own
validation. Ten interleaved rounds per sample, arm order alternating,
statistic the per-round paired ratio: **1.020** (spread 0.701-1.131, 2/10
below parity) and **1.049** (spread 0.950-1.106, 3/10). Roughly 2-5%,
with both arms returning the identical engine depth 179.690. Two samples
for the reason the fused-pair-query entry gives; sample 1's 0.701 round
is another agent's benchmark landing on the box, not a coordinator
speedup.

### What this stage shipped

The default path is untouched. All four pinned regression gates reproduce
the pristine `0cf1163` binary as **whole documents** on the
default-features worktree binary - mode 20 at `independentDepthMm`
206.869 / `8a7737381238fa4d`, the three mode-22 records at raw
159.09233022733062, 159.07876040364795 and 164.0375677990678 at
`fa01012af1d559ae`, `e28fba007f8031d4` and `49f094d7e59a9008`, every
counter and every diagnostic field agreeing with wall-clock and
build-identity fields removed. The coordinator is reachable only through
a new trailing positional argument that every existing invocation leaves
empty, and the three new relaxed settings it needs - a constructor
restart window, a void-grid cell divisor salt, an alternation cycle cap -
are `None` everywhere else and can only ever *shorten* what the operator
they touch was already allowed to do.

Evidence, drivers and raw event streams:
`docs/experiments/pr7-portfolio-coordinator/`.

## Three quarters of the constructor's exact queries are real overlaps, and that is the ceiling

The bit-grid redesign left mode 20 at 6.2 s with its leaf dominated by the
exact confirmation *inside* construction - `exactOverlapTest` 33.1% plus
`collisionPolygonBuild` 20.1%, 1,266,102 overlap-test spans and 750,434
collision builds on the gate-1 stream. Sol's portfolio wants that
constructor at about two seconds. The obvious move is a proxy-first
prefilter: run a cheap verdict before the exact one and skip the exact
work when the cheap one says clean. This entry counts the stream before
building it, and the count decides the size of the prize before a line of
the prefilter exists.

### The census

`constructor-census` is a counting build in the sense the pole-loop and
collider stages use the term: opt-in, empty when off, and slow on
purpose. It attributes every confirmation row and every exact pair
question to its call path, and on every pair that reaches Clipper it
evaluates three separation tests beside the exact answer - the
axis-aligned box already in the code, a four-direction DOP (the box plus
the two diagonals), and a separating-axis test over both convex hulls.

All three run on the **integer Clipper path** the exact query is executed
on. That is the whole design: the collision polygon's `Path64` is on the
0.001 mm contractual grid, so its coordinates are integer-valued `f64`,
and every projection and cross product below stays inside the exactly
representable range under an explicit guard. Each test therefore answers
"provably separated" or "no information" - never "probably". A separated
pair has zero intersection area, so the verdict a skip substitutes is the
verdict the exact query would have returned.

The census checks that claim from the other side rather than asserting
it: it counts pairs the exact query calls *overlapping* that a cheap test
called separated. On 997,826 overlapping pairs, both counters are **0**.

### What the stream is made of

Mode-20 gate-1 stream, which reproduces 206.869 / `8a7737381238fa4d`
under the census:

| quantity | count | share |
|---|---:|---:|
| pair questions offered | 22,080,053 | |
| ... rejected by the existing box test | 20,789,701 | 94.16% |
| ... reaching Clipper | 1,290,352 | 5.84% |
| ... **genuinely overlapping** | 997,826 | **77.33%** |
| ... clean | 292,526 | 22.67% |
| clean, separated by the DOP | 136,671 | 46.72% of clean |
| clean, separated by the hull | 245,715 | **84.00% of clean** |

**The ceiling is 22.67%**, and it is a ceiling for *any* sound prefilter,
not for this one. Three quarters of the constructor's exact queries
answer "they overlap", and no outer approximation can ever remove one of
those - proving an overlap needs an inner certificate. The hull tier
reaches 19.04% of all Clipper queries, which is 84% of everything that is
reachable at all. That is the honest size of the proxy-first idea on this
operator, established before it was implemented rather than after.

The census reconciles with the profiler to the digit, and the two count
different things: `exactOverlapTest` opens one span per confirmation
*row* in the deep constructor and one per narrow-phase *query* in
`general_fast`, so its 1,266,102 spans are 680,602 deep rows past
containment plus 584,478 `general_fast` queries plus 1,022 elsewhere.
Inside them are 1,290,352 Clipper pair queries and 13.1M box rejects.

Two further facts the census produced by existing. The mean query is
**13.69 combined vertices** and the phase costs 3,309.6 ms over 1,290,352
of them - 2.6 us each - so `exactOverlapTest` is not geometry cost on a
fourteen-vertex problem; it is Clipper's per-call setup. And **78.66% of collision
polygon builds are wasted**: 590,299 of 750,434 are spent on a pose the
row discards two lines later, 66,919 for sheet containment and 523,375
for an overlap.

The three constructor call paths are not one population:

| site | rows | accepted | reaching Clipper | overlapping |
|---|---:|---:|---:|---:|
| candidate stream | 432,710 | 2.6% | 450,840 | 88.3% |
| contact drop ladder | 191,925 | 68.0% | 140,790 | 33.2% |
| slide bisection | 122,886 | 12.6% | 113,222 | 69.4% |
| `general_fast` short-side-first | - | - | 584,478 | 81.2% |

The candidate stream is speculative and the slide ladder is not, because
every rung of a ladder starts from an already-valid pose. "Reject the
speculative stream earlier" and "make the exact query cheaper" are
therefore different projects with different prizes, and only the second
is sound today.

### The prefilter, and what it is worth

`fast-constructor-confirm` stacks on `fast-constructor-profile` and is
`search::construction_confirm_shield`: DOP then hull, both proofs, the
parent's certificates derived once per beam slot and the row's once per
row into reused buffers. The reuse is not incidental - a first version
that allocated a hull per row measured 0.9494 where the final one
measures 0.9396.

Paired interleaved A/Bs of the mode-20 gate-1 stream against
`fast-constructor-profile` alone, arms alternating order every round,
statistic the per-round paired ratio, two independent samples:

| sample | rounds | flag off | flag on | paired median | spread | rounds below 1.0 |
|---|---:|---:|---:|---:|---|---:|
| 1 | 14 | 6.231 s | 5.858 s | **0.9396** | 0.9245-1.0340 | 13/14 |
| 2 | 10 | 6.264 s | 5.848 s | **0.9367** | 0.6793-1.1404 | 9/10 |

Profiled, the delta is entirely inside the phase the change is in:

| phase | flag off | flag on | calls |
|---|---:|---:|---:|
| `exactOverlapTest` | 3,309.6 ms (32.64%) | 2,926.2 ms (29.94%) | 1,266,102 both |
| `collisionPolygonBuild` | 2,016.5 ms | 2,015.5 ms | 750,434 both |
| `pairCollide` | 1,285.7 ms | 1,285.4 ms | 15,562,760 both |
| leaf total | 10,141.0 ms | 9,774.8 ms | |

**The honest multiplier is 4.48x**, against Sol's ~13x: 4.21x from the
bit-grid redesign times 1.064 from this. Mode 20 goes 26.2 s -> 6.2 s ->
5.86 s. A two-second slice still needs 2.9x that this stage does not
have, and the census says where it is not: not in the exact pair
question, three quarters of whose calls are load-bearing.

### Equivalence, and why the quality gate is trivially zero

Flag off, all four regression gates reproduce the pristine `0cf1163`
binary as **whole documents** - 3,271 and 3,252 compared leaf fields per
gate, six differing, all of them the executable hash and the five
wall-clock fields. Flag on, the gate-1 document is identical to the
`fast-constructor-profile` arm in every field but one:
`clipperInputVertices` falls 39,043,027 -> 37,012,470, which is the work
the prefilter removed being honestly not charged; gates 2, 3 and 4 are
identical in every field. Two flag-on runs are identical field for field
apart from the clock. A debug build with the `debug_assert` live - every
skipped pair handed to Clipper anyway and required to return zero area -
reproduces all four gates, and the assertion never fired.

The constructor-quality gate is four salts, and the salt is the **target
depth** rather than the relaxed-seed argument, per the caveat the
previous round recorded: `construction_seed` derives from the anchor, the
seed domain and the target, so seeds are replicas and targets are
samples. The four targets produce four different endpoints - 206.869,
206.666, 199.801, 214.042 - each pinned and given the identical short
mode-22 descent by the *default* binary at two relaxed seeds. All eight
paired descended-depth deltas are exactly 0.0, at identical endpoint and
descendant fingerprints.

That is the gate passing, but the interesting part is *why* it is a
tautology here. A sound prefilter cannot move a search: it removes
queries whose answers were never in doubt. The gate is worth running
precisely because it would catch the case where the soundness argument is
wrong, and it is worth saying plainly that a zero here is the predicted
result rather than a lucky one.

### What this entry leaves open, sized

* **The wasted build is the bigger prize and it is not sound yet.**
  78.66% of collision-polygon builds are discarded and
  `collisionPolygonBuild` is 20% of leaf. Removing one needs a
  certificate in the *opposite* direction - a proof that a pose **does**
  overlap - and the natural one is an inner circle cover of the
  unexpanded source, transformed rigidly per pose: two inscribed circles
  at centre distance below `r1 + r2 + 2 * expansion` prove the expanded
  polygons meet. Cheap, general, and resting on `offset_miter(P, e)`
  containing `P + disc(e)`, which is believable for Clipper's miter join
  and is not proved here. It must not ship on "believable"; the whole
  value of this stage is that its skips are proofs.
* **The residual `exactOverlapTest` is Clipper's per-call setup.**
  Fourteen vertices and 2.6 us. A pair query that reused one engine and
  one scratch path set across calls is the next measurement, and it is a
  Clipper-binding change rather than a search change.
* **`general_fast`'s constructor is untouched deliberately.** It carries
  584,478 of the 1,290,352 Clipper queries and 15.9% of them are
  hull-separable, but it is the protected legacy path and it runs on
  eight threads in about 0.65 s, so its share of the wall is small and
  its share of the risk is not.

Evidence, drivers and the full per-site census:
`docs/experiments/constructor-exact-census/`.

## Coordinator v2: the schedule's own verdicts were worth more than the coordinator, and its generality was a mixed-61 fact

PR7 delivered a coordinator that beat the baseline 9 of 9 and then wrote
down what its trace said was wrong with it. This stage acts on all of
it: the two recorded defects, the three measured verdicts on where the
budget went, the anytime curve nobody had drawn, and the one thing PR7
did not test - whether the schedule was general code or a description of
one request. It was a description of one request, in two places, and
both of them are now measured rather than assumed.

### The two recorded defects were already fixed, and there is a third

Both of PR7's defects are shipped at `8d9f7e5`. `ParentRole::Prior` is
in the basin phase and it is live - in a v2 triangle-20 run the
incumbent that eight mode-20 arms read as their pose prior ends at
`descents: 0` while the basins a quantum actually descended from carry
1, 2 and 4 - and `distinct_frontier` sorts `raw_depth_mm` first with
`descents` only as a tie-break, pinned by two unit tests.

The residual is in the other direction and this stage fixes it: **the
crossover's second parent was never charged a descent at all.** Mode 23
descends from both parents; v1 charged `frontier[0]` and passed
`frontier[1]` as a pinned secondary without telling the archive. Its
measured effect is nil, and the reason is the interesting part: once the
frontier is ordered by quality first, the descent counter only decides
exact depth ties between structurally distinct layouts, which no stream
here produced. Fixing the first defect made the second one nearly inert.
That is a correctness fix to an instrument, and it is reported as one
rather than as a win.

### The rebudget, and the one change that carried it

The schedule is reordered by measured productivity: alternation quanta
first (9 publications in 18 calls), then crossover (largest single
gains), then the compression micro-descent (3 in 9), then the
constructor slice (0 in 19), conditional and last.

**Crossover, made repeatable, is the whole gain.** v1 made one crossover
per run; the review's own schedule made one in *nine* runs, because the
constructor slice ahead of it had spent its deadline. Seed 0, one paired
round, from the operator ledger: v1's single crossover produces 179.639
and publishes nothing, and the run ends at 179.587; v2's second crossover
attempt produces **176.309** and publishes, and the compression quantum
then takes it to **174.208**. Ten seconds, three seeds, three rounds,
paired and interleaved, against the bare engine: **median -3.634 mm, min
-7.381 mm, 9 of 9 strictly better**, where v1's focused arm on the same
rounds is -2.002 mm. Against the v1 champion: **0 of 9 worse, 3 of 9
better, all by 5.379 mm**. 174.208 mm is a new best-from-request layout
at ten seconds, 1.848 mm below PR7's 176.056.

**The constructor slice is conditional, and the condition is priced
in-run.** Not a seconds threshold and not the stall test - the thing both
were proxies for: draw a basin only when the run can still afford to
*descend* from it, because a drawn-and-undescended basin is exactly the
19/19 refusal PR7 measured. The phase draws one salted arm and spends a
quantum on it in the same iteration, and refuses to start unless the
remaining budget covers `mean(mode20) + mean(mode22)`, both measured from
this run's own calls in the budget's own currency.

**And it stops when it stops paying.** `basin_patience`, default 1: the
phase ends after one iteration that publishes nothing, and the stopping
signal is the *descendant*, never the arm's own depth, because
Pearson(immediate, descended) = -0.212 makes immediate depth an invalid
proxy. At thirty seconds on mixed-61, three arms of a paired battery -
`never`, `patience=1`, `patience=8` - published **identical depths in all
27 rounds**, at 10.20 s, 12.57 s and 23.91 s of median process wall.
Patience 8 spends 13.7 s on 72 exact-valid constructor arms and 72
descents from them and changes nothing.

**Mode 31 is demoted to Sol's own trigger.** v1 asked it to legalize a
clean m22 fixpoint: 6 calls, 0 exact-valid. v2 compresses first and hands
m31 the residue only if the compressing descent returns a complete layout
the exact validator refuses. It was called **zero times** in 36 measured
runs across three requests, because there was never a residue. The
demotion is a measured no-op on quality that removes a call which has
never once succeeded.

Two further changes the measurements forced. "May I start?" became "can I
finish?": an operator call is refused unless the remaining budget covers
that operator's own measured mean cost, so a 2.7 s crossover can no
longer be launched 0.1 s before its deadline. And phase deadlines became
fractions of what phase 0 *left* rather than of the whole budget - see
below, because that one is a generality bug.

### The anytime curve, and where it saturates

Best published depth against wall budget, from the bare request, three
seeds, three rounds each, quality-trace armed with counters off:

| budget | seed 0 | seed 1 | seed 2 | vs bare engine |
|---|---:|---:|---:|---|
| bare engine (~2.0-2.3 s) | 181.589 | 179.690 | 179.662 | - |
| 3 s | 179.587 | 179.633 | 179.006 | -0.656 mm, 9/9 |
| 10 s | 174.208 / 179.587 | 176.056 | 179.006 | -2.002 mm, 9/9 |
| 30 s | 174.208 | 176.056 | 179.006 | -3.634 mm, 9/9 |

Time to depth, seed 0, thirty-second arm: 185 mm at 0.68 s, 182 at 0.97,
180 at 3.00, 179 at 7.67, 177 at 8.36, 175 at 10.59, **174.5 at 11.22**.

**The curve saturates at about eleven seconds.** The thirty-second arm's
median process wall is 12.57 s: every phase reaches a joint fixpoint and
the schedule ends with more than half its budget unspent. What thirty
seconds buys over ten is not depth but *reliability* - seed 0's 174.208
needs a second crossover to fit inside the budget, and on the quieter box
of the first two batteries it fit 6 times in 6 at ten seconds while on
the busier box of the last it fit once in three. Pooled over all three
ten-second batteries: **27 paired rounds against the v1 champion, 7
strictly better, 0 worse, 20 identical.**

Sparrow's pins here are 157.971 at 3 s and 150.165 at 10 s. Ours are
179.6 and 174.2 - **21.6 mm and 24.0 mm behind** - and the shape of the
curve says the gap is not a scheduling gap: our operators reach a joint
fixpoint at eleven seconds, so more budget spent this way does not close
it. The review said orchestration alone cannot reach 160; this is the
first curve that says so in our own numbers.

### Generality: two requests, two bugs, and a verdict that flipped

**The constructor clamp was a fact about mixed-61.** `2.0 x area lower
bound` is dimensionless, and the module header claimed on that basis that
every length here is derived from the request. A dimensionless constant
can still be a fact about one request: twice the area bound is above the
reachable depth only when the request packs at better than 50% of its own
bound, and mixed-61's phase-0 constructor packs at 1.40x its bound while
shapes-17 packs at 2.09x and triangle-20 at 2.29x. On both other requests
**every constructor arm failed** - "skyline construction produced no
publishable layout within the target depth" - eight arms per run, 2.04 s
of a 3.88 s shapes-17 run, buying a guaranteed refusal. The clamp is now
the larger of the area-bound multiple and a depth the request is *known*
to admit a complete layout at, which is the one phase 0 just built;
mixed-61's clamp is unchanged to the digit and both other requests are
rescued, 12 of 12 and 9 of 9 arms exact-valid. The phase also now stops
at the first arm that produces no complete layout, because consecutive
slots differ by a salt of one part in ten thousand.

**Phase deadlines were fractions of the whole budget.** Mode 0 costs
about two seconds on this box, which is 0.67 of a three-second budget, so
every phase whose absolute fraction was below 0.67 was skipped and the
first one above it ran: the most productive operator dropped, a crossover
run in its place on an archive nothing had descended in, and a 3.9 s
process against a 3.0 s budget. Deadlines are now
`f0 + (1 - f0) * share`. This is also what makes the schedule the *same*
schedule across requests, where mode 0 is 20% of ten seconds on 61 pieces
and 9% on 17.

**shapes-17 is a fixpoint and the coordinator says so by stopping.**
200.349 mm at every budget, every seed, every round, identical to the
bare engine, with **zero publications by any operator** in 27 rounds:
mode 0's result is already a joint fixpoint, the coupled separator's arms
collapse to one layout, so `distinct_frontier(2)` has one member and the
crossover phase never runs. The schedule terminates in 2.57 s whether the
budget is 3 s or 30 s. It does not burn a budget it cannot use.

**triangle-20 flips the constructor verdict.** 70.931/70.904/70.901 from
the bare engine, **70.727 on every seed and every round at thirty
seconds**, -0.177 mm paired median at ten seconds, 9 of 9. And the
operator ranking is different: crossover 10 publications in 23 calls,
alternation 9 in 16, compression 7 in 7, and **the constructor slice 6
publications in 12 arms**. On mixed-61 that same slice has now published
0 of 207 arms in this stage and 0 of 19 in PR7. PR7's "the constructor
slice does not pay" is a true statement about mixed-61 and a false one
about triangle-20, and that is the reason v2 makes the slice conditional
rather than deleting it.

One PR7 caveat is *not* discharged: the archive's eviction rule still
never fires in the shipping configuration - triangle-20 peaks at 11 of 16
with zero evictions - though it did fire on a pre-patience probe of the
same request, so it is reachable and still only unit-tested.

### Determinism, and what a work budget does not promise

The affordability guard reads a measured operator cost, which is exactly
the kind of thing that quietly makes a schedule clock-dependent. It does
not, because the cost is quoted in the budget's own currency - work units
under a work budget, seconds only under a wall budget - pinned by a unit
test. Two independent processes are identical **as whole documents** at
40M units on mixed-61 (176.056, 32,327,123 spent), at a *binding* 20M
where the schedule is genuinely different (176.753, one descent call
instead of two, one crossover instead of three, three phases refused),
and at 20M on triangle-20 (70.747).

The honest limit is visible in that last row: it spent 23.3M against a
20M budget, because the guard cannot refuse an operator it has never
priced and triangle-20's first crossover cost 9.45M units. **A work
budget is a bound on what may be started, not on what is spent.** PR7's
other limit stands: the deep operators' Clipper counters are behind
`search-profiling`, so a work budget under-prices constructor arms.

### What this stage shipped

The default path is untouched. All four pinned regression gates reproduce
the pristine `8d9f7e5` binary as **whole documents** - 3,261 and 3,242
compared leaf fields per gate, **0 differences** - with mode 20 at
`independentDepthMm` 206.869 / `8a7737381238fa4d` and the three mode-22
records at raw 159.09233022733062, 159.07876040364795 and
164.0375677990678 at `fa01012af1d559ae`, `e28fba007f8031d4` and
`49f094d7e59a9008`, every arm `exactValid` and `contractValid`, and each
gate additionally reproduced field for field by a second process. Full
release suite green at 1,238 tests, including six new portfolio unit
tests. The coordinator remains reachable only through trailing positional
argument 48.

One observation that is not this stage's doing but belongs in the record:
**`cargo build --release` with the literal default feature set does not
compile at `8d9f7e5`** - `CoupledSeparatorArm::label` and
`LaneSearch::uses_dynamic_pressure` are `#[cfg(feature =
"jagua-experimental")]` while their call sites in `general_relaxed.rs`
are not, and that file is byte-identical to the base commit here. The
gate binary is `--features jagua-experimental` with no measurement
features, which is what "the default-features binary" has meant in
practice in this ledger.

Evidence, drivers and raw batteries:
`docs/experiments/pr7-coordinator-v2/`.
## The inner certificate: Clipper's square join is a tangent cut, so the containment holds at every miter limit

The census closed with two open items and an admission. The prize it
named was the wasted collision-polygon build - 78.66% of 750,434 builds
spent on a pose the row discards two lines later - and the reason it
could not be claimed was that removing one needs a proof in the
*opposite* direction from the separation shield: a proof that a pose
**does** overlap. The natural such proof is an inner circle cover, and
the census wrote that it "rests on `offset_miter(P, e)` containing
`P + disc(e)`, which is believable for Clipper's miter join but is not
proved here. It should not ship on 'believable'."

This entry proves it, builds the certificate on it, and finds that the
proof was not the hard part and the lemma is not even load-bearing.

### The lemma, and why the miter limit is irrelevant to it

Write `Q = offset_miter(P, e)` for the region the offsetter returns for a
positively oriented ring `P` at delta `e > 0`. One thing is taken as
given, and it is the offsetter's own specification rather than a
geometric claim: the positive-fill union of the emitted path is the union
of `P`, of the outward band of every edge, and of the join region emitted
at every vertex. That is what `crates/polygon-nesting-core/src/clipper/`
implements and what its vector tests pin against the reference.

Take `x` in `P + disc(e)`.

* If `x` is in `P` it is in `Q`.
* Otherwise let `p*` be a nearest point of `P` to `x`, so `p*` is on the
  boundary and `|x - p*| <= e`.
  * If `p*` is interior to an edge `E`, then `x - p*` is normal to `E`
    and points away from the material, so `x` is in `E`'s outward band at
    depth at most `e`.
  * If `p*` is a vertex `v`, then `x` is in `disc(v, e)` intersected with
    the exterior normal cone `N(v)`. `N(v)` is empty unless `v` is convex
    seen from outside - which is exactly the vertices Clipper routes to
    `doMiter`/`doSquare` rather than to its concave branch - and for
    those, `disc(v, e)` intersected with `N(v)` is the circular sector
    between the two perpendicular offset points.

So the lemma reduces to one question: **does the join emitted at a convex
vertex contain that sector?** Clipper2 emits one of two joins there,
selected by `cos_a > mitLimSqr - 1`, and both do.

* `doMiter` emits `v + (n_k + n_j) * e/(1 + cos A)` with
  `cos A = n_j . n_k`. Its distance from `v` is `e * sqrt(2/(1 + cos A))`,
  which is at least `e`, along the outward bisector, and it is the
  intersection of the two offset edge lines. The sector's arc is tangent
  to both of those lines at the two perpendicular points, and an arc
  tangent to two lines at two points lies inside the triangle those
  tangency points make with the lines' intersection. Miter contains
  sector.
* `doSquare` sets `ptQ = v + e * b`, where `b` is `getAvgUnitVector` of
  the incoming edge direction and the reverse of the outgoing one - the
  outward angle bisector - and cuts the miter with the line through
  `ptQ` perpendicular to `b`. The arc's farthest point along `b` is at
  distance exactly `e`, which is *on* that line. **The cut is tangent to
  the arc.** Square contains sector too.

The second bullet is the one the census guessed wrong. `MiterLimit` -
2.0 here, `CLIPPER_MITER_LIMIT` - decides only *which* of the two joins is
emitted, and both contain the round join, so

    offset_miter(P, e) contains P + disc(e)

holds **at every miter limit and for every shape**. There is no
counterexample to look for at these parameters. The sharp spike that
would break a bevel join does not break this one, because Clipper's
square join is a tangent cut at `e` rather than a chord. A counterexample
would need an offsetter whose fallback join cuts *inside* `e`, and
Clipper2 does not have one on the Miter+Polygon path this engine uses.

The lemma is about exact arithmetic; four discretisations sit between it
and the code, and each is charged as an explicit erosion of the certified
radius - the grid snap of the transformed ring (0.000708 mm), the
rounding of the offset distance (0.000500), `math_round` on each emitted
offset vertex (0.000708), and `f64` (negligible). They sum to 0.001916 mm
and the constant is 0.005 mm. A polygon whose boundary moves by at most
`d` in Hausdorff distance still contains `disc(c, r - d)` for any
`disc(c, r)` it contained, which is why the erosions simply add.

### The certificate, and where it lands

`fast-constructor-reject` stacks on `fast-constructor-confirm` and is
`search::construction_reject_certificate`. Discs inscribed in the
parent's collision polygons - a computation on the polygon the query is
executed on, needing no lemma at all - against discs inscribed in the
candidate's *source* polygon, transformed rigidly to the pose and
inflated by the expansion, which is where the lemma is spent. Two discs
at centre distance below the sum of the certified radii prove the two
collision polygons meet in positive area, so the row returns `None`
before the Clipper offset and before a single pair question.

The counting build priced it beside the exact answer on every row, at
four cover sizes, and counted the observation that would falsify it - a
certificate issued for a row the exact tier then *accepted*:

| site | rows | accepted | overlap rejects | certified @1 | @2 | @4 | @8 |
|---|---:|---:|---:|---:|---:|---:|---:|
| candidate | 432,710 | 11,292 | 398,050 | 288,693 | 320,216 | 330,260 | 330,746 |
| slide ladder | 191,925 | 130,482 | 46,709 | 2,420 | 5,199 | 5,878 | 5,976 |
| slide bisect | 122,886 | 15,453 | 78,616 | 1,895 | 4,133 | 4,846 | 4,889 |
| deep total | 747,521 | 157,227 | 523,375 | 293,008 | 329,548 | 340,984 | 341,611 |

Soundness violations: **0**, over 157,227 accepted rows.

It lands where the census said the speculation is. **76.32% of the
candidate stream's rows, and 82.97% of its overlap rejections, are proved
at four discs.** The two slide sites are 3.1% and 3.9%, which is the
census's asymmetry restated from the other side: a ladder rung starts
from an already-valid pose, so its failures are shallow contact-band
overlaps rather than a candidate dropped on top of a placed piece. Four
discs is the knee - eight adds 486 rows to the candidate stream, 0.15%,
for four times the pair arithmetic.

And the lemma turns out not to be load-bearing. With the whole inflation
taken back off - the fallback that needs nothing but `offset(P, e)`
containing `P`, which is far weaker - the candidate stream still proves
**312,692 rows, 94.5% of the inflated certificate's**. The slide sites
collapse to 241 and 18. So the proof above buys 5.5% of the prize on the
population that matters, and the design could retreat from it without
losing the stage. That is worth recording precisely because the census
identified the lemma as the blocker.

### Ordering was measured, and pruning is what shipped

The task the census set was "prune the speculative stream", and the first
question is whether *ordering* it would do better. The counting build
answers it directly rather than by argument. Per candidate slot it
records each confirmed row as `(signed certificate pressure, accepted)` -
pressure positive is a proof and its depth, negative is the closest
approach the certificate could not close, so ascending is "cleanest
first" - and compares two prefix lengths over the identical row set:

| statistic | value |
|---|---:|
| candidate slots | 2,872 |
| candidate rows confirmed | 432,710 |
| acceptances | 11,292 |
| rows the current order confirms to reach them | 432,710 |
| rows a lazy proxy-ordered confirmation would confirm | **59,414** |

`prefixActual` equals `rows` exactly, which is a fact about the loop and
not a coincidence: it breaks at the top of the iteration after the fourth
finalist, so the last candidate row it confirms is always the accepting
one. A perfect lazy proxy order would therefore avoid **86.27%** of the
candidate stream's exact confirmations. The sound prune avoids
**76.32%** - **88.5% of the ordering prize, with the accept/reject
semantics untouched.**

The remaining 11.5% is not available, and the reason is worth stating
because the brief allowed for gating it. **A reordering of this loop is
not semantics-preserving and cannot be rescued by a tie-break
refinement.** The loop is capped three ways - four finalists per slot,
320 rows per piece, and a per-provenance row cap - and every accepted
candidate then spends further rows on a contact walk. Which four poses
become finalists, and how many rows remain when they do, are both
functions of the order the rows were offered in, so there is no
configuration in which the acceptance rule is order-invariant while any
cap is live. The certificate sidesteps the question rather than answering
it: it removes rows whose verdict was never in doubt, in place, leaving
the order, the row charges and the finalist set identical.

### What it is worth

Paired interleaved A/Bs of the gate-1 stream, arms alternating order
every round, statistic the per-round paired ratio, on a box a second
agent was benchmarking on:

| sample | rounds | arm A | A median | B median | paired median | spread | below 1.0 |
|---|---:|---|---:|---:|---:|---|---:|
| 1 | 12 | confirm | 5.849 s | 4.054 s | **0.6924** | 0.6875-0.6981 | 12/12 |
| 2 | 10 | confirm | 5.851 s | 4.066 s | **0.6956** | 0.6881-0.6984 | 10/10 |
| 3 | 8 | confirm | 5.853 s | 4.052 s | **0.6923** | 0.6875-0.6975 | 8/8 |
| chain | 8 | **default** | 26.278 s | 4.059 s | **0.1545** | 0.1538-0.1565 | 8/8 |
| mode 22 | 10 | confirm | 3.135 s | 3.119 s | **0.9966** | 0.9822-1.0137 | 6/10 |

Three independent samples of the mode-20 A/B agree to 0.5%, the third
taken on binaries rebuilt from the committed tree. **The honest
multiplier is 6.47x**, and this time it is one measurement rather than
three chained: the default build and the flag-on build, interleaved, on
the same stream. Mode 20 goes 26.2 s -> 6.2 s -> 5.85 s -> **4.06 s**,
against Sol's ~2 s, so the remaining gap is about 2.0x where the census
left it at 2.9x. The mode-22 row is a real zero and is reported because
it could have been a regression - the record replay barely exercises the
constructor, so the certificate's arithmetic is charged there with almost
nothing to remove, and it comes back at parity with the sign changing
inside the sample.

Profiled, two numbers carry the whole result:

| phase | flag off | flag on | calls off | calls on |
|---|---:|---:|---:|---:|
| `exactOverlapTest` | 2,948.8 ms | 1,791.5 ms | 1,266,102 | 925,309 |
| `collisionPolygonBuild` | 2,024.3 ms | 1,062.9 ms | 750,434 | **409,450** |
| `vacancyExactRows` | 3,562.4 ms | 1,514.4 ms | 747,521 | **747,521** |
| `pairCollide` | 1,253.7 ms | 1,264.7 ms | 15,562,760 | 15,562,760 |
| `moveSweep` | 5,229.7 ms | 5,222.5 ms | 4,089 | 4,089 |
| leaf total | 9,712.8 ms | 7,565.1 ms | | |

`collisionPolygonBuild` loses exactly 340,984 calls - the census's
`rowsCertified4` total, to the digit - and `vacancyExactRows` keeps all
747,521 of its calls, because the row is still charged, still counted
against the finalist-row budget, and still asked. Only the work behind
the answer is gone. Everything outside the constructor moves by less than
1%, in both directions, which is the box.

### Equivalence, and the one thing that is not bit-identical

Flag off, all four gates reproduce the pristine `8d9f7e5` binary as whole
documents - 3,271 and 3,252 compared leaf fields per gate, six differing,
all of them the executable hash and the five wall-clock fields. Flag on,
gates 2, 3 and 4 are identical in **every** field; gate 1 differs in
five, and all five are the removed work honestly not charged:
`experimentalCollisionBuilds` 786,724 -> 445,740,
`transformedCollisionVertices` 5,221,858 -> 3,313,762,
`experimentalPairVisits` 13,120,423 -> 8,189,813, `clipperInputVertices`
37,012,470 -> 32,823,802, `clipperOutputVertices` 2,572,148 -> 654,362.

Those five are quota quantities, so the honest statement of the risk is
that a cap they gate could bind later on the flag-on arm than on the
flag-off one. Three things bound it. The cap that actually paces the
constructor is `max_exact_finalist_rows`, and it is charged *identically* -
the row is still counted before the certificate is consulted, which is
why `vacancyExactRows` keeps all 747,521 calls. The other caps are
derived from that one and are the "every build at the 512-vertex ceiling"
bound, which is slack by orders of magnitude. And empirically, if any cap
had bound in either arm the other 3,266 fields of gate 1 could not have
been identical, because the arm that ran further would have produced a
different layout rather than the same one with smaller counters.

Two runs of the flag-on binary are identical field for field on all four
gates apart from the clock. A debug build with the `debug_assert` live -
every certified row builds its collision polygon anyway and must find a
positive exact intersection area against some active piece - reproduces
all four gates and the assertion never fired. Two unit tests issue
certificates over dense placement grids of a rotated and of a mirrored
non-convex piece and require that none is ever issued for a pair whose
exact collision polygons are disjoint. The release suite is green at
1,234 tests on `jagua-experimental` and 1,236 with the reject flag on,
which is the same suite plus those two. One caveat is inherited rather
than introduced: `cargo test --features fast-constructor-profile` does
not compile at the base commit either, because a `construction_void_grid`
test there calls `derived_cell_mm` with three arguments of four.

The constructor-quality gate is the same four salted targets and two
relaxed seeds as the shield's, each endpoint pinned and descended by the
**default** binary: endpoints 206.869, 206.666, 199.801 and 214.042, all
eight paired descended-depth deltas exactly **0.0**, at identical
endpoint and descendant fingerprints. As with the shield, a zero here is
the predicted result rather than a lucky one, and the gate earns its
place by being what would catch a wrong soundness argument.

### What this entry leaves open, sized

* **The containment rejection is the next 66,919 rows.** The census
  counts 66,919 deep rows rejected by `fits_rect` rather than by an
  overlap - 8.95% of rows, every one a wasted build, and 16.3% of the
  409,450 builds this stage leaves. The lemma above already covers it: a
  transformed source vertex more than `e` outside the sheet rectangle
  proves the row fails containment. It is not implemented here
  deliberately, because it is a second mechanism with a second slack
  budget and this stage's value is that it ships one.
* **The candidate stream's last 17%** - 67,304 of its 398,050 overlap
  rejections - are grazes a disc cover cannot close, and the ordering
  measurement says a perfect proxy would reach about ten points more of
  the stream than the sound prune does. Closing it needs a better inner
  cover rather than a different idea: discs cover a long thin piece
  badly, and an inner convex decomposition is the honest next tier.
* **The slide sites stay almost untouched**, at 3.1% and 3.9% of rows,
  and the uninflated column says why - their overlaps live inside the
  expansion band, so they are exactly the population an inner certificate
  is worst at and the separation shield is best at.
* **The residual is no longer the constructor.** At 4.06 s the mode-20
  leaf is `moveSweep` 5,222.5 ms and `scorePlacement` 4,752.7 ms - the
  relaxed lane. The constructor's two exact phases are now 2,854 ms of a
  7,565 ms leaf, down from 4,973 of 9,713, and the next measurement
  belongs somewhere else.

Evidence, drivers, the full per-site census and the ordering statistic:
`docs/experiments/constructor-inner-certificate/`.

## PR9 - The relaxed lane's residual: a decomposition, and the 2x that is not there

The previous chapter handed on a pointer rather than a result: at 4.06 s the
mode-20 leaf was `moveSweep` 5,222.5 ms and `scorePlacement` 4,752.7 ms, "the
relaxed lane, not the constructor", with Sol's ~2 s slice about 2.0x away. This
chapter measures that lane on three streams, and its main finding is a negative
one that the ledger should carry forward explicitly: **the 2x is not available
in this lane as a semantics-preserving change.**

### One correction that changes how the previous table reads

`moveSweep` at 5,222.5 ms inside a 4,060 ms stream is not a paradox. **Phase
milliseconds are summed across the eight lane threads.** Every phase total in
this repository's profiled tables is a thread sum, so it can exceed the stream's
wall clock and it is never a wall-clock claim. The chapter above is not wrong -
its A/B rows are all paired wall-clock measurements - but the leaf table beside
them invites the reading that `moveSweep` *is* 5.2 seconds of the stream, and it
is not.

A second correction, for anyone reproducing: `fast-constructor-confirm` and
`fast-constructor-reject` are **stacked on `fast-constructor-profile`**. Built
without it, the mode-20 gate-1 stream is 24.2 s rather than 4.06 s even though
the certificate is fully active and `collisionPolygonBuilds` falls to exactly
409,450.

### What the lane is doing

A new measurement build, `relaxed-lane-census`, splits the generic scorer into
`scoreProbe` / `scoreScan` / `scoreFinalize` and adds five exact counters. The
scan's structure turns out to be almost **stream-invariant**:

| statistic | m20 g1 | m22 g2 | coordinator 10 s |
|---|---:|---:|---:|
| candidate queries | 4,089,768 | 10,898,458 | 20,645,490 |
| ordered-catalogue descents | 22,617,886 | 59,877,384 | 102,974,975 |
| neighbours returned / visited per scan | 7.37 / 4.28 | 7.45 / 4.29 | 7.17 / 4.14 |
| scans stopping on the upper bound | 81.7% | 83.7% | 82.1% |
| collision rows per scan | 1.77 | 1.80 | 1.71 |
| scan residual per visited neighbour | 60.1 ns | 59.8 ns | 59.4 ns |
| `scoreProbe` per generic scan | 147.2 ns | 153.2 ns | 149.2 ns |
| `pairCollide`+`pairPressure` share of `scorePlacement` | 42.0% | 42.0% | 37.7% |

Two per-unit costs reproducing to within 1.2% across three streams with a 5x
range of call counts is the evidence that this is the loop and not the noise.

**The coordinator confirms the premise in its strongest form**: on a 10 s run
from the bare request, `scorePlacement` is **91.6% of leaf time**, and
`collisionPolygonBuilds` is 2,913. At a budget the constructor is a rounding
error and this lane is the whole engine.

### Two of the four hypotheses are dead

* **"Upper-bound cutoffs unexploited" is false.** 81.7-83.7% of scans already
  stop early on the caller's bound. That cutoff is *why* only 4.2 of 7.3
  returned neighbours are visited; there is nothing left to take.
* **"Rescans of unmoved pieces" is not where the calls are.** The scan is
  re-entered because the candidate moved, and its neighbour set is already
  small and bounded by a 16x16 broad-phase grid.

### What shipped, and what it was worth

Two stacked flags, off by default, **bit-identical as whole documents on all
four gates** - 6 fields differing, the executable hash and five wall-clock
quartiles, and no work diagnostic moving at all.

`relaxed-scan-shape-reuse` removes the scorer's *second* descent for the
candidate's own key: the broad-phase probe needed the bounds that live in the
shape the scan was about to resolve anyway. `relaxed-cached-pose-bounds` routes
the lane's per-pose bounds lookup through the `AngleKeyCache` memo the scan has
always used, instead of re-deriving the rotation key with `rem_euclid` on each
of 4.45M / 11.64M / 23.36M calls. Together they remove 36% / 35% / 39% of all
catalogue descents.

| sample | rounds | paired median | range | below 1.0 |
|---|---:|---:|---|---:|
| m20 g1, `+shape-reuse` only | 12 | 0.9976 | 0.9888-1.0052 | 7/12 |
| m22 g2, `+shape-reuse` only | 10 | 0.9754 | 0.9611-1.0031 | 9/10 |
| m20 g1, `+both` | 12 | **0.9917** | 0.9836-1.0023 | 11/12 |
| m22 g2, `+both` | 12 | **0.9700** | 0.9473-0.9823 | 12/12 |
| coordinator `work=20000000`, `+both` | 10 | **0.9750** | 0.9620-0.9909 | 10/10 |

The coordinator row is the load-bearing one: budgeted in *work units*, both arms
run the identical scheduled search, all ten rounds are below parity, and the
incumbent is identical - depth 180.64489329491147, one publication at 9,064,287
work units, only the publication's wall-clock timestamp moving. Mode 22 is the
biggest winner and is the mirror image of the previous stage, where m22 was the
row that came back at parity: that stream barely touches the constructor, so it
is nearly pure relaxed lane.

The m20 `shape-reuse`-only row is reported as the parity result it is, range
straddling 1.0, rather than rounded up into a win.

### Why there is no 2x here, and where the next one is

**42% of the scorer is `pairCollide` + `pairPressure`**, already proved to be at
their floor. What remains is a 60 ns-per-neighbour residual and a
149 ns-per-scan probe, and neither is one thing - each is a catalogue descent, a
key derivation, a weights lookup, a bin walk, a small sort and a `Vec` push.
Removing a third of the largest of those items bought 2.5-3.0%. There is no
single semantics-preserving item left in this lane worth more than a few points.

The two things that *are* worth more:

* **Allocation, and it is bigger than what shipped.** On m22 g2 - the
  relaxed-dominated stream - the run makes **50,455,080 allocations for 8.41 GB
  of gross demand, 5.30 per candidate scan.** The scorer builds a fresh
  `Vec<(usize, usize, f64)>` per call for 1.80 rows, grows it a power of two at
  a time, and `search_piece` then clones it twice more. A pooled row buffer the
  tracker swaps rather than copies is the shape of the fix - the lane already
  does this for `collision_merge_scratch` - but it changes what `MovedRowDelta`
  owns and wants its own stage.
* **Ordering the scan cheapest-first, which is class (B).** 42% of returned
  neighbours are never visited because the cutoff fires first, so the prize is
  in the order. But the iteration order decides which rows land before the
  cutoff, hence `pruned`, hence `MovedRows`, hence what the tracker installs -
  the same structure the constructor census met in its finalist loop, with the
  same verdict: not semantics-preserving, and no tie-break refinement makes it
  so. Its experiment is designed in the stage directory: descendant depth under
  a fixed *work* budget, four target salts x two relaxed seeds, paired per salt,
  with `exactValid`/`contractValid` on every publication as the falsifier.

One caveat is recorded against the fixed-side descents that remain (15.4M /
40.8M / 69.4M, one per visited neighbour): **their prize must not be estimated
by scaling this stage's result.** What shipped removed a `rem_euclid` *and* a
descent per call, while the fixed-side loop already reads its key from the memo,
so its per-unit value is strictly smaller than 4.3x what shipped. Removing them
needs a slab - `BTreeMap<SurrogateKey, u32>` plus `Vec<OrientedSurrogate>` -
with a per-piece slot memo, because the present map cannot hand out a handle
that survives across calls.

Evidence, drivers, all three decompositions and the five A/B samples:
`docs/experiments/relaxed-lane-residual/`.
## Mode 26's seconds are a rollback comparison, not geometry — and the compression clamp is already a proxy-tier parameter

Mode 26 (clamped-sheet ladder compression) is the mechanism behind the
159.079 record and the review's standing objection to it: "12-95 seconds
of operator work against a 0.5-1.0 second production slice". The next
round wants that mechanism at kernel frequency — a continuous per-move
compression schedule on the proxy kernel. This entry is the measured
anatomy that round has to be built from. It changes no engine
behaviour: the only code change is a wall-clock anatomy block on the
mode-26 ladder, rung and arm diagnostics, compiled in **only** under
`search-profiling`, and all four pinned gates reproduce with every
search-visible field identical before and after.

Sample: eight profiled mode-26 ladders on the true 5.0/5.0
exact-clearance contract — drops of 0.3, 0.55 and 1.0 mm below the
record parent at seeds 0 and 1, plus 0.3 and 1.0 mm below the
from-scratch parent — **35 rungs, 171 rung arms, 330.73 s of arm wall**.
Every number below is from a profiling build and is a decomposition,
never a wall-clock claim.

### The band is the ladder, and orchestration is nothing

An operator call is 9.98-81.13 s (process wall 12.51-83.25 s), a rung is
4.66-13.80 s and an arm is 1.23-4.03 s. The time *between* the rungs —
both warm-start fingerprints, the placement clones, the publication
bookkeeping — is **35.0 ms in total across all 35 rungs**, and the
ladder's own overhead outside its rungs is 0.010-0.045 ms per ladder.
There is nothing to optimise in the orchestration. It is all inside the
arms, and **90.14% of an arm is one call**: the clamped mode-0 pipeline.

### Three quarters of the time is discarded by a four-ulp comparison

**146 of 171 arms (85.4%) end on a rollback-tracker abort and produce no
state at all**, at a median 1.687 s each — **249.81 s of 330.73 s,
75.53% of every second mode 26 spends.** Not one of the 171 arms
produced an exact-valid state; every one attempted exactly one
contraction target of the 32 available and accepted zero, and
`epochsImproved` was 0 in all 171.

All 146 aborts are the same comparison — the per-piece **incident-loss
vector**, never a pair pressure and never the boundary total — and the
gaps are 0-6 f32 ulps (median 2), relative 3.9e-8 to 4.9e-7. The
mechanism is in the code and is not a bug: `ToleratesPoleRounding`
grants 64 f32 ulps to `RollbackMagnitude::PairPressure` because a pole
pressure reaches `f64` through `f64::from(f32)`, while the per-piece
sums are `NativeF64` and fall back to `equal_within_one_ulp` at one
**f64** ulp. At the magnitudes involved that rule is nine orders of
magnitude tighter than the disagreement it refuses, and the same policy
already tolerates 644 wider-provenance comparisons in the same runs at a
per-rung maximum of 2-6 ulps. Widening it is a search-trajectory change
and would need its own gate; it is recorded here because it is where
mode 26's wall time goes.

### The inner loop is already at production speed

| ladder | rungs | wall | candidate queries/s | effective moves/s |
|---|---:|---:|---:|---:|
| lin d0.3 s0 | 2 | 9.98 s | 3.814M | 34,059 |
| lin d1.0 s1 | 7 | 81.13 s | 3.352M | 29,749 |
| fs d1.0 s0 | 7 | 62.90 s | 3.493M | 30,962 |

Across all eight: 3.31-3.81M candidate queries/s and 29.2-34.1K
effective moves/s, **under a profiling build that costs ~4.5%**, against
this ledger's own m22 replay figure of 3.775M evaluations/s at ~265 ns
and 33.9K moves/s. Within measurement it is the same loop at the same
rate. The 12-95 seconds are 38-272 **million** candidate evaluations
spent to move one bound by 0.159 mm — a structural cost, not a slow
kernel.

Leaf shares over the whole sample (1,322.88 thread-s, 4.00x effective
parallelism on 8 workers): proxy collider plus boundary penalty
**55.59%**, dynamic hazard adapter **41.27%**, exact tier **1.55%**
(`exactOverlapTest` 1,904.8 ns/call, `collisionPolygonBuild` 4,149.3
ns/call, all of it inside the repair tiers). `boundaryPenalty` — the
clamp — is 84.9 ns/call over 1,175,941,640 calls, 1.042 per candidate
query.

### The repair tiers, priced

Only the 25 arms that produced a terminal state ever reach a repair.
Per arm when it runs: `micro_legalize` 0.808 ms, single-piece
re-placement 51.5 ms, joint re-placement 495.6 ms, the global Hildreth
program (mode 31) 888.3 ms — medians. **The only tier cheap enough for a
per-move loop published in none of the 25.**

### The number the port turns on

One complete exact confirmation of the 61-piece layout — depth, the
1,830-pair exact overlap census and `validate_and_measure_placements` —
costs **0.491 ms mean (0.213-0.664, n=25)**. At 5% of the production
slice that buys 50 confirmations in 0.5 s or 101 in 1.0 s: one per ~340
effective moves, ~100/s against the 16 m0 epoch scopes in 1.193 s this
ledger's own quality-curve trace records (13.4/s) — about 7.5x more
often than mode 0 confirms today.

### What is already proxy-tier, and what is missing

`boundary_penalty(&placement, strip_depth_mm)` is pure `f64` arithmetic
on cached axis-aligned bounds and **already takes the depth as a
parameter at all eleven of its call sites**; every candidate generator
derives its sampling box from the same scalar; `PairTracker.
collision_pairs` plus `piece_is_active` already are the violating-pair
queue a repair needs, and `move_sweep` already builds its work order
from them. A per-sweep schedule is therefore a *substitution at the call
site*: one `f64` write per sweep and zero additional geometry.

What is genuinely missing is not geometry. It is (a) a lane-owned
schedule object — `strip_depth_mm` is written in five non-test places
and every one is a whole-pipeline decision; (b) a **monotone floor in the proxy
tier**, because today the only thing preventing depth-ward relaxation is
`sheet_long_axis_mm` in the *exact* tier, which is precisely why mode 26
had to build a whole clamped pipeline per rung; (c) a lane-local
deepest-confirmed slot; (d) a repair that answers the residue a schedule
makes, given that the affordable tier answered none of 25; and (e) a
rollback contract that survives a moving depth — the one that already
destroys 75.5% of mode 26.

### The design, and the budget it has to fit

At the plan's production rate and this sample's measured sweep shape
(2,555.4 candidate queries and 22.70 accepted moves per sweep), a
0.5-1.0 s slice is 1.89-3.78M queries, 16,950-33,900 effective moves and
739-1,478 sweeps — **5.9% to 11.7% of a single mode-26 rung**. One rung's
0.159 mm of depth spread over those sweeps is 0.11-0.22 µm each, below
the canonical 1/1000 mm grid, so the schedule quantises to **1 µm per
step, 159 steps per rung-equivalent, 4.6-9.3 repair sweeps per step, and
an exact confirmation every fourth step** (40 x 0.491 ms = 19.6 ms, 2.0%
of a 1.0 s slice).

Feature flag `compression-schedule`, off by default, with the setting
`None` reproducing today's `state.strip_depth_mm` path exactly. Quality
gate: matched-arm at equal *work* budget — one measured rung, 32,246,564
candidate queries plus 5 x 233,445 exact pair tests = 33,413,789
`PortfolioBudget::Work` units — fast schedule against a
mode-26 short ladder, both from the same pinned parent
(159.07876040364795 and 164.0375677990678) at the same seed, statistic
the paired median delta of best raw depth, with the parent as the floor
for both arms.

Three risks, in order: the rollback contract is the dominant cost and a
moving depth makes that comparison strictly harder; the residue may not
be rounding-scale, and the only repair the slice can afford published in
0 of 25 arms; and **this sample measured mode 26's cost honestly and its
yield not at all** — zero of eight ladders published, so a matched-arm
gate run at these seeds would compare two zeros and must be run at
enough seeds that the control publishes.

Evidence, drivers, the per-ladder/per-rung/per-arm tables and the abort
census: `docs/experiments/mode26-rung-anatomy/`.

## The saturated run had 4,318 crossover actions and had tried three — and the arm that broke it was the clamped ladder, not the constructor

Sol's round-4 close asks for one measurement before any further spend: an
**opportunity-and-delayed-credit ledger** on the saturated state, then an
**A/B/C at identical work** on the three saturated archives. This chapter is
that measurement. It reproduces coordinator v2's three depths exactly
(174.20812003998896 / 176.05599999999998 / 179.006, all `dualGateValid`, all
cross-process deterministic over 8,233-8,844 compared fields) and then reports
what those states still had available.

Everything is `mixed-61` from the bare request, `work=120,000,000` (three times
coordinator v2's own 40M ten-second anchor), search-offset allowance `0.002`
— **coordinator v2's contract, not the four pinned gates' `0.0005`** — so
these depths are comparable to 174.208 and are not comparable to the 159/164
record lineage.

### The saturation is a naming problem, and the ledger says so in one field

`PhaseReport` now carries an exit cause, and on all three seeds it reads the
same:

| phase | exit cause |
|---|---|
| `descent` | **`keysExhausted`** |
| `crossover` | **`completed`** — its `crossover_attempts = 3` counter, not its pairs |
| `compression` | **`noResidue`** |
| `diversify` | **`patience`** |

**Not one phase on any seed exits on `deadline` or on `affordability`**, and the
run stops having spent 23-27% of its budget. Sol's "it is a fixpoint of the
finite top-3/midpoint/one-direction queue, not of the operator space" is now a
field rather than an inference.

The size of what the queue does not name:

| | seed 0 | seed 1 | seed 2 |
|---|---:|---:|---:|
| ordered pairs over the whole archive | 72 | 72 | 56 |
| ordered, cut-derived crossover actions | **4,318** | **4,316** | **3,357** |
| attempted | **3** | **3** | **3** |
| actions over the crossover phase's *own* top-3 frontier | 360 | 360 | 360 |
| of those, still on the frontier at exit and attempted | **1** | **1** | **1** |

The cuts are derived rather than gridded: for an ordered pair the cut only
partitions **A's occupied short-axis positions**, so the continuum collapses to
one action per gap, placed at the gap's midpoint. On 61 pieces that is 60 bands
per ordered pair, and **all 60 produce a distinct hybrid on every seed** —
`bands whose lower edge holds no differing piece: 0 of 360`. Band gaps run
0.072-185.167 mm with a p50 of 19.8-24.4 mm.

And the phase ordering costs more than the queue does: **on seeds 0 and 1 the
final rank-0 state was never a crossover parent at all**, because it is born in
the *compression* phase, after the crossover phase has ended. On seed 1 the
ledger's next untried action is the plain `rank0 -> rank1` **midpoint** cut —
the schedule's own action, on the schedule's own two best states, made
unreachable by the order the phases run in.

### Selection excludes by top-K; the bit-exact similarity rule excludes nobody

| | seed 0 | seed 1 | seed 2 |
|---|---:|---:|---:|
| excluded by **top-K** | 6 | 6 | 5 |
| excluded by the **bit-exact-pose similarity rule** | **0** | **0** | **0** |
| members receiving **no action at all** | 3 | 4 | 4 |

Review 4 §4 names the pose-equality rule as a reason members never receive an
action. On these three archives it never fires. The rule is as fragile as §4
says and this does not defend it — but it is not what is costing actions here,
and eviction remains inert (8-9 of 16, zero evictions, zero
`RefusedArchiveFullAllDistinct`).

### Deferred credit: the archive earns its retention, the m20 feeder does not

`ArchivedBasin` now records **both** crossover parents. It recorded one, so on
the old record every recombination was a genealogical dead end and no basin
that fed parent B was ever anyone's ancestor. With the edge in place, seed 0's
incumbent lineage is:

```
m0     181.5890 @  8,777,493 units
mode22 179.5869 @ 10,792,266
mode23 179.6386 @ 18,921,527   <- worse than its own parent, top-K excluded, and an ancestor anyway
mode23 176.3094 @ 23,988,191
mode22 174.2081 @ 31,427,729
```

That 179.6386 state is precisely the object the archive exists for. The m0 basin
is likewise top-K excluded on seeds 0 and 1 and is an ancestor at 3 and 2
generations.

**The m20 feeder judged on deferred credit, which is the measure Sol asks for:**
on all three seeds the archived mode-20 basin receives exactly **1** action and
has **0 descendant publications**; the phase-0 constructor basin (182.976, the
same seed-independent fingerprint on all three) receives **0** actions.

### Δraw per million evaluations, and a four-order-of-magnitude mispricing

| phase / operator | calls | published | Δraw mm (s0/s1/s2) | **Δraw / M eval** |
|---|---:|---:|---|---|
| compression / mode22 | 1 | 1/1/0 | 2.101 / 0.697 / 0 | **1.1017 / 0.4407 / 0** |
| descent / mode22 | 2 | 1/1/1 | 2.002 / 0.057 / 0.656 | 0.4264 / 0.0119 / 0.1832 |
| crossover / mode23 | 3 | 1/1/0 | 3.277 / 2.880 / 0 | 0.2043 / 0.1881 / 0 |
| diversify / mode20 | 1 | 0/0/0 | 0 | 0 |

The compressing micro-descent is the most efficient operator in the schedule by
a factor of five, and the schedule gives it one call.

And the caveat PR7 recorded is not a rounding error: **a mode-20 arm costs
260-335 work units and 3.02-3.24 seconds.** The work budget prices a constructor
arm at about 1/6,000 of an m22 quantum and the clock prices it at 3x one.

### The A/B/C, at 21,000,000 work units per arm, plus the control it needed

Every arm runs the identical base schedule and then one probe phase *after the
drain*, so the arms are paired on the same saturated archive by construction.
21M is the smallest round allowance above every arm's own spend; re-running at
400M is bit-identical, so the allowance is non-binding.

A fourth arm was added because without it C's number is not a statement about
the clamp: **D** asks the schedule's own mode-22 for the *same target depth*
from the *same parent*, with no clamp.

| seed | A: next derived crossover | B: m20 ticket -> crossover -> m22 | **C: m26 short ladder -> m31** | D: control |
|---:|---:|---:|---:|---:|
| 0 | 0.0000 | 0.0000 | **-4.9571** | -2.6203 |
| 1 | 0.0000 | 0.0000 | **-4.3170** | 0.0000 |
| 2 | **-0.7203** | 0.0000 | 0.0000 | 0.0000 |
| publishes | 1 of 3 | **0 of 3** | **2 of 3** | 1 of 3 |

**Sol predicted B breaks the saturation and C is the 165 component. B breaks
nothing at any seed; C is both the breaker and the largest single gain this
coordinator has ever made.**

* **169.251 mm on seed 0 and 171.739 mm on seed 1**, from the bare request, both
  `exactValid` and `contractValid`, and both **re-confirmed in a separate
  process from the default-feature binary through mode 27 with zero repair
  applied and the fingerprint unchanged**. 169.251 is **4.957 mm below the
  previous best-from-request layout on this request**.
* C's ladder is 2 rungs and 2-3 arms — the mode's own bounds function turns a
  0.3 mm drop at a 174 mm parent into a 0.174208 mm step — and **rung 1
  publishes 4.25 mm and 3.61 mm below its own requested bound.** The clamp
  removes the depth-ward room and the separator then compresses far past what it
  was asked for.
* **This is the census the review demanded as the gate before the L-sized m26
  port, and it passes.** The rung anatomy measured 0 of 171 publishing arms at
  159 mm and 164 mm parents; at 174-179 mm parents, two of three ladders publish
  on their first probe. The anatomy's sample is not a prediction for this band.
* B's chain is monotonically improving and starts 42-53 mm behind: ticket
  216.5-227.5, direct crossover with the incumbent 192.0-200.9, short m22
  187.3-194.6. One generation of `m20 -> crossover -> m22` does not close that,
  and Part 1's genealogy says the same thing from the other side.
* A produces a legal hybrid every time (176.817 / 178.722 / 178.286) and it is
  simply not better than its own parent on two of three seeds. The derived cut
  is not a no-op — seed 0's is cut 0.4958 in a 4.606 mm band, 30 pieces from A
  and 31 from B — it is just not a win there.
* **D is a finding of its own.** On seed 0, asking mode 22 for `incumbent - 0.3`
  publishes **2.620 mm for 3.08M units and 1.37 s**. The schedule never asks:
  its compression phase asks for `depth + 0.8`, a *looser* target than the
  incumbent it already holds, gets an exact-valid answer, and exits `noResidue`.
  2.620 mm was one sign away from the schedule's own most efficient operator.
  C-minus-D — 2.337 mm on seed 0, the whole 4.317 mm on seed 1 — is the clamp.
* The coordinator-level mode 31 lands **exactly on the bound it is given** on
  both states where it succeeded (0.404 mm, published depth equal to the
  requested bound to the digit). Its contribution here is a property of
  `COMPRESSION_RUNG_MM`, not a measurement of its reach. That is a lead.

### What this does not measure

One request and three seeds. `0.002`, not `0.0005`. **Equal work is not equal
wall and the gap is worst exactly where it matters** — at equal work B is the
cheapest arm and on the clock it is the second most expensive, so the *cost*
half of B's verdict is unreliable in both directions, though its 0/3 is not.
No wall-clock claim is made: the box is shared and one arm's probe took 2.36 s,
2.44 s and 5.62 s on three runs with a bit-identical 6,036,325-unit spend. Arm C's mode 26
runs its own internal repair tiers including the global program, and the split
between those and the outer m31 rung is not separated here. No default was
moved; the probe is one action from a saturated state, not a proposal for where
it belongs in a budget.

Instrumented behind `portfolio-ledger`, off by default; the four pinned gates
reproduce the pristine `57ad992` binary as **whole documents**, 3,244-3,263
fields each, **0 differences**. Release suite green: 1,238 passed, 0 failed,
2 ignored.

Evidence, drivers, the five ledger tables and the per-call A/B/C/D detail:
`docs/experiments/opportunity-ledger/`.
## PR10 — One neighbour visit in seven is removable, and it costs 8.968 mm; the row buffer is 15% of the allocator and buys nothing

The relaxed-lane census left two levers sized and unbuilt: the class (B)
scan ordering, and the class (A) allocation. Both were built behind
default-off flags at `57ad992` and measured. **The verdicts are
opposite, and neither is the one the sizing note expected.**

### The scan ordering: rejected, and it removes the most work of anything measured in this lane

`relaxed-scan-order-proxy` sorts the broad phase's neighbours by the
squared distance between the two placements' translation origins before
the scan, instead of taking them in the ascending piece index
`PieceQueryScratch::query_into` leaves them in. Near neighbours are asked
first, so the caller's upper bound — which already ends 82% of scans — is
crossed on fewer of them.

It works, on its own terms. `pieceBroadPhaseProbes` falls from
15,562,760 to 13,255,792 on m20 g1 (**0.8518**) and from 19,264,010 to
16,313,072 on each mode-22 gate (**0.8468**). One neighbour visit in
seven disappears. `satTests` falls 2.9-3.7%, `cellIndexProbes` 0.8-1.2%,
`surrogateEvaluations` 0.5% and `acceptedMoves` 0.3%.

And it is rejected on both of the things a class (B) lever has to buy:

* **Quality.** Sixteen matched cells — four mode-20 target salts x four
  relaxed seeds, both arms descending the *same* pinned parent on the
  identical mode-22 schedule. Fifteen cells are bit-identical, raw depth
  and placement fingerprint alike. The sixteenth loses **8.968 mm**
  (170.648 → 179.616, salt 320.000, seed 1). **Zero cells improve.**
  Every cell in both arms is `exactValid` and `contractValid`, so the
  designed falsifier is not what sinks it — the ledger is.
* **Speed.** Paired interleaved coordinator A/Bs at
  `work=20000000`: 0.9867 over 14 rounds in a quiet window, 1.0100 over
  10 rounds in a loaded one, both ranges straddling parity. **The two
  campaigns disagree in sign.** Deleting one visit in seven does not move
  the wall clock, which says the per-neighbour cost saved is close to the
  per-scan cost of the keying pass and the sort that save it.

At an identical *work* budget the lever cannot convert speed into search
— the budget is in work units — so the coordinator run measures quality
alone, and quality there is exactly neutral: five seeds, five identical
incumbents, five identical published fingerprints, with the publication
work ordinals differing by up to 8,228 units, so the arms did diverge and
reconverge.

### The methodological finding: the four pinned gates cannot see this lever

**Every one of the four pinned values reproduces under the flag** —
206.869/`8a7737381238fa4d`, 159.09233022733062/`fa01012af1d559ae`,
159.07876040364795/`e28fba007f8031d4`,
164.0375677990678/`49f094d7e59a9008`, all `exactValid` and
`contractValid`, and the gate-1 depth list identical element for element
— while nine document fields move on g1 and sixteen on each mode-22
gate, including `acceptedMoves`.

That is not evidence for the lever. It is evidence that the four pinned
replays are attractors: the counters prove the search took a different
path and arrived at the same place. **A regression gate that a lever
deleting 15% of the neighbour visits cannot move is not measuring that
lever**, and the same warning applies to every future class (B)
candidate. The 16-cell matched-parent gate resolved exactly one cell out
of sixteen, and that resolution was negative; the endpoint the census
designed has very little power, and the honest way to report it is
"fifteen ties, one loss, no wins", not "neutral".

### The allocation lever: bit-identical, 15% of the allocator, and parity

`relaxed-row-buffer-reuse` recycles the candidate scorer's collision-row
buffer. The scorer wrote `Vec::new()` per call for ~1.80 rows and the
refinement loop then retired **two** buffers per iteration — the loser of
the paired probe, and the incumbent it displaced — straight back to the
allocator. The flag hands both to a four-slot per-lane pool.

The structural change the sizing note feared was not needed:
`MovedRowDelta` still owns a plain `Vec`, and only the two points where
the refinement loop *drops* one had to change. It is bit-identical by
construction — the buffer is cleared, the same values are pushed in the
same order, the terminal sort runs on the same slice, and nothing reads a
capacity — and bit-identical in fact: **all four gates, 3,271 and 3,252
fields compared, 6 differing, the executable hash and the five wall-clock
quartiles.** No work diagnostic moves.

Allocations removed, `profiling-allocator` builds, gross demand:

| stream | default | `+reuse` | removed | per candidate scan |
|---|---:|---:|---:|---:|
| m22 g2 | 50,455,078 | 42,881,288 | **7,573,790 (15.01%)** | 0.695 |
| m22 g2 bytes | 8,406,034,810 | 7,671,679,301 | 734,355,509 (8.74%) | |
| m20 g1 | 115,375,028 | 112,544,207 | 2,830,821 (2.45%) | 0.692 |
| m20 g1 bytes | 24,341,428,590 | 24,065,306,857 | 276,121,733 (1.13%) | |

The two streams agree to **0.4% on the per-scan rate** across a 2.7x
range of scan counts, two operators and two completely different
allocator mixes, which is what identifies the removed traffic as exactly
the refinement loop's two retired buffers. The m22 baseline reproduces
the census figure to two allocations in fifty million.

**And the wall clock does not move**: 1.0014 (m20 g1, 14 rounds), 1.0004
(m22 g2, 16 rounds), 0.9976 (coordinator at `work=20000000`, 16 rounds),
with 5/14, 8/16 and 10/16 rounds below parity. The arithmetic agrees
rather than contradicting: 7.57M allocation/free pairs at a
tcache-resident 20-30 ns is 0.15-0.23 s *thread-summed*, and the m22 g2
stream runs about 4.8 lane-seconds per wall second, so the whole prize is
30-50 ms on a 3.17 s stream — **1.0-1.5%, inside this box's per-round
spread.**

So the lever is kept as a default-off flag rather than proposed for the
default build: it is free to turn on, it removes real allocator traffic,
and it is not a speed claim on this hardware.

### Two things this chapter corrects

* The sizing note credited part of the per-scan allocation to
  `search_piece` cloning the row vector twice. **Both clone sites are
  unreachable on every stream measured here** — one is inside
  `if self.uses_directional_pressure()`, which the default backend never
  satisfies, and the other inside `if ENABLE_NFP_AXIS_MINIMIZER`, which is
  a `const false`.
* The m20 g1 allocation total is **115.4M, not the 72.2M** quoted before:
  that figure was taken on the `fast-constructor-*` stream and this one on
  the plain `jagua-experimental` build. Per-scan rate, not total, is the
  comparable statistic across builds.

### What this leaves

The relaxed lane's remaining semantics-preserving levers are the
fixed-side catalogue descents (15.4M/40.8M/69.4M, needing a slab and a
per-piece slot memo) and the `weights` `BTreeMap` descent per colliding
row. Both are class (A) and both are now known to be competing against a
noise floor of about 1.5% per measurement on this box, so neither is
worth building until there is a stream where the lane is a larger share
of the wall than mode 22's is, or a quieter box. **The class (B) door in
this lane is closed**: the one ordering that was cheap enough to be worth
trying removes a seventh of the work, changes the trajectory, and pays
8.968 mm for it.

Evidence, drivers, the sixteen quality cells, the coordinator work-budget
cells, the allocation counts and both timing campaigns with their
per-round rows: `docs/experiments/relaxed-lane-residual/` and
`evidence-stage2.json`.

## Coordinator v3: the ranked action queue, the compression sign, and the ladder as a phase

Full evidence, drivers and every table below as measured:
`docs/experiments/coordinator-v3/`.

The opportunity ledger established two facts about coordinator v2 that this
chapter acts on. The saturated run is **not** a fixpoint of the operator set -
it stops at 23-27% of its budget because it has run out of *keys it knows how
to name* - and the A/B/C refuted the prediction that a constructor feeder would
break that: arm B published 0 of 3 and stayed 42-53 mm behind, while arm C, one
short mode-26 ladder into the global legalizer tier, published on 2 of 3 seeds
and reached 169.251 mm.

Coordinator v3 is three changes to `search/portfolio.rs`, behind
`PortfolioSettings::coordinator_v3` (spec key `v3=1`), off by default.

### 1. The compression target had the wrong sign

v2's compression phase asked mode 22 for `depth + 0.8` - a **looser** bound than
the incumbent it already held - received an exact-valid answer and exited
`noResidue`, so the mode-31 tier behind it never ran. The A/B/C's control D
asked the same operator, from the same parent, for `depth - 0.3` and published
2.620 mm for 3.08M work units. v3's compression class asks for
`depth - COMPRESSION_RUNG_MM`: incumbent-relative, derived from the engine's own
smallest construction drop, never an absolute slack above the depth the parent
already holds.

### 2. The schedule was a single pass

The ledger found that on seeds 0 and 1 the final rank-0 state is born in the
*compression* phase, after the crossover phase has ended, so the run's best
state and its recombination operator never meet. v3 replaces the five-phase
sequence with one action queue that re-enumerates after every action. A state
born late re-enters compression, descent, the ladder and crossover until the
budget or a true all-actions fixpoint.

The queue's actions are the ledger's derived ones: **ordered** directional pairs
(`A->B` and `B->A` are different layouts and both are named), the constant cut
`0.5` first and then the **interface-band-derived** cuts outward from it, and
attempted-keys built from the two parents **in the order they are handed to the
operator** plus the cut's bit pattern - never from ranks, which is the ledger's
own pinned bug. Enumeration is bounded by construction: at most 21 actions per
iteration over the top-3 frontier, against the ledger's 360 on that frontier and
4,318 over the whole archive.

Ranking is `expected Δraw per action / expected cost per action`, with the cost
quoted as a multiple of **the protected phase-0 pipeline this run just paid
for** so that one prior table prices a 61-piece request and a 17-piece one, and
a wall budget and a work budget. The priors are the ledger's own §5 rows and the
A/B/C's arm C, and they reproduce the ledger's Δraw/M-evaluation ordering to
better than 10% (`5.39 : 2.09 : 1` for compression : descent : crossover). Each
prior is worth two actions of evidence; the run's own publications displace it.

### 3. Mode 26 is a scheduled class, priced honestly

The ladder class is two rungs of the separator's own relative contraction
quantum - the drop is `2 x depth x COUPLED_SEPARATOR_CONTRACTION_RATIO`, so no
millimetre is carried across requests - followed by the coordinator's global
legalizer tier on what it leaves. Its price before its first action is the
**largest** of the three arm-C spends the A/B/C measured, expressed against that
run's own protected phase; afterwards it is the larger of that prior and this
run's own worst ladder. There is no "unpriced operators get a free pass" clause,
which is the ledger's mode-20 finding turned into a rule.

Measured, over every ladder action at a 120M work budget: the prior is right to
within a factor of 2.9 in both directions and conservative in four of six
(`0.35, 0.39, 0.49, 0.59, 1.14, 1.22` actual over estimate). By contrast the one
class still priced by v2's `WhenDescendable` rule - diversify - estimates an m20
ticket on shapes-17 at 0.10 s and pays 1.18 s for it, **a factor of 12 on the
clock** to go with the ledger's four orders of magnitude in work units.

### What it measures

mixed-61, from the bare request, allowance `0.002`, work budget 120,000,000,
three seeds. Work-budget mode is deterministic and load-independent.

| seed | coordinator v2 | **coordinator v3** | Δ | arm C's post-drain probe |
|---:|---:|---:|---:|---:|
| 0 | 174.20812003998896 | **169.14057315694365** | **−5.068** | 169.251 |
| 1 | 176.05599999999998 | **169.92832830680420** | **−6.128** | 171.739 |
| 2 | 179.006 | **172.086** | **−6.920** | *(nothing)* |

All `exactValid` and `contractValid`, and all twelve work-budget layouts - six
v3 and six v2 - were re-confirmed by replaying them through mode 27 in a
separate process from a **pristine base-commit binary that contains no v3
code**: zero repair applied, zero violating pairs, fingerprint unchanged, raw
depth to the digit. **169.141 mm is a new best-from-request layout on this
request at this allowance**, and unlike the 169.251 it is reached **in
schedule** - the drain published nothing in any of the six work-budget runs and
the final publication's phase is `compression` in all six.

The budget statement moves with the depth. v2 stops at 23-27% of a 120M budget
on `keysExhausted`; v3 stops at **94.9 - 97.7%** on `affordability`.

Anytime curve, three seeds x three rounds, paired and interleaved:

| budget | median Δ (v3 − v2) | min | v3 better | v3 worse | v2 coordinator wall | v3 coordinator wall |
|---|---:|---:|---:|---:|---:|---:|
| 3 s | 0.000 | 0.000 | 0 | **2** | **4.23 s** | 2.71 s |
| 10 s | 0.000 | **−2.293** | 3 | 0 | **10.57 s** | 9.24 s |
| **30 s** | **−5.068** | **−6.920** | **9** | **0** | 14.16 s | 28.77 s |

**v2 overran its own budget at 3 s and at 10 s; v3 overran neither in any of its
27 mixed-61 runs.** That is the affordability rule: v2 asks "may I start?", v3
asks "can I pay for the worst version of this I have seen?".

Pooled class economics at 120M, over the three seeds:

| class | actions | published | Δraw | Δraw / M eval | Δraw / action |
|---|---:|---:|---:|---:|---:|
| compression | 31 | 21 | 18.159 | **0.2461** | 0.586 |
| ladder | 6 | 5 | 6.291 | 0.0662 | **1.048** |
| crossover | 22 | 3 | 5.337 | 0.0428 | 0.243 |
| descent | 10 | **0** | 0.000 | 0.000 | 0.000 |

The measured order is **not** the prior order, and the queue found that out
during the run. Compression's prior was right. Descent's - the ledger's second
highest - is wrong on this stream. The ladder's was too pessimistic: it is the
worst class per evaluation and the best per action, and on seed 1 it produced
5.486 mm of a 9.762 mm run. It pays on 2 of 3 seeds, the same shape arm C had,
on a different set of parents.

One trace line is worth the chapter. On seed 0 at 30 s, six consecutive
crossovers fail - the constant `0.5` twice and three derived cuts - and the
seventh, a derived interface-band cut at `0.539114160` in a 33.134 mm band,
publishes 1.736 mm; six consecutive compression actions on six successive
incumbents then take 177.770 to 169.141. Neither the cut nor the chain is an
action v2 can name.

### The negatives

* **3 s on mixed-61: 2 of 9 rounds worse, by 2.880 mm, all on seed 1.** At that
  budget the queue affords exactly one action and spends it on compression where
  v2 spends it on a descent quantum. The compression prior is confounded with
  *position in the schedule* - the ledger measured it on a state two publications
  deep - and the 3 s tier is where that assumption is load-bearing and wrong.
  From the second action on, the tight ask wins by 5-7 mm.
* **shapes-17: identical layout, 9.4x the coordinator wall at 30 s** (28.90 s
  against 3.06 s). 281 crossover actions across nine runs for 0.0034 mm. Each
  crossover publishes a rounding-scale improvement, that publication is a new
  archive member, the frontier changes and the ordered pairs regenerate. The
  queue is not stuck; its keys are worth 12 µm each.
* **triangle-20: 0.00279 mm worse, 9 of 9 rounds.** v3 makes the diversify class
  eligible only when the priced queue is empty, and on triangle-20 it never
  empties, so the constructor slice that published on half its arms under v2
  never draws a ticket.
* **No stopping rule.** The interval a global barren-action patience would have
  to sit in is *measured* rather than guessed: at least **8**, because the
  seed-0 30 s publication at action #13 came after seven barren actions, and at
  most **32**, because shapes-17's churn runs 33 barren actions between 12 µm
  publications. The constant inside `[8, 32]` is not measured and this round
  declines to fit one to nine runs on one request.

### Regression

Both binaries built from the same worktree - the pristine one from a detached
checkout of the base commit - and run through the four pinned gates: 206.869 /
`8a7737381238fa4d`, 159.09233022733062 / `fa01012af1d559ae`, 159.07876040364795
/ `e28fba007f8031d4`, 164.0375677990678 / `49f094d7e59a9008`. Compared as whole
documents with wall-clock and build-identity fields removed: **0 differences
over 3,261 / 3,242 / 3,242 / 3,242 fields.** The gates never enter the
coordinator; the argument is the default `false`, and this is the check.

Stronger than the gates, because it is the coordinator path rather than one
that never enters it: the `v3=0` arm of the work-budget battery spends
**32,393,757 / 31,957,935 / 27,938,867** units for **174.20812003998896 /
176.05599999999998 / 179.006** - the opportunity ledger's Part 1 table, digit
for digit and unit for unit, from a binary that contains the whole v3 queue.

Determinism, two processes per cell at `work=40,000,000`, whole documents:
**0 differing fields** on all three seeds for both schedules, with the work-unit
spend identical to the unit.

`cargo test --release --features jagua-experimental`: **1,244 passed, 0 failed,
2 ignored**, including six new coordinator-v3 unit tests.

### What this leaves

The 10 s tier is the open question. v3 is never worse there and better on one
seed, but the mechanism that produced the 5-7 mm - a long chain of cheap
compression actions after a crossover finally lands - needed 20 actions on
seed 0, and a 10 s budget buys 6.4 (58 actions over nine runs against 170 at
30 s). The ladder is unaffordable below roughly 10 s by its own honest
price, which is correct behaviour and not a tuning. So the 10 s answer is
probably not this ladder at all: it is either the m26 **port** (a compression
schedule at kernel frequency inside `move_sweep`, whose design and costing are
in `docs/experiments/mode26-rung-anatomy/`), or a cheaper crossover - mode 23
spends 5.7M evaluations per action here and returns 0.043 mm per million, the
worst ratio of any class that publishes at all, because its seam legalization
re-enters the protected 8-lane path.

The three v3 negatives are all one missing instrument: a rule that retires an
action class when its *own measured* yield stops justifying its price. The
shrinkage limits the damage; it does not stop the run. Sizing that rule - a
barren-action patience in `[8, 32]`, or better, a yield floor in millimetres per
unit of budget - is the smallest next step, and shapes-17 and triangle-20 are
the two requests that will tell it apart from a constant fitted to mixed-61.
## The compression schedule: the clamp bought one micron at a time, and the rollback that cost 97% of it

The mode-26 rung anatomy ended with a design: the clamped-sheet ladder is
expensive because it rebuilds a whole mode-0 pipeline per rung, but the
clamp it rebuilds for is **already a proxy-tier parameter** —
`boundary_penalty` takes the depth as an argument at all eleven of its
call sites and every candidate generator derives its sampling box from
the same scalar — so buying depth needs a clock, not geometry. The
opportunity ledger's A/B/C then cleared that design's gate: at the
coordinator's own 174-179 mm parents the clamped ladder publishes 2 of 3
and is the only mechanism that broke the saturation. This chapter is the
port, behind `compression-schedule`, off by default.

### The verdict, at equal work

Twelve pinned coordinator parents (one per seed, `work=120,000,000` from
the bare request, the ledger's own `0.002` allowance), both arms from the
same fixture at the same seed, allowance **33,413,789 work units** — one
measured mode-26 rung. Statistic: raw source depth of the best exact-valid
publication, parent as the floor for both arms.

| arm | publishes | median Δ | mm / M units | median operator work |
|---|---:|---:|---:|---:|
| one short mode-26 ladder | 10 / 12 | 0.876 mm | 0.168 | 14,755,710 |
| **the schedule** | **12 / 12** | **12.110 mm** | **0.623** | 17,481,265 |
| the schedule at 10% of a rung | 11 / 12 | 1.104 mm | **1.013** | **869,133** |

Paired per cell, the schedule beats the ladder in **12 of 12** cells with
a median advantage of **7.479 mm** — and also in 12 of 12 when it is read
at the control's *own measured spend* rather than at the shared
allowance, median **4.340 mm**. Every one of the twelve publications was
re-confirmed through mode 27 in a separate process from the pristine
default-feature binary: exact-valid, contract-valid, fingerprint
unchanged, zero pieces moved. The best cell reaches **160.985 mm**,
**8.266 mm below the previous best-from-request layout on this request**
(169.251 mm, ledger arm C).

At the design budget the port does what the anatomy hoped: **1.013 mm per
million work units against the ladder's 0.168**, for a median spend of
2.6% of a rung.

### The finding that was not predicted: the port's own rollback was 97% of its depth

The anatomy's piece (e) asked for "a rollback contract that survives a
moving depth". It was built, it is correct, and its trigger — 32 depth
steps without an accepted confirmation, chosen before any measurement —
is catastrophic:

| | rollback 32 | rollback 0 |
|---|---:|---:|
| publications | 8 / 12 | **12 / 12** |
| median Δ | 0.359 mm | **12.110 mm** |
| paired vs the ladder | **loses** 9 of 12 | **wins** 12 of 12 |
| median confirmations accepted | 128 | 1,838 |
| paired difference per cell | — | median **+10.962 mm**, 12 of 12 |

The mechanism is in the step rows: a compression frontier is
proxy-infeasible **82% of the time by construction** — that is what a
compression frontier *is* — so a rollback keyed on "the frontier has not
been publishable lately" fires almost every time it can, and the schedule
spends its budget descending 32 microns, being thrown back, and
descending again.

That is the anatomy's own headline about mode 26 (85.4% of arms abort on
a rollback, 75.5% of the arm wall) reproduced one level down, inside the
port built to avoid inheriting it. The generalisable rule: **a rollback
whose trigger is the normal state of the thing it guards is not a
guard.** The mechanism stays, tested, with `rollback_after_steps`
defaulting to `0` and the measurement in its doc comment.

### Two corrections to the anatomy, both arithmetic

**One exact confirmation costs 4.83 ms, not 0.491 ms.** The anatomy
called 0.491 ms "the hinge of the porting design" and budgeted the exact
tier at 2.0% of a 1.0 s slice. Measured over 23,176 confirmations here:
4.83 ms mean, 4.18-5.65 ms over cells. The anatomy's own phase table
implies it — an *accepting* confirmation asks all 1,830 pairs, which at
that round's 1,904.8 ns per `exactOverlapTest` is 3.485 ms before the 61
collision-polygon builds — and the 0.491 ms figure is the cost of a
confirmation that **fails**, exiting at the first violating pair. Every
one of that round's 25 samples was a rejection, because zero of its 171
arms produced an exact-valid state. The port survives the correction only
because of a clause the design did not name: a layout the proxy tier
already calls infeasible is never offered to the exact validator, which
suppresses 82% of the confirmations the cadence makes due.

**Every 171-179 mm parent arrives at the relaxed lane already
proxy-infeasible.** All twelve are `exactValid` and `contractValid`, and
the proxy tier sees 26-38 colliding pairs and 4-11 boundary violations at
each. `initialize_complete_state` snaps a warm start's rotations onto the
structured surrogate's 2.5-degree grid (`general_relaxed.rs:15397`), and
17 of seed 0's 61 poses are off that grid — all 61 of the 159.079 record
parent's are. The first exact-valid depth after that entry transform is a
median **0.448 mm worse** than the parent it came from. This is upstream
of both operators and identical for both, so it does not move the gate;
it is the largest single thing between this band and the next one, and it
is a protected shared path that four modes' trajectories depend on.

### Regression, and what the flag costs when off

All four pinned gates reproduce as **whole documents** against the
pristine binary — 3,263 / 3,244 / 3,244 / 3,244 fields, **0 differences**
— for the default-feature build *and* for the `compression-schedule`
build with the feature compiled in and unarmed. The feature adds one
`Option` field no existing caller constructs, one `#[cfg]`-paired call at
the top of `move_sweep` whose disabled half has no body, and one match
arm. Release suite: 1,238 passed / 0 failed with the feature off, 1,250
passed / 0 failed with it on.

### What this does not settle

The schedule is **one lane** where a mode-26 rung is eight, so equal work
is emphatically not equal wall and no wall claim is made here. The work
meter itself counts only the *narrow phase* of the exact tier
(`kernel::exact` increments past the bounds reject), so the exact tier is
24-52% of the schedule's wall and about 4% of its metered work; the
schedule's own cap deliberately over-charges by ~18x in the other
direction, which is why it stopped at 52% of the allowance in the
coordinator's currency and still won 12 of 12. `micro_legalize` was never
invoked — zero of 23,176 confirmations were refused, so at a one-micron
step a proxy-feasible layout was always exact-valid — which answers the
anatomy's second risk on this fixture and leaves the tier itself
untested. And nothing here is a schedule change: the coordinator has no
compression-schedule phase, and mode 34 is reachable only from an
explicit CLI mode in a build that carries the feature.

Evidence, drivers, the twelve cells per arm, the twenty-four independent
confirmations, the depth-versus-work curves and the record-line contrast:
`docs/experiments/compression-schedule/`.

## Coordinator v4 — the schedule becomes a class, the queue learns to stop, and the slice competes

Coordinator v3 shipped a ranked action queue and three measured negatives.
This stage is those three negatives, discharged, on the same request, the
same allowance and the same paired discipline. Merged-HEAD v3 is the
reference arm and it is **the same binary**: three portfolio spec keys
(`sched=`, `barren=`, `divq=`) select the schedule, so every A/B is two
processes of one build.

**mixed-61 from the bare request, work budget 120,000,000, three seeds:
169.141 → 163.927, 169.928 → 162.161, 172.086 → 164.004.** Six of six at
both measured budgets, every one `exactValid` and `contractValid`, every
one re-confirmed through mode 27 in a separate process from a pristine
base-commit binary that contains none of this code. **162.161 mm is a new
best-from-request layout on this request** — 6.967 mm below v3's 169.141
and 12.047 mm below v2's 174.208, at allowance `0.002` and therefore not
comparable to the 159.079 / 164.038 record lineage.

On the wall clock, paired and interleaved over nine rounds a tier, v4 is
**strictly better in 9 of 9 rounds at 10 s and at 30 s and not worse in a
single round at any tier** — including the 3 s tier, where v3 was worse
than v2 in 2 of 9. The 10 s tier is the one the port's data predicted
would move, and it moved past 174.208 on every seed: **173.575 / 171.362 /
176.162** against v3's 174.208 / 176.056 / 178.286. The prediction was
mechanical rather than statistical — a class costing a third of a
protected phase and publishing 1.1 mm becomes affordable at a budget where
the ladder never is — and the traces confirm it: the ladder makes 0
actions in the v4 10 s arm and the schedule class makes 9.

### The compression schedule is now a priced class, and it is the best one

The port left mode 34 reachable only from an explicit CLI mode. It is now
an action class offered over the same best distinct state the two mode-22
classes are offered over, and pooled over three seeds at 120M it makes 19
actions, **publishes on 17 of them**, and returns 20.292 mm for 34.9M of
the coordinator's units — **0.581 mm per million, twice compression's
0.298 and eleven times the ladder's 0.053**, at a twelfth of the ladder's
cost per action.

The slice is **nine rungs of the separator's own relative contraction
quantum**, walked one canonical grid unit at a time. Nine is a
reproduction, not a tuning: the port's cheap arm walked a median 1,568
one-micron steps, and `9 × 174.208 × 0.001` is 1.5679 mm, which is 1,568
steps. Expressing it as rungs rather than as the port's 3,341,379-unit cap
is what lets it cross a request — 1.80 mm on shapes-17's 200 mm parent,
0.64 mm on triangle-20's 70.7 mm one — and what makes the arm
deterministic without reading a counter, which a cap in the coordinator's
currency could not be, because that currency is zero when profiling is off
and a wall-budget run has it off.

**The pricing is the honest part.** The port's own §6.3 says the work
meter counts the narrow phase of the exact tier only, so the schedule's
exact tier is 24-52% of its wall and ~4% of its metered work. On the
port's twelve cells the same self-capped arm reads **307,767 to 3,343,739
units on the coordinator's meter — an 11x spread — and 3,341,665 to
3,356,020 on its own, a spread of 0.4%.** Extending the process-wide meter
was rejected on blast radius: every pinned work-unit number in this
repository is denominated in the current counter, including the ledger's
32,393,757 / 31,957,935 / 27,938,867 that v3 §6.1 reproduces to the unit.
So the coordinator charges the **larger of the two** into the class's price
ratchet — a price, never a spend, so the budget still advances on the
meter — and both numbers are in every action row. The result is the best
first-action estimate any class in the queue gets: actual/estimate
**0.991 / 0.966 / 1.013** on the three seeds, against the ladder's
0.39-1.33 and compression's 0.84-1.08.

### A stopping rule, sized from the interval rather than fitted

v3 §5.2 measured the interval a global patience would have to live in —
**at least 8**, or it cuts the mixed-61 30 s run that published after seven
barren actions; **at most 32**, or it does not cut shapes-17's 33-action
churn — and declined to fit a constant. `BARREN_ACTION_PATIENCE = 16` is
the **geometric** midpoint of `[8, 32]`, taken geometrically because the
quantity is a ratio, and it is simultaneously twice the largest productive
barren run ever measured and half the churn it has to cut. The loop exits
`patience` **with its queue still full**, which is a third exit cause
distinct from `keysExhausted` and `affordability`.

On shapes-17 at 30 s it takes the coordinator wall from **28.98 s to
19.06 s** — 9 of 9 runs exit `patience` where v3's 9 of 9 exit
`affordability` — and cuts crossover from 272 actions to 108. The price is
stated rather than rounded away: it cuts a 33-action productive barren run
by construction, and that costs **0.38 µm in three of nine rounds**. It
does **not** reach v2's 2.57 s, and this round says why: on that stream the
first publication is action #9, so nine actions precede any barren counter
at all and a patience of 16 has a 26-action floor whatever the budget.
Cutting the rest needs the first publication sooner, not the last barren
run shorter.

### A prior of zero is not a prior

v3 gave diversify `prior Δraw = 0.0` and scheduled it by an eligibility
rule that triangle-20 never satisfies. Zero is absorbing: a class ranked at
zero is never chosen, so it never earns the evidence that would displace
its prior, and v3's own "the prior is worth two actions" becomes
unfalsifiable for that one class. This round measured the number instead,
on all three requests rather than one — **10 constructor arms, 0.05826 mm,
all of it on triangle-20: 0.005826 mm per action.**

It also measured the price, and found that one price cannot do the job.
The ledger priced an m20 arm at 260-335 work units against 3.1 s of clock;
v3 §1.3 measured the same rule 11.7-12.0x wrong on the wall and left it.
Measured here on three requests, the diversify phase costs **0.067-1.224
phase-zeros in work units and 1.254-1.976 in seconds** — the same action,
priced 17x apart on mixed-61. So the class carries **two** priors, each the
worst case of its own currency, and it is the only class that does; a test
pins that asymmetry. The first diversify action of a shapes-17 run is now
estimated at ~2.0 s and costs ~1.2 s — **1.6x over**, where v3's rule was
11.8x under — and an overestimate is the right side to be wrong on, because
at a 3 s budget the queue now refuses a 4 s ticket on affordability instead
of buying it on an eligibility clause.

Ranking it is necessary and not sufficient, and this round says so: a prior
of 0.005826 mm never outranks crossover's 1.0923 mm at any budget this
engine runs at. So the queue additionally **auditions** one untested ticket
after eight consecutive barren actions — the floor of the same measured
interval, and a count the mixed-61 headline stream (longest productive
barren run 7) never reaches. The pair reads as one rule: *at eight barren
actions the queue buys a new basin, at sixteen it stops.*

On **triangle-20 at 30 s the 0.00279 mm regression is gone, exactly**: all
three seeds and all three rounds reach **70.72726178003285**, coordinator
v2's own depth to the digit, and the last publication's class is
`diversify` in 9 of 9 runs. At 10 s it is not gone — the run is ten actions
long and never accumulates eight barren ones — and this round declines to
fit a smaller constant to one request.

### The ablation

Three changes landed together, so the attribution is measured. One key at
a time, mixed-61, `work=120,000,000`, three seeds, one run per cell:
**`sched=1` alone reproduces the whole headline** — 163.927 / 162.161 /
164.004 — and `barren=16` alone and `divq=1` alone reproduce the reference
arm's depth, its iteration count and its work spend **to the unit**. That
is the designed behaviour and not a null result: on mixed-61 no barren run
reaches 16, so the patience never trips, and none reaches 8, so the
audition never fires. Read across the three requests: **the schedule class
buys the depth, the patience buys the wall, and the audition buys the
3 µm.**

### Regression, and what the reference arm proves

The strongest statement is not the gates. `v3=1,sched=0,barren=0,divq=0`
was run against the **pristine `5d6ce0c` binary** through the coordinator
itself, at `work=40,000,000` on three seeds, and compared as whole
documents: 3,405 / 2,770 / 3,483 fields, **29 differing in total, every one
of them `meteredCost` — a field this round adds that the pristine binary
does not emit.** No behavioural field differs and the work-unit spend is
identical to the unit, which means every affordability decision, every
ranking value and every action the queue took is the same. The four pinned
gates then reproduce as whole documents — 3,261 / 3,242 / 3,242 / 3,242
fields, **0 differences** — on this tree's default-feature build *and* on
its `compression-schedule` build. Determinism: two processes, six arms,
**0 differing fields** and identical work-unit spends. Release suite:
**1,250 passed / 0 failed** with the feature off and **1,262 / 0** with it
on, six new tests, each pinning a number this stage argues from.

### The 2.5-degree snap: opened, and left alone with a reason

The port named the warm-start snap as the largest single thing between this
band and the next (+0.448 mm median entry loss). This round opened it and
is not touching it, and the reason is not caution: **there is no flag to
put it behind.** `canonical_angle` is not a normalisation applied to a
representation that could hold something finer — the structured surrogate
catalog `SurrogateCatalogMode::StructuredGrid` enumerates
`i × SURROGATE_ANGLE_STEP_DEG` and nothing else, so an off-grid rotation is
a pose the proxy tier has no surrogate for. Removing the snap means
switching to `CurrentAssignment`, which is a different pressure model, a
different candidate stream and a different cost model — and every number
the port measured is a measurement of the structured tier. A flag whose
two arms run different pressure models is not an ablation; it is two
engines. The 0.448 mm is therefore **not attributable to `canonical_angle`
alone**, and settling it is a campaign, named here with its three line
references.

### What this does not settle

**v4 overran its own coordinator budget in 2 of 27 mixed-61 runs where v3
overran in 0 of 27.** One is a crossover this stage did not touch (1.9%
over at 10 s); the other is a schedule slice estimated at 1.95 s that cost
5.12 s (7.4% over at 30 s). For scale v2 overran by 41% at 3 s and 6% at
10 s on the same request — but this is a regression against v3's own
headline and it is the same weak number as everything else below.

The headline is one request and three seeds. On the other two the schedule
class publishes **nothing at all** — 0 of 29 actions on shapes-17, 0 of 37
on triangle-20 — because its 1.104 mm prior is a mixed-61 number, exactly
as crossover's 1.0923 mm is one. Worse, its price transfers in work units and not in seconds:
first-action actual/estimate is **0.97-1.01** at a work budget and
**2.54-2.59** on the same request's clock, 2.94-3.07 on shapes-17 and 5.1
on triangle-20. At a 10 s budget one action is 20% of the run, which is
the mechanism behind triangle-20's new ≤2 µm regression on one seed at
10 s and behind the 30 s overrun above.
`DIVERSIFY_AUDITION_BARREN` is not a spec key and so is argued from the
trace and the interval's floor rather than ablated. And charging the
self-cap prices *one class* honestly; it does not make the coordinator's
meter correct, so at a work budget v4 still gets slightly more schedule
than its own accounting says it paid for — reported in every action row
rather than smoothed.

Evidence, drivers, the 126 battery runs, the ablation, the twelve
independent confirmations and the whole-document reproductions:
`docs/experiments/coordinator-v4/`.
## The record line: a sub-grid clamp took 3.656 mm off the record, and the cascade's cost ordering starved its best instrument

The compression schedule's first invariant was that its step is one canonical
grid unit, because 1 µm is the finest depth change a *layout* can express —
`snap_mm` rounds every translation onto that lattice. The invariant is true of a
pose and false of the clamp. `strip_depth_mm` is a proxy-tier scalar that
`boundary_penalty` reads as a continuous number, so a sub-grid step is not a
finer *move*, it is a smaller increment of pressure per step; and because
`confirm_every` counts steps rather than microns, a quarter step asks the exact
tier four times as often per micron of descent and spends four times as many
repair sweeps getting there.

`step=` — canonical grid units, default `1`, inside the already off-by-default
`compression-schedule` feature — is that knob. On the port's own from-scratch
state at a fixed 20M units, seed 5, `past=1`: `step=1` published 159.102 with 64
accepted confirmations; **`step=0.25` published 158.668 with 190**. The coarse
steps (2 and 4 grid units) accepted **zero** confirmations in thousands of
steps — the frontier outruns the repair — and the relationship is not monotone,
because `step=0.5` also published nothing. The direction is mechanical; the
curve is a search.

That one arm put the **from-scratch line below the standing record**, and the
cascade that followed it took the line to **155.42229074464285 mm raw**,
`exactValid` and `contractValid` on the true 5.0/5.0 exact-clearance contract at
the record lineage's `''` 0.0005 tail. That is **3.656 mm below the previous
record** (159.07876040364792) and 0.422 mm above the 155 mm goal, and it is
reached **unaided**: 164.0375677990678 → 159.668 → 158.668 → … → 155.422 with no
record-line placement imported at any step. The pin replays through modes 27, 30
and 22 seeds 0-3 on the pristine default-feature binary — which contains no mode
34 at all — at **0 ULPs** from the declared raw with the fingerprint unchanged,
and it holds a finite negative on a declared battery of 30 further search arms
(the certification's `probeArms: 36` folds in the 6 replay arms above) including
mode 26 and mode 34 at three distinct step sizes — 0.25, 1 and 0.1, with
`step=0.25` also probed at a second work budget, which is four *specs* but not
four step sizes. That is a negative on the arms that were declared and run, not
a certified fixpoint: see `docs/experiments/record-line-cascade/README.md` §7
for what the battery does and does not cover.

### Mode 34 is an operator with a precondition, and the precondition is itself

Every barren stretch of this round has one cause. `initialize_complete_state`
maps warm-start rotations through `canonical_angle`, which snaps them onto the
structured surrogate's 2.5-degree grid; the resulting collisions are
*rotation*-induced and the schedule's repair is translation-only, so it never
recovers. A state mode 34 produced arrives proxy-**feasible** (0 colliding
pairs, 0.019 mm of entry loss) and the schedule ratchets on it. A state modes
22/33 produced arrives with 28 colliding pairs and 0.647 mm of entry loss, and
the schedule then confirms **nothing** — not at 10x the budget, not at 40 sweeps
per step, not at `confirm=1`, and not at any of eight seeds (22 arms, 0 below).
Pre-snapping the poses onto the 2.5-degree grid and legalizing with modes 30/31
does restore the ratchet — 37 accepted confirmations — but the round trip loses
0.391 mm to win 0.171 mm, so it is a confirmed mechanism and a negative trade at
this depth.

The consequence for scheduling is concrete: mode 34's tier belongs behind a
gate, not in every round.

### The cascade's cost ordering starved its most productive instrument for 555 arms

The cascade adopts on the first strictly-deeper publication and restarts the
round, and its tiers were ordered cheapest-first: mode 22 at 3 s, the flatten
grid at 2 s, then mode 26 at 44-88 s. The cheap tiers never went *barren* — they
published 0.001-0.002 mm, round after round — so the round always restarted
before reaching mode 26. **555 arms** later, the certification battery ran mode
26 against that same incumbent and **six of six arms came back below it, the
best by 0.628 mm**, which is 300x the round's going rate.

The general statement is that an adopt-and-restart cascade ordered by arm cost
starves any expensive tier whenever the cheap tiers are merely non-zero, and
"merely non-zero" is the *normal* state of a repair tier near a fixpoint. A
cheap tier's yield has to be compared against the expensive tier's yield per
round, not against zero. Mode 26's own yield is basin-shaped rather than steady —
6 of 6 at 156.091, then 0 of 12 over four drops and four seeds at 155.452 — so
the fix is a gate and a periodic concurrent sweep, not a permanent promotion.

### What this does not settle

The record fell to the *from-scratch* lineage, so the 159.079 record parent
itself is exactly where it was: probed at six step sizes, two budgets and two
seeds, all twelve arms returned its own depth to the digit, and its fixpoint now
survives the very knob that broke the other line open. Mode 23 crossover — the
brief's other instrument — was never reached *inside* the cascade, because no
round ever went barren above it, and run separately against the certified
fixpoint it is barren too: 24 arms over two ancestors, both record co-states,
three cut fractions and both directions published 13 layouts and **none** below
the incumbent, the best 2.7 mm above it. The 155 mm goal is 0.422 mm away and the final state
holds a finite negative on the declared battery, so closing it needs an
instrument this round did not fire. And every number here is one request, one fixture, work-budgeted or seeded
arms on a shared box, with no wall-clock claim made anywhere.

Evidence, drivers, the nine pinned states, the step sweeps, the three negatives
and the two certifications: `docs/experiments/record-line-cascade/`, with the
pins under
`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-line-cascade/`.

## The orientation floor was still a floor, and the repair tier — not the entry — was the wall

The previous entry ended on a finite negative over a declared battery and a
clear demand: the last
0.422 mm needs "an instrument this round did not fire rather than more of the
same". Four were fired. They compose, they are all cheap, and every one was
found by reading a diagnostic the engine was already emitting rather than by
widening a grid. The record moved **155.42229074464285 → 155.26442950832842**,
0.158 mm, in 7,204 arms.

The fourth is the one worth carrying forward, because of *when* it was found:
after the third had run its whole battery to a negative. That battery was
honest and it was also a statement about the four instruments in it, and
inventing a fifth entry family took another 0.066 mm out of it in eighteen
rounds. This is why the rounds no longer say "certified fixpoint": the claim
the batteries actually support is "none of *these* instruments found anything",
and this round walked past four such negatives.

### The floor's own symptom had already been written down

The round that moved the ladder floor from 0.02 to 0.0032 degrees reported its
own residual: of 40 accepted rotations, 27 sat *on* the new floor and 13 one
rung above it. A distribution that piles on the floor is what a floor placed
above the useful band looks like, and it said so — "the same argument that
justified this change justifies testing another two rungs down".

It is one rung down, not two, and the arithmetic says which. A rung `d` moves a
vertex at radius `r` by `r · d · π/180`. At 0.00128 degrees that is 0.00223 mm
on a hand-sized 100 mm radius and 0.00120 mm on this request's depth-setting
family — one to two 0.001 mm pose quanta, the edge of expressibility. The next
rung of the same 5/2 ratio, 0.000512, travels 0.00089 mm on the *largest* piece
in the request, below one quantum, so the stream would be emitting angles the
grid rounds away. The floor is now at the grid rather than above it, and this
particular lever is spent.

The A/B on the battery-negative pin is the whole argument in one table: the same
22-arm flatten grid, nine rungs versus ten, **0 below versus 2 below**. The
winning arm accepted exactly one pose — `acceptedAnchorLocal = 0`,
`acceptedStation = 0`, one piece rotated by exactly +0.00128 degrees — and the
old ladder could not have produced it by any *sequence* of its own rungs: in
micro-degrees the nine old rungs generate a lattice of spacing 12.5, and 1280 is
102.4 of those. That is a lattice fact, not a search observation.

### The entry grid was three pieces deep and the frontier was seven

Printing the frontier stack (`drivers/frontier.py`) was the round's cheapest
measurement and its most useful. Ranks 1–7 sit inside 0.040 mm; rank 8 is at
0.171. The cascade's frontier-flatten grid ran {0.0005 … 0.01}, which perturbs
ranks 1–3 and never touches 4–7 — the grid could not express the move the state
needed. Extending it to 0.2 mm turned a round that found 3 arms below out of 109
into one that found 25 out of 120.

### Mode 33 throws away the repairs it has already made

That still left a fixpoint at 155.4087, of 164 arms. The diagnostics named the
wall in one field pair: `componentsRepaired: 1, componentsRefused: 1`, then
rejection of the whole arm. Mode 33's repair is all-or-nothing — one refusing
violation component discards every component the pass already placed — and the
entry was never the problem, since the entry had reached 155.3787.

So the same entries went to the tiers that do not enumerate insertion orders at
all. Modes 30 and 31 push the whole layout under a displacement cap, so a
component that will not re-place is a push rather than a veto. On the 164-arm
fixpoint: **14 of 28** and **13 of 28** arms strictly below, against 0 of 28 for
mode 27 (which is the probe authority and never repairs). The productive deltas
are an order of magnitude deeper than the re-insertion tier's — 0.1–0.3 mm
rather than 0.001–0.03 — which is the same statement from the other side. The
entry and the repair are independent choices, and the previous cascade had been
varying only one of them.

Naming that tier H and putting it in the interleave took the next round from
3 arms below out of 138 to **76 out of 198**, and it supplied ten of the
eighteen steps in that line.

### Every entry family on this line was a translation

Separating the entry from the repair immediately asks what *else* an entry could
be, and the answer had been sitting in plain sight: the frontier flatten, the
rank nudges and the k-deepest nudge all move pieces along the depth axis. The
orientation freedom was only ever reachable from *inside* modes 32 and 33, as a
candidate stream — and that stream can only perturb the pieces those modes
themselves ejected.

Tier I rotates the k deepest pieces **in place**, about each one's own
transformed bounding-box centre, by rungs drawn from the ladder itself. On the
battery-negative pin at 155.33041597699957 — a state that had just survived 132
certification search arms (plus its 6 replays, for `probeArms: 138`) and 110
further compositions — 80 rotation-entry arms
published 3 below. The 0.0006 mm is not the point. The point is that tier I then
broke **three consecutive fixpoints** nothing else could touch (rounds whose
only arms below were rotation entries, 4, 4 and 2 of them), and the state the
third handed on had **44** arms below. Thirteen of that cascade's eighteen
adoptions are rotation entries and the other five are the flatten and
legalization grids re-opened by them. The entry families are not substitutes;
they are a cycle, and the cycle carried the last 0.066 mm at up to 0.008 mm a
round — twenty times the rate the record-line round's cheap tiers ground at.

### Two prior findings that did not carry, and one process knob

**Mode 32 is not the unproductive tier here.** The previous round measured 4 of
4 sub-record publications for mode 33 and none for mode 32, and explained it by
the vertex cover. The explanation is intact and the measurement does not
generalise: across this round's cascades mode 32 took **97 of 352** arms below
the incumbent against mode 33's 66 of 352. "Mode 32 is unproductive" was a fact
about a basin whose conflicts were partner-blocked.

**Deferred credit, then frequency.** The cascade no longer restarts on the first
improvement; it runs every tier of a round to completion and adopts the round's
strictly-best publication. That removes the ordering bias the previous round
diagnosed — neither cheapest-first nor mode-26-first works — and exposes the one
hiding underneath it: *cost*. Modes 26 and 34 were 70% of a round's seconds for
0 of its adoptions. The answer is frequency rather than deletion (deletion is
how the previous round lost mode 26 for 555 arms), and moving the barren tiers
to every-Nth-round took a round from 349 s to 111 s. The knob cuts both ways:
mode 22 was 0 of 24 below on one state and **48 of 48** on the states the
legalization tier had just moved, so it went back to every round. A tier's yield
is conditional on what the previous round did to the state, which is exactly
what a fixed schedule cannot see.

### The negatives, and what the round does not claim

Mode 34 is inert on this whole lineage — 48 arms across eight step/budget specs,
two seeds and three parents, every one returning its parent's depth to the
digit — and the schedule's own block says why: `parentProxyFeasible: false`,
35 colliding pairs, and a `startDepthMm` 0.825 mm above the incumbent, all of it
the 2.5-degree `canonical_angle` entry snap. Walking around the snap still costs
more than it pays, now measured at 155.4 as well as at 156.9: the regrid probe
moves 49 of 61 poses, loses 0.779 mm on entry, and mode 34 then *does* ratchet
(155, 294 and 467 accepted confirmations against 0) to 155.604 — 0.195 mm worse
than the incumbent it left. Mode 23 crossover is barren against a pool it should
have liked: seven same-lineage siblings inside 0.09 mm, five cuts, both
directions, 70 arms, 0 below. The k-deepest nudge is barren against both the
re-insertion tiers (32 arms) and tier H (60 arms). Modes 33 and 30 are
seed-invariant, 18 arms each.

The final state replays `exactValid` and `contractValid` at **0 ULPs** on the
**pristine base-commit binary**, which knows nothing of the new rung — the
ladder change is what found the state, not what verifies it — and it holds a
finite negative on a declared battery of 132 further search arms (plus 6
replays, for `probeArms: 138`) including tier H's own grid and both ladder
generations. All four pinned gates hit, and the whole-document
comparison of the two binaries differs in **0** of 3,262/3,243/3,243/3,243
fields, which is the required result: the gates are modes 20 and 22 and neither
enters the orientation stream.

The 155.000 mm threshold is **not** reached; the gap is 0.236 mm, down from
0.422, and the last cascade was stopped while still adopting rather than at a
fixpoint, so no claim about the remaining distance is made either way. What the
round claims is narrower and reusable: all four levers were found by reading
diagnostics the engine already emits — an acceptance histogram piled on a
constant, a frontier stack four times wider than the grid probing it, a
`componentsRefused` counter sitting next to a `componentsRepaired` one, and the
observation that every entry the line had was a translation — and not one of
them was a parameter of the search. The corollary is the process finding:
a finite negative bounds the instruments in the battery and nothing else, and
the way past one is to add an instrument, not arms. Everything here is one request, one contract,
work-budgeted or seeded arms on a deliberately oversubscribed box, with no
wall-clock claim made anywhere.

Evidence, drivers, the thirty-five pinned states, the two certification
batteries and the eight negatives: `docs/experiments/orientation-floor/`, with
the pins under
`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/orientation-floor/`.

## Coordinator v5, item 1 only: the self-meter debits the budget it prices, and the other three items are named rather than faked

> **Corrected by the chapter below** ("The budget debit binds at 40M"). Keep
> this chapter for the record, but do not read its measurement section as
> evidence. Sol review 6 §1 checked the battery it rests on and found the arm
> ran with `v3=0`: the coordinator's v3 loop, the schedule class and mode 34
> were all switched off, so the "identical depths in all four combinations"
> below is a true statement about runs that never executed a single line of
> the code this chapter is about. The paragraph beginning "The honest result
> item 1 asked for" is wrong in its conclusion, not merely thin: rerun
> properly, the debit binds at 40M and costs 4.376 mm on one seed of three.

Sol review 5 §2 gave four items. This round did one of them — the budget-debit
bug at `portfolio.rs:3438` — and verified it; it did not attempt the other
three, and says so rather than presenting a partial result as the full one.

**The fix.** `schedule_self_cost_units`'s doc comment said the quiet part
outright: "The charge is a *price*, never a spend: the budget still advances
at the meter's own rate." Under a work budget that let a class whose own
meter reads up to 11x the coordinator's global counter (§6.3's twelve gate
cells) buy more of itself than the nominal budget allowed, because the number
gating every later affordability check never saw the higher price.
`BudgetMeter` now carries a `self_metered_debit` accumulator; the call site
that already computed `cost = metered_cost.max(self_metered_units)` for
ranking now also calls `run.meter.debit_self_metered(metered_cost, units)`,
so `work_units()` — and so `spent_fraction`/`remaining_to`/every
affordability check — reads `max(global_meter_delta, operator_self_units)`,
not just the global delta. No-op under a wall budget and for every action
that never reports a self-metered charge, by construction.

**Verified, not just built.** The four pinned gates rebuilt with the fix hit
bit-for-bit — same four depths, same fingerprint prefixes, `ALL_PASS: true` —
confirming the fix is inert wherever the gates run (modes 20/22 never report
a self-metered charge). All 26 `portfolio::` unit tests pass with
`compression-schedule` compiled in. A paired baseline-vs-fixed comparison at
`work=120,000,000`, mixed-61, seeds 0/1/2, with and without the
`compression-schedule` feature, produced identical depths in all four
combinations — 174.208 / 176.056 / 179.006 — because every one of those runs
spent well under its cap before the self-metered debit could matter. A
warm-start probe from the pinned 159.092 record parent at 40M work units was
inconclusive (its output matched the cold-start seed-1 number, so it likely
never engaged the intended warm path).

**The honest result item 1 asked for.** No headline number moved, in either
direction, in any run this round executed. That is not "the bug is not real"
— the fix is real, targeted at exactly the code path the review named, and
proven not to disturb any pinned or unit-tested behavior. It is that every
scenario reachable in the time available ended with the coordinator stopping
on `KeysExhausted`/`Affordability` against its own priced queue before the
debit could be the deciding factor — consistent with this same document's
orientation-floor finding that mode 34 is now inert from a generic starting
point (`parentProxyFeasible: false`) on this lineage. The debit bug and item
2's eligibility-prior gap are the same fact from two sides: a class this
rarely both eligible and load-bearing at the budget boundary is hard to catch
red-handed with a bare-request, 3-seed, 1-round battery. Reproducing the
exact conditions of the original v4 battery (base commit `5d6ce0c`, four
merged rounds before this one) was out of scope for the time this round had.

**Items 2, 3, and the rest of 4 were not attempted.** Three-level priors and
the wall batch are each a multi-file subsystem with their own tests and gate
re-verification, not a same-session addition on top of item 1. The
anytime-curve battery (3/10/30s, three corpora, three seeds, three rounds,
paired), the two-process determinism check, and the full `cargo test
--release --features jagua-experimental` suite were not run; a truncated
stand-in for any of them would misrepresent a measurement this project
treats as load-bearing, so none is reported.

Evidence, the fix diff, the gate output, the unit-test log and all four
battery documents: `docs/experiments/coordinator-v5-budget-debit/`.

## The budget debit binds at 40M: the first round's battery ran with the code switched off, and the honest number is a 4.376 mm regression that is the budget being enforced

Sol review 6 §1 accepted coordinator v5's debit and rejected everything that
had been said about it. Both halves were right. This chapter is the corrected
round: the ordering fix the review asked for, and the measurement the previous
chapter should have had.

**The retraction first.** The previous chapter's evidence is one battery, and
that battery's only arm carries `"v3": false` — spec
`work=120000000,cells=13:15:17:19,v3=0`, printed in every row of
`docs/experiments/coordinator-v5-budget-debit/evidence/battery-fixed-sched.json`.
With `v3=0` the coordinator's v3 loop does
not run, so the schedule class does not run, so mode 34 does not run, so
`schedule_self_cost_units` returns `None` and `debit_self_metered` is never
called. The depths 174.208 / 176.056 / 179.006 are correct numbers about runs
that executed no part of the code under test, and the conclusion drawn from
them — "no headline number moved in either direction", "every run in reach
stopped on its own priced queue before the debit could be the deciding factor"
— is false. Under the true v4 configuration the debit is the deciding factor at
40M on **every** seed.

**The ordering fix.** `run_operator` is now a four-step transaction — dispatch,
determine the charge, debit, then stamp — where before it archived, published
and wrote its call report *before* returning the self-cost to `v3_loop`, which
is where the debit was applied. The old order put an action's own charge on the
next action's readings: `birth_work_units`, `PublicationEvent::work_units` and
`OperatorCallReport::work_units` were all one debit behind. `debit_self_metered`
is now `u64` end to end (`operator_self_units.saturating_sub(global_meter_delta)`,
no `f64` in the path), returns the extra it applied, and holds the wall-budget
no-op itself rather than trusting each call site to remember it. Six named
tests plus two more pin the arithmetic; two drivers pin the ordering on real
runs.

**Finding 4, measured rather than asserted.** Across nine paired 40M cells,
every debited call satisfies `workUnits == globalUnits + debitedUnits` (18/18)
— an identity the old ordering could not produce, since it computed
`work_units` before the debit and so could only emit `workUnits ==
globalUnits`. Stronger, because the fixed and unfixed arms are bit-identical
run prefixes: for every layout both arms produced, the difference between their
publication and `birthWorkUnits` stamps is the cumulative debit *through the
current call inclusive*, never the exclusive sum the old ordering produced —
**36 of 36 comparable stamps, 0 of 36 matching the pre-fix identity**. Seed 2's
four debits of 2,090,715 / 1,998,160 / 2,252,965 / 2,470,770 show up as stamp
deltas of 2,090,715 / 4,088,875 / 6,341,840 / 8,812,610.

**The honest number.** True v4 (`v3=1,sched=1,barren=16,divq=1`) on a
`compression-schedule` build, mixed-61 from the bare request, seeds 0/1/2,
three paired interleaved rounds against `f32c629`:

| budget | seed 0 | seed 1 | seed 2 |
|---|---|---|---|
| 40M fixed | 169.891 | 171.362 | **170.155** |
| 40M unfixed | 169.891 | 171.362 | **165.779** |
| 120M fixed | 163.927 | 162.161 | 164.004 |
| 120M unfixed | 163.927 | 162.161 | 164.004 |

At 40M the fix costs **4.376 mm on one seed of three** (median 0.0, mean
−1.459 over nine cells) and buys strictly fewer actions on all three. At 120M
nothing moves. Every cell reproduced exactly across all three rounds.

**Why that regression is the instrument working.** The unfixed binary still
reports `actualCost` and `meteredCost` per action, so the debit it discarded is
recoverable. At 40M its *true* spend is 41,805,185 / 41,188,355 / 51,328,640 —
Sol's counterfactual from the pinned v4 trace was 41.81M / 41.19M / 51.33M, and
these are independent runs on a different commit. At 120M: 122,358,786 /
121,613,866 / 126,516,058 against Sol's 122.36M / 121.61M / 126.52M. **Nine of
nine unfixed runs at 40M overran their budget, by up to 28.3%; none of the nine
fixed runs did.** The 165.779 was bought with 51.3M units against a 40M budget.
The two seed-2 runs are identical action for action and metered unit for
metered unit through iteration 10; then the fixed run stops on `affordability`
at 39.1M of 40M while the unfixed run, whose meter reads about 30M at the same
instant, buys four more schedule slices — each of which publishes — down to
165.779. The regression is not a loss of search power. It is the difference
between a budget and a suggestion.

**And the control that settles it.** Give the fixed arm 52M — above every
unfixed *true* spend at the 40M point — and compare at matched true cost. Seed
2's unfixed 40M run spent 51,328,640 true units and reached 165.779. Seed 2's
fixed 52M run spends 51,339,455 and reaches **165.779**: two runs 10,815 units
apart, 0.02%, on exactly the same depth. The fix costs no search quality at
all. It costs only the ability to spend work the budget did not authorise.

**The wall curves, and the two negative results worth having.** 3/10/30 s,
three seeds, three paired rounds, 54 runs: thirty self-metered mode-34 calls
fired under a wall budget and **not one debited a unit**, which is the no-op
observed rather than asserted. The cells that differ are the box — the
within-arm round-to-round spread is 5.709 mm mean at 10 s and 4.276 mm at 30 s
against a between-arm paired difference of 0.703 mm and 0.455 mm. Separately,
running Sol's `barren=1` literally produces **zero** self-metered calls on all
three seeds and depths 8–10 mm worse than v4's: it is a patience of one, not a
boolean, and a battery run that way would have repeated the previous round's
mistake in a new costume. That is why the main battery uses v4's actual
`BARREN_ACTION_PATIENCE = 16`.

**What it still does not do, and this matters for the roadmap.** The debit is
charged *after* the action it prices, so one indivisible action can still
overshoot. Completely: 0 of 9 fixed runs exceed the cap at 40M and 0 of 6 at
52M, against 9 of 9 and 4 of 6 unfixed; at 120M **3 of 9 fixed runs do** — the
same seed-1 cell in all three rounds, 121,474,651 against 120,000,000, +1.23%
— against 9 of 9 unfixed at up to +5.43%. The fix bounds the overrun to at most one action's own debit rather than
removing it, and Sol review 6 §1 finding 3 names the instrument that would —
preflight/p95 pricing, shorter quanta, or a deadline-aware batch. It is also
explicitly a no-op under a wall budget, so it is **not** what would have caught
the 2/27 wall overruns; the wall curves confirm that empirically, below. And
the class it now correctly charges was *productive* on seed 2 — every extra
slice the unfixed run bought published — which is an argument for item 2's
eligibility prior deciding *whether* to schedule mode 34, not for making its
price a lie again.

Evidence, drivers, both suites, the gate documents, the retraction index and
every battery: `docs/experiments/coordinator-v5-budget-debit/`, round 2 under
`evidence/round6/`.
## CurrentPoseOverlay: isolating the snap found a bug the campaign had to catch first

Sol review 5 §3 asked for `StructuredGrid + CurrentPoseOverlay` before any
`CurrentAssignment`/`DirectionalPenetration` comparison is trusted, precisely
so the `+0.448 mm` entry-damage claim could be measured without also swapping
the catalogue, the pair-NFP table and the pressure model the way
`CurrentAssignment` does. This round built it, ran the A/B/C campaign, and the
first honest result is that **the overlay initially did nothing at all** —
not because it was inert by design, but because it was inert by bug.

`GeneralRelaxedSettings::current_pose_overlay` (off by default, compiled only
under `compression-schedule`) seeds `initialize_complete_state` with the
parent's continuous rotation instead of `canonical_angle`-snapping it, and
layers a small per-piece map onto the `StructuredGrid` catalogue so that pose
resolves. (The v5 round layered it onto a *clone* of that catalogue; Sol review
6 §2.2 rejected the clone and the next chapter replaces it — 8 ms of setup per
mode-34 invocation that the ten-second path cannot absorb.)
`build_surrogate_catalog`'s grid branch, and every
candidate `random_candidate`/`seed_angle` can still propose, are untouched —
every consumer of `catalog.orientations` in the file is a point lookup, never
an enumeration, so extra entries in the clone change nothing about what a
fresh grid build produces. The first campaign run against fifteen parents
(the twelve compression-schedule port parents plus the three true-contract
pins) reported **bit-identical** entry loss between the grid and overlay arms
on all fifteen, despite the overlay correctly counting 8-50 off-grid pieces
per parent — including **49 of 61** on the `155.4223` pin, the exact number
the record-line-cascade evidence already reported for that parent
independently. Bit-identical results with correctly-counted overlay entries
is the signature of dead code, not of a real null result, and that is what it
was: `derive_rotation_key`'s non-directional branch calls `canonical_angle`
**unconditionally**, so every lookup re-snapped a placement's key before it
ever reached the overlay's own entries, regardless of what continuous angle
`initialize_complete_state` had correctly seeded into the placement itself.
The fix, `continuous_rotation_keys`, threads the overlay flag into rotation-
key derivation at the six call sites that fed a `directional` bool into it —
carefully kept separate from `uses_directional_pressure()`'s branch-dispatch
checks, so the overlay still never switches the pressure model or the
collision backend. A regression test pins the resolution on a hand-built
continuous-rotation fixture. (That test was not a regression test for this
bug — it used symmetric, well-separated squares and passes against the bug.
Sol review 6 §2.1 caught it; the next chapter replaces it.)

With the fix in place, at equal work (compression-schedule's own 3,341,379-
unit design slice) on all fifteen parents: entry loss falls (median **-1420**,
11 of 15 reduced) and boundary violations fall (median **-2**, 11 of 15
reduced, none increased) — but the proxy tier's own collision-*pair* count
rises on **14 of 15**, every time it moves at all. Not zero, not a clean gain:
a trade along two axes of the same entry measurement, exactly the range Sol's
review warned the number could fall in. Downstream, arm B appeared to publish
more (12 of 15 parents against 9 of 15, 5.984 mm total drop against 4.136 mm)
at the same work budget and within 1% on queries/second — **a claim the next
chapter retracts**: that campaign ran `rollback=32`, a configuration the
compression-schedule port had already certified as costing ~11 mm of published
depth and which neither the schedule's defaults nor the coordinator's mode-34
call site use. Rerun at `rollback=0` the direction reverses and the effect
vanishes into noise. The prize the
review actually named — an `m33`/`m22`-produced state passing
`parentProxyFeasible` under the overlay where it failed under the grid —
did not land: **zero of fifteen** parents flip, because none of the fifteen
were close enough to the feasibility boundary (26-46 colliding pairs on
every one, both arms) for a sub-1.25-degree correction to cross it. Arm C
could not be run at all: mode 34 is reachable only through the coupled
separator's gate, which unconditionally requires the structured pressure
model for both its arms regardless of which persistent-vacancy mode is
asked for, and the fixture-loading path's settings check rejects any sheet-
depth override outright — so there is currently no CLI-reachable way to ask
the other engine to compress a parent at all, a sharper and more general
statement than the catalogue/pair-NFP/pressure-model entanglement the review
already named.

Four pinned gates hit on the unmodified gate binary
(`jagua-experimental` only, the flag compiled out entirely); a whole-document
diff against a binary built from the pre-round commit differs in exactly the
build/run artefacts expected (executable hash, source-tree hash, worktree
status, wall-clock quartiles) and in nothing else. The full suite passes,
`EXIT=0`, 55 binaries, 0 failures, the known-flaky eviction test included on
the first attempt.

Evidence, drivers, the campaign table, the gate runs and the regression test:
`docs/experiments/current-pose-overlay/`.

## The overlay corrected: a retracted downstream claim, and a `+9` that is conservatism

Sol review 6 §2 (`docs/sol-review-6-premerge-v5.md`) returned the overlay
branch **MERGE CON CORREZIONI** with seven named findings. Applying them cost
one published claim and turned one open question into a measurement. Both are
worth recording, because the pattern — *a result that survives its correction
in shape but not in sign* — is the thing a campaign has to be able to detect
about itself.

**The regression test did not test the regression.** The v5 round's only
overlay test used two symmetric, well-separated squares, so both arms produced
identical numbers whether or not any lookup ever reached the overlay. Sol
called it, and it is checkable rather than arguable: re-introducing the
never-looked-up bug verbatim and running all seven overlay tests, the v5 test
**passes** and three of this round's six new ones **fail** — precisely the
three that assert a lookup returned the overlay's shape rather than the grid
snap. The new fixtures are asymmetric and interacting (a 30x30 L with a 22x22
bite, and a 26x5 bar, at `13.37°` against a `12.5°` snap), and they cover every
lookup path (`rotation_key`, `surrogate_key`, `memoised_surrogate_key` on both
a miss and the cached hit, `oriented`, `local_shape_bounds`), both free scan
bodies, `score_state`, and mode 34 end to end. A test that cannot fail against
the bug it is named for is not a regression test, and the cheapest way to know
which kind you have is to put the bug back.

**The deep clone is gone, and it was 8 ms.** Installing the overlay used to
clone `catalog.orientations` — every polygon, triangle, cell-axis set, pole and
cell index for all 144 grid angles of every geometry class — in order to add
8-50 entries. It now takes `Arc::get_mut` on the catalogue the call already
sole-owns and moves the surrogates in. Measured with the engine's own new
`currentPoseOverlaySetupMs` meter, same instrumentation on both sides, on the
campaign's fifteen parents: **median 7.997 ms → 0.323 ms**, a median 23.7x, and
the one grid-native parent correctly shows no difference on either arm. That is
0.08% of a ten-second envelope per mode-34 invocation, recurring once per
basin the coordinator schedules.

**The downstream claim does not survive `rollback=0`.** The v5 campaign ran
`rollback=32`. The compression-schedule port had already certified that arming
the rollback costs a paired median of ~11 mm of published depth, which is
exactly why `CompressionScheduleSettings::default()` and the coordinator's own
mode-34 call site both set it to zero — so the v5 downstream numbers were
measured on a configuration that does not ship. Rerun at `rollback=0` with
coordinator v4's settings written out in full, on the same fifteen parents at
the same budget: both arms publish far more and far deeper (16.3 and 18.6 mm of
total drop against 4.1 and 6.0), and **the direction reverses** — the grid arm
now publishes on 13 of 15 parents against the overlay's 12. Paired on the depth
each arm actually reached, the overlay ends deeper on 7 parents, shallower on
4, tied on 4, median delta **0.000 mm**, sign-test p ≈ 0.55. The correct
statement is that the overlay has *no measurable downstream effect* at the
shipping rollback setting, and that "publishes more" was an artefact. The
entry-side numbers are untouched by the rerun and reproduce to the bit, as they
must: they are taken before the schedule's first step, where no schedule knob
can reach them.

**The `+9` is conservatism at the contract boundary, not inaccuracy.** The
review would not accept "the expected price" for the collision-pair rise until
each new pair was classified, and named the four things to measure. They are
now a diagnostics field, and the answer is unambiguous. Across the fifteen
parents the overlay adds 194 pairs and removes 73. **All 194 added pairs sit
1-2 µm from the envelope-feasibility boundary** — the same band, to the micron,
that the pairs the *grid* proxy already flags occupy; not one is outside it.
**67 of the 73 removed pairs** are pairs with 0.1-1.0 mm of genuine envelope
slack that the grid proxy was flagging wrongly, median 0.113 mm. The overlay
never drops a real conflict. This is the 5.0/5.0 exact-clearance contract and
these parents are compression outputs, so their close pairs sit *on* the
contract with 1-2 µm of envelope clearance; at that separation any
pole-and-triangle proxy disagrees with the exact tier somewhere, and the
continuous resolution disagrees only there while the grid resolution also
disagrees a hundred times further out. What the classification cannot do is
call the overlay *accurate*: an exact-valid parent has no real conflicts to
catch, so that column is zero by construction. Deciding that needs a parent
that actually conflicts — the causal sweep around the proxy-feasible boundary
the review asks for next.

**Two negatives on this round's own work.** The counters were separated —
`currentPoseOverlayEntries` is a catalogue size that collapses duplicate
`(geometry_class, angle, mirror)` keys, `currentPoseOverlayOffGridPieces` is the
placement count the snap would have damaged — but on all fifteen parents the
two are equal, so the fix moves no published number and its divergence is
covered by unit test only. And the flag-off wall A/B (12 paired interleaved
rounds, three binaries, both gate streams, `gateMisses: 0`) finds **no
significant difference anywhere**: the overlay commit costs at most +0.5% on a
26-second mode-20 stream against the pre-overlay baseline at p = 0.146, and
making the predicate lane-local rather than a by-value helper moves it +0.06%
at p = 0.77. The lane-local form is kept for clarity and because it is what
makes the lookup test able to assert the bit — not because it was measured
faster, and it is not claimed to be.

The seam stays what it was: **off by default, not enabled in the coordinator**,
now with a regression suite that fails against its own bug, a setup cost that
is 23.7x smaller, a downstream claim retracted, and the entry-side price
classified.

Evidence: `docs/experiments/current-pose-overlay/` (README §0.2-§0.4 for the
tests, the clone and the flag-off; §3.1 for the retraction; §4 for the
classification).
## The SE(2) certificate, rewritten: the model says 0.9 mm, the exact validator signs for 0.039

Sol review 6 §3 rejected the previous round's SE(2) branch. Its documentary
corrections were right and are carried forward; its certificate solved the wrong
program and is replaced rather than patched. The distinction the rewrite is
built around is the one that round did not have: **a bound on a linear model is
not a statement about the geometry, and only one of the two numbers in a bracket
can be handed to anybody.**

The old program maximized `min_i (a_i . x - rhs_i)` — uniform slack on every
row — which asks to open every pair contact, the left edge, the bottom and the
short edge by the same amount at once. Reducing a published depth requires none
of that. The corrected program puts `delta` only on the rows that measure the
depth, holds everything else at its own contract, and separates the published
material depth from the collision envelope's strip bound instead of clamping one
`sheet_long_axis_mm` onto both.

That separation retired the previous round's most-quoted anomaly. It had
reported that two of four parents "needed their depth bound calibrated upward by
0.15-0.28 mm" before the program would call the state feasible. That number is
now a measurement rather than a correction: `stripExcessMm` is **0.276570** on
155.264 and **0.151163** on 171.238, against 0.002709 and 0.002000 on the other
two. It was the miter reach of the collision envelope all along, in exactly the
range that used to be applied by hand — and with the two gates measured
separately no calibration is needed anywhere, which the run demonstrates rather
than asserts: the parent's own worst residual is non-negative in all six row
families on all four parents.

Five further model corrections, each of which changes a reported number: the
boundary rows carry `a_theta = n . J(p - c)` instead of `theta = 0` (a zero there
lets rotation open a pair contact without paying for the vertex it drives into
the sheet edge, which *overestimates* rotational room); the witness survives the
touch, so active contacts stop reporting a zero rotational coefficient; envelope
rows exist for every reachable pair rather than only for pairs already
overlapping; the guard band is `2*trust + Theta_i*reach_i + Theta_j*reach_j`; and
`Approach`'s witness storage is compiled out when the feature is off, where the
old branch had it on the production path.

The largest finding is the one that only appears once the certificate is made to
return a vector. The rows are relaxed outward by the exact second-order chord
term, which is what makes the dual bound a valid upper bound on rotated
geometry — and the price is that the model's optimum sits microns outside the
true constraint, so its full-length vector is rejected by `validate_publication`
essentially every time. Handing back "witness rejected" would have left the
diagnostic with no constructive lower bound at all, which was Sol's complaint.
So the model supplies the direction and the exact validator decides the length,
by line search. The result:

| parent | model upper bound at 1 mm trust | exactly-validated best |
|---|---|---|
| 155.264 (record) | 0.615525 | **0.039131** |
| 155.422 | 0.919870 | **0.030420** |
| 156.418 | 1.022000 | 0.499876 |
| 171.238 | 0.934582 | 0.211093 |

On the record parents the model claims 0.6-0.9 mm and the geometry signs for
0.03-0.04 mm — a factor of 20. Nobody is blocked, so no front is rigid, but the
gap between those two columns is the honest measure of how much of an SE(2)
bound on this contact front is model error.

It also answers Sol review 5's rank-0 question properly for the first time.
On the model, rotation is worth between 1.00x and 70.48x translation, the same
story the old branch told and then some. On the exactly-validated number, SE(2)
beats translation in 5 of 24 cells, ties 3, and loses 16. Every win is at a small
trust radius and every win has the model's full step surviving the exact
validator intact — 1.56x on 155.264 at 6 microns, 1.78x on 171.238 at 6/25/100
microns — while the losses split into two different causes. Most are the model
over-reaching once the box is wide: the accepted fraction of its step collapses
to a few percent and the line search hands the claimed room back, with the
crossover between 6 and 25 microns on 155.264 and between 0.1 and 0.25 mm on the
other two. But 155.422 loses at *every* radius including 6 microns, and it is
also the parent whose strip excess is nearly zero — its envelopes and its
material agree about where the depth is, so rotation has nothing to exploit. So
rotation is a real lever worth 1.5-1.8x on some fronts, only under a trust radius
small enough for the linearization to hold, and on at least one of these four
parents it is not a lever at all.

None of this is a record claim, and the round says so in its own README before
anyone else has to. `validate_publication` gates material containment and
material pair clearance; it never looks at the collision envelope, and
`contractValid` was not run on any witness. That matters concretely here,
because `EnvelopePair` slack is exactly 0.0 on all four parents — the envelopes
are already touching. Every number was nevertheless re-derived out of engine, by
a Python implementation of the placement transform and a brute-force pair
distance that calibrates itself against the parent's pinned depth before it is
trusted: over all 96 cells, `ALL_AGREE=True ALL_CONTAINED=True ALL_PAIR_OK=True`.
That cross-check also settled the contract rather than assuming it - the
parent's worst pair distance measures 5.004 mm and the certificate's own
`MaterialPair` residual, from unrelated code, is 0.004.

Two process findings are worth more than the millimetres. The first: a draft of
this rewrite reintroduced the exact guard-band defect it was fixing, by
recovering the trust radius as `theta_cap * reach` — correct for a rotatable
piece, **zero** for a pinned one — so two pinned pieces inside the translation
band got no pair row at all. It is caught by a test that fails on the draft and
passes on the fix, which is the only reason it is a footnote instead of a
finding in the next review. The second: the whole-document reproducibility
instrument this round inherited was broken. `lib.doc_digest` dropped `elapsedMs`
but not the five summary statistics computed from it, nor `engineWorktreeStatus`,
so two runs of the same binary on the same gate hashed differently every time —
a digest mismatch proved nothing and a match would have been luck. With the list
repaired, four independent runs across two binaries produce identical digests on
all four gates, and the only leaf path that differs between flag-off and
flag-on-unarmed is `/executableSha256`. All four pinned gates hit on both
binaries, both feature-combination suites pass, and the default path is
bit-reproducing.

Evidence, drivers and the raw certificates: `docs/experiments/se2-rigidity/`.

## Pricing the m34 slice: the three-second budget was being overrun by a slice that never publishes, and the ten-second curve does not move

Grok review 1 §2b asked for three things about the compression-schedule class:
wall-price its first action (item 1), make its entry feasible with the existing
translation-only legalizer or skip the slice (item 3), and give it a one-bit
per-request prior where it has published nothing (item 4). This round did all
three, measured all three, and ships two of them. The headline against the
binding priority is a zero: **at ten seconds, on all three requests, every
published depth is identical to HEAD's in 27 of 27 paired rounds.** What moved
is the wall, at the tiers either side of it.

**The re-baseline first, because the v4 numbers are v4's.** Over eighteen cells
on HEAD - three requests, three seeds, ten and thirty seconds - the class's
first slice costs 2.60-5.88x its work-denominated estimate on the clock, which
reproduces coordinator v4 §8 and is slightly worse. In the currency a prior can
carry, that first slice is 0.990-1.147 phase-zeros on mixed-61, 1.138-1.619 on
shapes-17 and 2.124-2.238 on triangle-20. **2.2375** - the worst of the
eighteen - is the new wall prior, by the same worst-case rule the ladder and the
diversify class are already priced by.

**Two rules read that prior, and both readings had to be measured to be
rejected.** Letting the *ranking* value read it drops the class to
`1.104 / 2.2375 = 0.493`, below the ladder, and cost a paired median **0.649 mm**
over nine thirty-second rounds on mixed-61 with the slice count falling from
2.89 per run to 1.00 - and those later slices publish on 23 of 26. Letting the
affordability gate hold it over *later* slices cost a further **0.137 mm**
median, +2.1 to +4.0 mm on one seed, by refusing a slice that fit: HEAD buys it,
it costs 2.606 s, it publishes 1.03 mm and the run still finishes inside its
budget. Both arms were run to completion as nine-round paired batteries and both
are in the evidence directory. What ships is the residue: the worst-case wall
price is read by the affordability gate, for the **first** slice of a run only,
and the ranking is left exactly where coordinator v4 put it.

**Where that residue pays is the three-second tier, and it is the clearest
number in the round.** HEAD offers the class a slice with about a second of
budget left, prices it at 0.35 phase-zeros, pays 1.11 s on shapes-17 and 1.82 s
on triangle-20, and publishes nothing - in 18 of 18 runs. It **overruns its own
three-second budget in 3 of 9 shapes-17 runs and 9 of 9 triangle-20 runs**,
finishing at a median 2.96 s and 3.70 s. Priced at its worst case the slice does
not fit: shapes-17 finishes at **1.85 s** with **0 of 9** overruns, triangle-20
at **3.10 s** with 6 of 9, and the published depth is identical in 18 of 18
rounds with three triangle-20 rounds 4 µm better.

**Item 4's bit pays at thirty seconds.** On triangle-20 HEAD takes **36 m34
slices across nine runs - four per run, 7.53 s, a quarter of the budget - and
publishes on none of them.** One sterile action now takes the class off the
queue for the rest of the run, with a single audition after sixteen further
barren actions; the count falls to one slice per run, 1.78 s, and the depth is
identical in 9 of 9. shapes-17 halves the same way. It is a within-run bit and
not the per-request memory the review asked for, because the engine is one
process per request with no store, and the README says so rather than eliding
it.

**Item 3 is a measured negative, twice, and the negative is more interesting
than the item.** `global_legalize` on the parent - once the bound was corrected
from the pre-snap parent depth to the entry state's own depth, a mistake this
round made and records - reaches proxy feasibility on **0 of 9** slices. More to
the point, the entry is infeasible on the request where the class publishes 9 of
9 exactly as it is on the two where it never has, so a skip on that predicate
refuses every slice on every request. A second discriminator, the entry's own
depth loss against the drop the slice is allowed, fires nowhere - and on two of
three mixed-61 seeds it cannot even be evaluated, because the snapped entry is
not a valid layout and has no measurable source depth.

What the entry census did settle is that **"the slice is part regrid" is a
per-request claim, not a general one**. triangle-20's slice spends **98-100%** of
its wall in proxy repair sweeps and 0.1-0.8% in the exact tier, and has **zero**
colliding pairs at entry - it is infeasible on boundary violations alone.
shapes-17 spends 76-82% in repair. mixed-61 spends **50-76% in accepted exact
confirmations**, which is the tier it is being paid to spend its wall in. One
mechanism, three different jobs.

**And a fourth mechanism, built because the measurement asked for it and off
because the measurement then took it back.** No number available before step 0
separates the request where this class pays from the two where it does not, but
the difference is plain from the slice's own first steps - so the slice was made
an anytime budget: spend a third of its steps, abandon it if nothing beats the
parent yet. It works. Sterile slices fall to 0.017-0.50 s. It is off for two
reasons neither of which is the ablation: the wall it returns buys **no depth**
on either request that has a sterile slice to cut, and at thirty seconds the
entry loss on a deeper mixed-61 parent is 0.453 mm of a 1.520 mm drop, so a
third of the steps expires before the lane has walked back the snap and a slice
that publishes 1.03 mm is abandoned instead. A step count cannot know how far a
lane has to walk before it is allowed to have evidence; the next attempt should
start entry-loss-relative.

All four pinned gates hit on three binaries - the base commit, this branch's
`jagua-experimental` gate build and its measurement build - with **identical
whole-document digests** on every gate. Flag-off reproduces the base commit as
whole documents on 9 of 9 cells at a work budget; the shipping arm is
deterministic across two processes on 9 of 9; both feature-combination suites
pass at 1,260 and 1,282 tests.

Evidence, drivers and both retracted batteries: `docs/experiments/m34-wall-price/`.
## Intra-arm parallelism of mode 34: the seven idle lanes were real, and they were idle during the wrong 22%

Grok's 10 s plan, action 2, starts from a fact this campaign had committed and
never quantified: one mode-34 arm runs one lane while a mode-26 rung arm runs
eight (`compression-schedule` README §6.3, which states it and explicitly
declines to make a wall claim). The action asks for one clock and eight workers
proposing and scoring moves under it, cadence and floor preserved, feature
flagged, with determinism in work mode as a hard gate rather than a preference.

The fact is right, and this round is the first to put a number on it. Process
CPU-seconds over wall-seconds, with the identical mode-0 preamble measured in
the same shape and subtracted — because the preamble *is* eight-lane and the
whole-process number (2.71 lanes) hides everything — gives the m34 slice an
occupancy of **0.99, 0.97 and 1.02 lanes** on the three parents. One lane, as
claimed.

The inference from it is wrong, and the same instrument says so before anything
was built. The schedule reports its own `repairMs` and `confirmationMs`, and at
the design slice the confirmation is **74.9%, 77.4% and 41.3%** of the arm.
Worse for the proposal, **55-79% of steps repair nothing at all**: a one-micron
step usually leaves the layout proxy-feasible, the sweep loop breaks
immediately, and there is nothing to hand eight workers. Amdahl on those two
numbers, computed before writing a line of the fan-out, predicts 1.25x / 1.25x /
2.06x for a *perfect* eight-way repair — and the measured fan-out came in below
even that.

So the round built both halves and priced them separately, and the interesting
one is the half the action listed under "preserved".

**Where a confirmation's 4.83 ms actually goes, and a correction to
`compression-schedule` §6.1.** That section explains the cost as "a
confirmation the validator accepts asks all 1,830 pairs, and at 1,904.8 ns per
`exactOverlapTest` that is 3.485 ms". The total is right — measured here, mode 34
minus the preamble, `publicationValidate` is 1,559.3 ms over 317 confirmations,
**4.92 ms each**. The attribution is not. `exactOverlapTest` over the same runs
is **41.3 ms in 30,985 calls**: **98 calls per confirmation, 0.13 ms, 2.6% of
it.** §6.1 multiplied 1,830 pairs by a per-*narrow-phase* cost, and its own §6.3
already contains the reason not to — the span and the counter are both entered
past the broad-phase bounds reject. The error is the same ~18x factor §6.3 names
for the self-meter, applied by accident in the opposite direction.

The other 4.79 ms is `validate_publication`'s own `n(n-1)/2` loop over
`minimum_boundary_distance`: the exact-clearance contract, walking every edge of
one material set against every edge of the other, with no broad phase and no
phase span. No round in this campaign had seen it, because nothing was
instrumented to.

**The two levers, measured.** `parallel-compression-schedule`, off by default,
stacked on `compression-schedule`, spec keys `lanes=` and `pconfirm=` for a
replay and `m34lanes=` / `m34pconfirm=` for the coordinator's own slice.

`lanes=8` — the fan-out the action specified — occupies the lanes it was
supposed to: repair does 10.0x the candidate queries in 1.65x the wall, about
**6.1x** effective throughput, and pinned to one CPU the same arm takes 3,126 ms
of repair against 621 ms unpinned, which is the direct proof the job pool is
reached rather than an inference from a rate. It also loses on every gate the
action set. At equal **work** on the twelve pinned 171-179 mm parents it is a
paired median **−0.867 mm, 1 win against 11 losses**, because the fan-out
charges every worker it dispatches — enforced by a test — and therefore walks
618 steps where the serial schedule walks 1,568. At equal **walk** it is
**0.912x wall**, 0 of 30 rounds above parity, for a paired median of +0.028 mm.
On the bare-request 10 s curve it is **−2.158 mm, 0 wins in 9**.

`pconfirm=1` — the confirmation, spread over the pool with a lowest-index
reduce so the verdict and its message stay the serial ones — is **2.623x** on
the m34 slice (30 of 30 paired rounds above parity, worst round 1.587x) and
1.431x on the whole process, at an occupancy of 0.99 → 3.16 lanes. At equal work
it is **0.000 mm on 12 of 12 cells**, and leaf-by-leaf its whole output document
differs from the serial schedule's in **exactly one leaf**: the diagnostic flag
that says it was armed. Every placement, every step row, every counter is
identical.

On the anytime curve from the bare request, 3 seeds x 3 rounds paired, that
buys **+1.882 mm at 10 s and +3.359 mm at 30 s, 9 of 9 wins at both**, and at
3 s it ties on 9 of 9 — the perfect control, because at 3 s the coordinator
schedules no mode-34 action at all. The mechanism is a counter, not a story: the
faster slice fits **two** m34 calls into 10 s where the serial one fits one, and
four into 30 s where it fits three. `lanes8` fits *fewer* — two at 30 s against
three — which is its 0.912x showing up as lost actions.

Grok's hypothesis was "this brings 10 s toward the 40M-work quality (~166), not
to 150". **It lands at 172.288**, about a quarter of the way, and it gets there
from the other lever. At 30 s, 163.927 is past the 40M band and inside the 120M
one (162.161 / 163.927 / 164.004), so the wall-versus-work gap §2a estimated at
4-6 mm is closed at 30 s and a third closed at 10 s.

**The determinism gate is met as worded.** Four arms x three parents x three
processes under a work cap: 12 of 12 cells, one distinct whole-document digest
each. In-process, the same eight-worker schedule on a one-thread pool and an
eight-thread pool produces an identical report including every step row and the
lane-win histogram. The argument behind the measurement is structural: a
worker's entire input is `(frontier, weights, depth, step, worker ordinal)`, the
job-pool maps return results in input order whatever order the workers finish
in, and the reduce is a total order whose last tiebreak is the worker ordinal.

That gate also found the third defect in this campaign's reproducibility
instrument, and found it the only way it could be found — with a control. The
first run reported three distinct digests for **every** cell including the
`serial` shipped schedule, which is deterministic by construction. The leaf diff
said why: the only fields that moved were `repairMs` and `confirmationMs`, the
compression-schedule round's own wall-clock decomposition, never added to the
`VOLATILE` set the SE(2) round repaired for exactly this class of bug. Without
the serial arm in the battery this would have been written up as the parallel
schedule failing its own gate.

All four pinned gates reproduce bit-for-bit on four binaries — HEAD, flag-off,
`compression-schedule`-only, and the armed build with an unarmed spec — as whole
documents and not only on the pinned scalars. All three suites pass. The twelve
gate parents were re-derived on the way in: seeds 3-11 were regenerated from the
bare request and reproduce the committed depth, fingerprint and work-unit spend
to the digit on 9 of 9, which independently confirms that round's parent band
from a different binary.

What the round recommends next is not in its brief. `minimum_boundary_distance`
was spread across eight lanes, not made cheaper; the publication contract costs
4.8 ms per layout and is `O(n^2 * edges^2)` with no broad phase, on **every**
publication in the engine rather than only mode 34's. A bounds reject there —
the one the collision tier already has — is plausibly worth more than any
parallelism, and it is worth it everywhere.

Evidence, drivers and every battery:
`docs/experiments/parallel-compression-schedule/`.

## The contract validator had no broad phase, 96% of its pairs never needed measuring, and the document did not move

The previous chapter ends by recommending this round: "`minimum_boundary_distance`
was spread across eight lanes, not made cheaper... A bounds reject there — the
one the collision tier already has — is plausibly worth more than any
parallelism, and it is worth it everywhere." This is that, built and priced.

**The design question came before the design, and it decided it.** A prefilter
may skip a pair only if it can prove the skip changes nothing a caller consumes,
and "what is consumed" has two possible answers that license completely
different filters. `minimum_boundary_distance` is module-private with **exactly
one call site**, and that site binds the value, tests
`!distance.is_finite() || distance < pair_clearance`, and drops it. Only the
threshold verdict is consumed — so the filter has to preserve a boolean, not a
number, and the campaign's 1-ULP pinned depths are not in the blast radius at
all (they are `raw_source_long_axis_depth_mm`, which never calls the loop). Case
(b) does exist in this engine — `general_micro_legalization`'s
`measure_approach` consumes the value and already carries sound running-minimum
pruning, whose doc comment is the in-repo precedent for the argument here — but
it is a different function on a different path and this round does not touch it.

**Two things a skip must prove, not one.** The scan row runs an overlap test
*and* a distance test, so a filter that reasons only about distance is unsound in
one specific way: **containment**. A region strictly inside another's outer ring
has a large positive boundary distance — 4 mm in the committed test — and is
nevertheless an overlap the validator must reject. It can never be skipped here
because containment makes one slab interval a subset of the other in every
direction, so no direction offers a gap. The other trap is that
`!distance.is_finite()` is a **rejection**: `minimum` starts at infinity, so "no
segment pair existed" fails, and a filter reasoning "far apart, therefore fine"
would invert it. `ClearanceSlabs::of` returns `None` for a pointless set, so a
skip structurally requires a segment pair to exist.

**The filter is four separating axes in the validator's own `f64` millimetres,
and deliberately not the constructor's.** Reusing `GridSlabs::separated` — the
same four-direction polytope, already certified — would have been the obvious
move and is wrong: `GridSlabs` projects the canonical **integer** grid, which is
the quantized geometry this module exists not to read, and importing it would
import the sub-grid blindness `sub_grid_source_overlap_is_not_hidden_by_search_snapping`
was written to forbid. The structure is borrowed; the numbers are rebuilt. The
margin is `1e-9 mm + 1e-12 * extent`, four orders above the worst rounding error
on either side of the comparison and six orders below the tightest clearance the
engine is asked for.

**What it buys.** 96.02% of pairs proved clear over **5,934,690 pairs** on the
twelve pinned parents, per-seed 95.49–97.07% — 1,830 pairs per confirmation
become about 73. Paired interleaved over the twelve parents × 10 rounds, equal
walk: **4.8236 ms → 0.8609 ms per accepted confirmation, 5.5745x, 110 of 110
cells above parity**, against a within-arm spread of ~15%. The flag-off baseline
reproduces §6.1's 4.83 ms and the previous chapter's 5.028 ms, which is the check
that this is the same quantity. Single-threaded, it beats what eight-lane
`pconfirm` achieved (1.091 ms).

**It compounds, and the mechanism is a counter.** On the bare-request 10 s curve
with both arms running the shipping `m34pconfirm=1` spec, **+2.716 mm median**,
3 of 3 seeds improving, and the m34 slice count rises 30 → 49 across nine runs —
a cheaper confirmation fits more slices under the same clock, exactly as the
previous chapter's `pconfirm` result did. `publicationValidate` falls from
**3.914% to 0.496%** of a 10 s run's leaf time while its **call count rises**,
236 → 330. The honest denominator: at a wall budget these runs are
near-deterministic, so nine cells are three results repeated — the sample is
three seeds, all three improving, not nine independent confirmations.

**The equivalence result is the strongest this campaign has produced.** All four
pinned gates reproduce as **whole documents with identical digests** on three
binaries — a pre-patch build of the base commit, flag-off, and flag-on — not
merely on the pinned scalars. Leaf-by-leaf on mode-34 documents,
flag-off against flag-on differs in **at most 2 leaves out of 27,113**, and they
are the same two wall clocks that differ between two processes of the *same*
binary — one of them `confirmationMs`, differing 1636.2 → 298.1 ms because it is
the field the feature exists to move. `pconfirm` could only claim "one leaf, the
flag that says it was armed"; this claims zero, because it changes no verdict and
carries **no counter in the document at all**. That is why the broad-phase census
reports on stderr: a counter in the document would have been the one field that
always differed. The 120 equal-walk cells in the wall battery are an independent
second proof — identical fingerprints, depths, steps, candidate queries and work
units in every one.

**The reproducibility instrument failed again, for the third recorded time, and
this round did not discover it.** The first determinism run failed on every cell
including flag-off; the leaf diff said `repairMs` and `confirmationMs`. That is
precisely the repair the previous chapter records making — its `lib.py:160`, "the
second repair of this list". This round inherited **m34-wall-price's** older
`gatelib.py`, which predates it, so the older copy reproduced the older bug. The
finding is not the defect; the finding is that the campaign carries N copies of
this list and repairs to one do not reach the others. A single shared module
would have prevented this round from spending a battery on it.

**What ships:** `fast-contract-validator`, off by default, no spec key, no
tuning constant, not armed in the coordinator by this round. It has no
arm-versus-arm question — it either proves a pair clear or it does not.

**What it does not do:** close the gap. 169.572 mm at 10 s against Sparrow's
150.165 mm is still 19.4 mm short. This makes an existing operator cheaper; it
adds no degree of freedom, and the continuous-rotation brief's case that the
remaining 12–16 mm needs rotation *in the search* is untouched by it. Only
mixed-61 was measured — shapes-17 and triangle-20 were not run — and the 96% is a
property of that corpus at 171–179 mm, not a general constant. The inner
`O(E₁·E₂)` nest over the ~73 survivors was left alone deliberately: the
confirmation is now 0.86 ms of an 826 ms slice, so the remaining headroom is
worth a few percent against a real increase in hand-proved geometry.

Evidence, drivers and every battery:
`docs/experiments/fast-contract-validator/`.
## Continuous rotation in the relaxed lane: 8.3% of the rungs are accepted, they buy 56% of the loss, and the arm is 3.7 mm worse at ten seconds

The continuous-rotation brief's design A, built and measured: rotation as a
degree of freedom **in the relaxed lane's own candidate loop**, at production
cost, feature-flagged `continuous-rotation` and off by default. The accumulated
evidence pointed here hard — the 2.5-degree snap costs 0.448 mm of entry damage,
the record line's productive rungs are 0.0032 degrees, the SE(2) certificate
finds 1.2-3x more room in rotation than in translation on every parent tested,
and both reviewers converged on "rotation as a continuous degree of freedom in
the search" as the thing standing between the engine and the ~12-16 mm below it.

**It does not work at ten seconds, and the reason is not the one the brief
prepared for.**

`refine_candidate`'s axis schedule gains a rotation rung derived per piece,
`dtheta = dx / r` in both signs — `r` the piece's bounding radius, `dx` the live
translation step of the same schedule, so the descent's own contraction
contracts the rung and nothing is tuned — plus a mirror toggle as a discrete
companion, recentred so a flip is a candidate rather than a teleport. Surrogates
at continuous angles are built on demand through the overlay round's
construction path into a lane-local cache with the brief's shape: a per-piece
pinned slot, an in-flight hold set, a 48-entry LRU, eviction at exactly one
site. The catalogue is never cloned and the angle space is never enumerated.

**The publication path was checked first, as the brief instructed, and it needed
nothing.** `validate_and_measure_placements` rebuilds a placement by
transforming the real polygon and enforces two rotation rules — finite, and zero
when `allow_rotation` is false. `canonical_angle` does not exist outside
`general_relaxed.rs`. The warm-start snap at `general_relaxed.rs:16313` runs
before the lane starts, on the parent's placements, and the operator's angles
never pass through it. An accepted rung is published at the angle it was
accepted at.

**Anytime WALL, paired, both arms with `pconfirm` armed, three seeds x three
rounds:** mixed-61 is **+3.721 mm worse at ten seconds on 0 of 9 rounds** and
+7.071 mm worse at thirty on 1 of 9; shapes-17 and triangle-20 are **0.000 mm**
at every budget, because their mode-34 slice publishes on 0 of 9 runs in *both*
arms. Against Sparrow on the same box the base arm is 22.1 mm behind at ten
seconds and the armed arm 25.1 mm: the operator moves the engine away from the
target the binding user priority names.

**And yet the mechanism works.** 655,477 rotation/mirror iterations at ten
seconds on mixed-61, **8.3% of them improving the incumbent**; **56.0% of all
the proxy loss the refinement removed was removed by a rotation or mirror move**,
against 44.0% for the four translation axes — the same instrument, the same
quantity, the same iterations. **67.1%** of committed moves changed the pose,
against 8.9% unarmed. The rungs reach the sheet: an armed publication carries
46 of 61 pieces at angles the 2.5-degree catalogue cannot express. The SE(2)
certificate's claim reproduces *inside the search at production cost*.

**What it loses is the clock, and the coordinator's action log says so
directly.** One ten-second run: the base arm makes 9 operator calls and descends
179.59 → 176.11 → 175.39 → 175.14 → 173.58 → 172.29; the armed arm makes 7,
reaches 175.22, and stalls. Over the nine paired rounds the base arm ran 30
mode-34 slices and published on 30; the armed arm ran 11 and published on 8. Per
slice, 0.87 s becomes 1.94 s — and **only a sixth of that is the surrogate
builds** (0.32 s per slice, 5.4 microseconds per rotation iteration at an 89.4%
cache hit rate). The rest is the resolution tax: an accepted rung forces the
lane's keys continuous, so every neighbour of an off-grid piece misses the
catalogue and then hits the lane map — two ordered-map descents where there was
one, on the file's most-called path.

**At equal WORK the loss disappears.** Replaying the twelve pinned 171-179 mm
parents through mode 34 under the same query cap, the two arms differ only in
the environment flag: paired median **+0.005 mm, six better and six worse**,
every loss under 0.15 mm — and one cell where the unarmed arm publishes nothing
at all and the armed arm descends **1.681 mm**. On the two record-lineage
parents (156.418, 155.422 at `''` 0.0005) neither arm publishes anything, at the
highest acceptance rate in the round (**33.0%**). The operator is therefore a
*wall* problem, not a search problem: it is worth its candidates and not its
seconds.

Two defects are recorded rather than quietly fixed. The first killed the very
first armed run — `build_oriented_surrogate` enforces `MAX_CELLS_PER_JOB`
against a *cumulative* counter, which is right for a catalogue that stays
resident and wrong for surrogates that are evicted, so the slice failed with
"at most 524288 generated cells" and the incumbent fell back to the m22 arm; it
is now a residency guard over what the cache actually holds, and the battery
confirms it never binds. The second no measurement in this round could have
caught: the mirror companion's second probe rotates, so a piece the request
forbids rotating would have been committed at a rotated pose and refused at
publication. Every piece of all three campaign requests allows both transforms.
It is now conditioned on `allow_rotation`, and the test that pins it was checked
against the bug rather than assumed to catch it.

Four pinned gates reproduce as whole documents on three binaries — the base
commit, the patched build without the feature, and the patched build with it
compiled and unarmed. Flag-off document reproduction against the base commit is
9 of 9. Determinism across two processes with the operator armed is 9 of 9, a
hard gate. Both suites pass, 1,261 and 1,293 tests.

What the round recommends is not in its brief. Five sixths of the price is the
*resolution* tax, not the geometry, and the attack on that is a catalogue whose
keys are continuous by construction rather than a 2.5-degree grid with a
lane-local overflow beside it. Designs B and C — rungs when the compression
clamp binds, and witness-driven rungs from the SE(2) dual — both propose orders
of magnitude fewer rotations than A's one-in-six, and A's numbers are the first
production measurement of what one of those rotations is worth (8.3% accepted,
56% of the loss) and what it costs (5.4 us of geometry, and a 2.2x slice).

Evidence, drivers and every battery:
`docs/experiments/continuous-rotation/`.

## The rotation tax was mismeasured: mode 22 was never counted, the builds are 89% of it, and 81% of a build is one Clipper offset

The brief was to make the proven rotation mechanism affordable on the wall and
then measure the compound. The first half turned up a measurement error in the
round that preceded it, and the error is the finding.

**The previous chapter concluded that "only a sixth" of the armed slice's
slowdown was surrogate builds and the other five sixths was the resolution tax.
That is wrong, and it is wrong because of a reporting gap rather than a
measurement error.** The coordinator arms the operator on modes **22 and 34**
(`portfolio.rs`: `matches!(mode, 22 | 34)`), and `rotation_surrogate_builds` is
only ever *reported* on a mode-34 schedule slice. Counted process-wide for the
first time — a new `rotation-tax-census` feature, per-thread counters, off by
default and never compiled into a binary that carries a wall claim — one armed
ten-second mixed-61 run performs **1,129,375 surrogate builds costing 6,407 ms**,
and the mode-34 slices of that same run report **169,772 builds costing 703 ms**.
Fifteen per cent of the builds, eleven per cent of the wall. The instrument is
not in doubt: 169,772 is the exact number the previous chapter's one-cell probe
recorded for the same cell.

**Four fifths of a build is one function call.** Timed stage by stage:
`transformed` 0.51 µs, **`.offset()` — the Clipper miter offset — 4.71 µs
(81.4%)**, `triangulate_ring` 0.16 µs, poles and cell index 0.41 µs.
`prepare_continuous_candidate` was entered armed 1,429,252 times and 1,129,375
of those (**79.0%**) landed on an angle the lane had never seen. That miss rate
is compulsory, not a cache-sizing problem: the rung contracts on every rejection,
so consecutive iterations never propose the same angle. Design A's price is one
polygon offset per rung, and no cache fixes it.

**Three fixes were built anyway, and they are answer-preserving.** A remembered
route in the existing per-piece angle memo, so an off-grid neighbour stops
descending the whole catalogue before reaching the map that has it (73.7% of
88.2 M double descents removed); a per-piece pose slot so
`ensure_state_surrogates` skips a piece that has not moved (428.2 ms → 39.1 ms
over three isolated slices, 10.9x); and the indexed probe queue the previous
chapter asked for, replacing a 48-entry linear scan whose cost it estimated at
"tens of millions of comparisons" and which this round counted at **212.5 M**.
**The third one was measured and taken back out**, and that is the second
finding. The index works — it brings 212.5 M comparisons down to 2.0 M, one per
call, reproducing the deque's eviction order element for element — and at the
shipped 48-entry window it is **0.38% slower on the slice and 1.14% slower on
the process**, 22 of 24 and 21 of 24 paired equal-work cells. Fifty-five
contiguous 24-byte tuples are twenty-one cache lines the prefetcher walks in
order; two ordered-map descents chase pointers. "Removable with an index" was
true; "worth removing" was not; and the only way to tell those apart was to
build both. The variant is committed as a patch, not as code, because the trade
flips if the window is ever raised.

At equal work, three pinned parents × 10 paired rounds: **1.0328x** on the slice,
30 of 30 cells, against a within-arm spread of 1–2%; and **zero mismatches** on
`fingerprint`, `rawSourceDepthMm`, `candidateQueries`, `exactPairTests`,
`exactValid` and `contractValid` across all 30. From the bare request at ten
seconds with the operator armed on both sides, the mode-34 slice costs
**1.9775 s → 1.8243 s**, 1.0796x, 12 of 12 paired cells — and the slice count
does not move, so the depth does not move.

**The compound battery is negative, and by more than last time.** 162 runs,
three requests × three seeds × three rounds × 3/10/30 s, both arms carrying
`fast-contract-validator` and `m34pconfirm=1`:

| request | 3 s | 10 s | 30 s |
|---|---|---|---|
| mixed-61 | +0.000 mm | **+6.735 mm worse, 3 of 9** | **+5.736 mm worse, 0 of 9** |
| shapes-17 | +0.000 mm | +0.000 mm | +0.000 mm |
| triangle-20 | +0.000 mm | +0.002 mm | +0.000 mm |

The previous round measured +3.721 mm at ten seconds; this one measures +6.735
mm with the tax **lower**. The reason is the fast contract validator, and it is
the answer to the question the brief actually asked. A 5.57x confirmation is a
multiplier on what a search can already do; it lifted the **base** arm from
172.288 mm to **168.484 mm** and left the armed arm at 175.219 mm, because the
armed lane's bottleneck is 1.13 M polygon offsets in the proxy tier and no
confirmation speedup touches those. More slices per second makes the arm that
fits more slices in win by more. Against Sparrow on the same box, the base arm
is now **18.3 mm** behind at ten seconds where it was 22.1 mm; arming rotation
puts it **25.1 mm** behind.

Two readings that are not the verdict. The rungs still work — 8.9% accepted,
**56.4%** of the proxy loss bought by rotation against 43.6% by all four
translation axes, 68.9% of committed moves changing the pose, every number
reproducing the previous chapter's. And the **89.4% cache hit rate that chapter
published was not the operator's**: it counted the hits
`ensure_state_surrogates` took re-confirming poses that had not moved. With
those skipped the real figure is **54.0%** on mixed-61 and **72.1%** on
shapes-17 — where the previous chapter reported 88.3% on an otherwise
unit-identical armed run.

**The next lever is named, priced, and deliberately not taken.** A miter-join
offset is rotation-equivariant, so the operator could offset each piece once at
zero degrees and derive every rung by transforming the already-offset ring —
roughly four times cheaper per rung. It is not done here because the two orders
round differently on Clipper's integer grid, so the surrogate's geometry
changes, so trajectories change: that is a new operator geometry needing its own
matched-arm quality battery, not a wall fix with a spot-check, and this round's
licence was "answer-preserving". What this round contributes is the number that
makes it worth doing.

Four pinned gates hit on three binaries — the base commit, the committed gate
build, and the committed measurement build with every flag off — and the
**whole-document digest is identical across all three on all four**, which is
the check that matters when the change touches `AngleKeyCache` and
`resolve_surrogate`, both on the hot path of every mode in a default build.
Flag-off document reproduction against the base commit is 9 of 9. Determinism
across two processes with the operator armed is 9 of 9, a hard gate. Both
suites pass; suite 1 needed the one rerun the campaign's known flake
(`free_material_multi_eviction_shrinks_retained_container_capacity`) always
needs, and the rerun is recorded rather than the first run discarded.

The instrument that made all of this visible is a new feature,
`rotation-tax-census`, off by default and never compiled into a binary that
carries a wall claim. Its own first version was a finding: one shared atomic
array across eight fan-out lanes was slow enough that the coordinator stopped
reaching a mode-34 slice at all, so the instrument destroyed the phase structure
it existed to describe.

Evidence, drivers and every battery: `docs/experiments/rotation-tax/`.

## Closing the contract validator: the lemma was true for an unwritten reason, the margin is a derivation, and `pconfirm` stopped being worth anything

Sol review 7 §1 refused to promote the broad phase and listed five things
promotion needed. This is those five, worked, plus the recommendation. **No
default is flipped by this round**; the recommendation is the deliverable.

**The `!is_finite` lemma was true, and the previous chapter's proof of it was
not.** That chapter argued "a skip requires both sets to have a point, hence a
ring, hence a segment pair, hence a finite minimum". Sol was right that the last
step does not follow: `point_segment_distance` has squares, products and a
division that manufacture `inf` and `NaN` from finite inputs, `f64::min` returns
the *non*-`NaN` operand and so leaves `INFINITY` standing, and a non-finite
minimum is a **rejection**. The committed witness builds two material sets at
`x = ±1.3e308` whose exact minimum is non-finite — so the scan row rejects them —
and whose unguarded slab gap overflows to `+inf` and clears every threshold. The
old certificate would have skipped a rejection.

**But it is unreachable, and finding out why is the actual result.** The grid
contract bounds the *source* ring at `9.007e12 mm` and does not survive the
transform: `translate_x` is only checked finite. `validate_sheet` cannot be
leaned on either — it runs after the transform, over outer rings only, against a
sheet width that is itself only required to be finite. The bound that holds comes
from **`interior_sample`**, via `transform_placement`, which rejects a region
with no discoverable material interior. That needs two *distinct* `f64` y-levels
and two distinct x-intersections; two distinct doubles of magnitude `M` differ by
at least `M * 2^-53`, and both differences are bounded by the region's diameter,
`2*sqrt(2)*9.007e12 ~= 2.55e13 mm`. So every coordinate `transform_placement`
admits satisfies `|x| <= 2.55e13 * 2^53 ~= 2.29e29`, and nothing overflows there.
**The validator's soundness was resting on an unstated consequence of a helper
whose declared job is "this piece has no interior".** It now rests on a check:
`CLEARANCE_SLAB_MAX_COORDINATE_MM = 2^112`, fail-closed, sitting `2.3e4` above
that structural ceiling (so it refuses nothing contractual) and 385 binades below
the `2^497` horizon where `orient2d`'s splitter would overflow (so it proves
finiteness on its own). The domain is also what keeps `orient2d` exactly signed,
which is what makes `rings_properly_cross` and `classify_point_in_ring` exact —
load-bearing for the overlap half of every skip.

**"A handful of ulps" is replaced by two separate things.** The slab side is now
a certificate with no epsilon in it: diagonal projections are rounded *outward*,
so the stored interval is a guaranteed superset and `next_down(gap)` is a
rigorous lower bound on the true gap. The exact loop's own error is derived
rather than asserted — tracking every rounding through the clamped parameter (in
`[0, 1]`, so `S + p(E-S)` is a real point *on* the segment), `closest`, the
difference and `hypot` gives `computed >= true - 16.5*C*u`, and the overlap half's
rounded midpoints need `1.5*C*u`. `32u = 3.553e-15` dominates both; the shipped
margin is `1e-12`, **281x** that, so nothing moved. The margin is computed as
`max(shipped, derived)`, which makes the proof structural rather than tested.

**The hot loop is byte-identical, on purpose.** The first implementation put the
outward rounding on the gap and cost ~3x the filter's arithmetic per pair. Since
`next_down` is monotonic, `next_down(g) >= t` is exactly `g >= next_up(t)`, so
the rounding moved to the threshold — `O(directions)` per call instead of
`O(pairs * directions)`. `provably_clear` is unchanged from the binary the 5.57x
was measured on, which is what lets that number stand without a re-run.

**The release shadow corpus, which is the gate the previous round could not
pass.** Its `debug_assert` is compiled out of release, so its 5.9 M-pair census
had no checking behind it, and its equivalence test compared one path against
enumerated expectations rather than against an implementation.
`validate_publication_exact_reference` now runs the validator with the broad
phase *disarmed* — every slab `None`, every threshold infinite, costing the armed
path nothing — so one release binary holds both implementations, and
`contract_validator_shadow_audit` re-checks both bypassed tests per certified
pair with explicit branches. Over five seeds: **1,051,980 layouts, 1,695,677
pairs, 1,002,726 certified, 470,524 layouts rejected, zero verdict mismatches
(whole `Result`, error message included) and zero per-pair audit mismatches**.
The tightest certificate sits **2.1 nanometres** above the clearance — the
randomized regime could only reach 7.8e-2 mm, so a deterministic axis-aligned
sweep steps the separation through `clearance + k*margin` for `k = -10..10`, and
no `k < 0` was ever certified. Holes, multi-region sets, slivers to 0.001 mm and
the contractual `0.0005 mm` clearance are all in it.

**The density prediction is refuted, in the favourable direction.** The previous
chapter's caveat was that 96% was measured only at 171–179 mm and "could be
materially lower" at the 155 mm record line. Walking the campaign's own pinned
layouts — the same 61 pieces on the same sheet — the record line at
155.264–164.038 mm skips **96.47%** against the band's **96.02%**. Packing 16 mm
deeper costs nothing: with 61 pieces on a 2000x2700 sheet most pairs are across
the sheet, and depth compression moves pieces closer only locally. Every fixture
the campaign has lands in **95.5%–97.2%** (shapes-17 95.90, triangle-20 97.19,
small-8 96.43).

**And the 5.93 M-pair census is bit-identical to the previous chapter's** —
3,243 calls, 5,934,690 pairs, 5,698,534 proved clear, `skipRate`
0.9602075255826337, every per-parent rate matching. The guard, the outward
rounding and the doubly-bumped threshold changed **not one of 5,934,690
pair-level certificate decisions**. The four pinned gates now run against *four*
binaries rather than three, which separates two questions the old three-way
conflated: `base-off == off` says the default build did not move, and
`base-on == on` says the **certificate** did not move — the question that only
exists because this round edited the flag-on path. Both hold, with all four gates
hitting their pins and one digest per gate across all four arms.

**Per-confirmation coverage, including two honest absences.** On triangle-20,
paired and interleaved over 10 rounds at equal walk, **0.2090 ms → 0.1062 ms,
1.97x, 10/10 above parity** — not 5.57x, and that is the right direction: 20
pieces offer 190 pairs where mixed-61 offers 1,830, so the filter's value scales
with the pairs it removes rather than with the fraction. On shapes-17, the
eight-piece request and the 155.264 mm record parent there is **no
per-confirmation wall at all**, because the validator is never called: all three
replay with `confirmationsAttempted = 0` and `confirmationsSkippedInfeasible`
equal to `stepsTaken - 3`, and a drop sweep over `0.05–0.8 mm` on shapes-17 found
zero confirmations at every step. The proxy tier calls every reduced layout
infeasible and the exact validator is never reached. On those fixtures the
feature is worth exactly zero — not because the filter fails but because the
operator it accelerates never gets to its expensive step.

**The factorial answers the `pconfirm` question both reviewers raised, and the
answer turned out to be about the box.** Four cells at a 10 s wall, 3 seeds x 3
rounds, paired and interleaved with cell order rotated:

| cell | median depth | per accepted confirmation |
|---|---:|---:|
| off / pconfirm 0 | 173.575 mm | 4.6116 ms |
| **on** / pconfirm 0 | **170.453 mm** | 0.7952 ms |
| off / pconfirm 1 | 172.288 mm | 0.9870 ms |
| **on** / pconfirm 1 | **168.756 mm** | **0.2774 ms** |

The tax hypothesis is **refuted at the microbenchmark**: `pconfirm` still buys
2.87x on top of the filter, so its dispatch has not overtaken the work. And both
levers compose — baseline → fcv alone is **+3.122 mm, 9/0/0**; baseline →
pconfirm alone is +1.882 mm, 9/0/0; baseline → both is **+4.819 mm, 9/0/0**; and
fcv alone → both is **+1.527 mm, 5 wins / 3 ties / 1 loss**.

**That last contrast is the one to be careful about, and this chapter had it
wrong once.** The battery was run twice on behaviourally identical binaries. The
first pass, on a busier box, put `on / pconfirm 1` at 171.111 mm and made
`fcv alone → both` **+0.000 mm, 4/3/2** — and it was written up as "`pconfirm`
buys nothing in depth", with a recommendation to ship it disarmed. **That is
retracted.** Both batteries are kept
(`evidence/factorial-10s-loaded-box.json`), because together they say something
neither says alone: **the two `pconfirm=0` cells are identical across the two
batteries to the millimetre on every seed, and the `on / pconfirm 1` cell moved
2.4 mm between them.** `pconfirm`'s value is a function of the cores actually
available; the serial arm's is a constant. On a contended machine the parallel
confirmation decays toward parity with serial.

It also costs cross-round reproducibility, and over both batteries that is 24
seed-cells with no ambiguity: **every one of the 12 `pconfirm=0` seed-cells
reproduced its depth exactly, and all four cells that varied were `pconfirm=1`.**
At a wall budget the parallel confirmation's jitter becomes how many actions fit,
and therefore depth. (The work-budget determinism gate still passes on all 18
cells with both binaries; this is a wall-budget statement.)

**Recommendation: arm `fast-contract-validator`, and keep `m34pconfirm=1`** —
which is what the previous chapter shipped, so this confirms it rather than
changing it. The qualification is new: a deployment that cannot promise spare
cores should expect the filter's `+3.1 mm` and not the combined `+4.8 mm`, and
one that needs reproducible wall-budget runs should prefer `m34pconfirm=0` and
accept `+3.1 mm`.

**What this does not settle.** `fcv on, pconfirm 1` at 168.756 mm reproduces the
rotation-tax chapter's 168.484 mm to 0.27 mm on the same commit, spec and
request — so this round confirms that number rather than improving on it, and
the gap to Sparrow's 150.165 mm is **18.6 mm**, exactly where the previous
chapter left it. Absolute wall-budget depths still do not travel between
batteries; only the paired contrasts inside one window are the claim, and the
retraction above is the demonstration of why. The seed set is still three.
Everything is still one x86_64 box. This round makes an operator sound and
covered, and adds no degree of freedom.

## Sparse rotation: the tax is gone, the operator is accepted five times as often, and the depth is a null

The brief was to take the expressive degree of freedom without the blanket tax.
Both reviews of the previous round had converged on the same pair of verdicts —
blanket design A is dead on the wall, and a sparse form pays the same per-rung
price unless the build cost is fixed first — so this round fixed the build cost
first, made the arming sparse second, and measured the compound.

**The build cost is fixed, and the previous chapter's own estimate was right.**
A miter-join offset is rotation-equivariant, so the operator now offsets each
piece **once** and derives every rung by transforming the already-offset ring.
The previous chapter priced that at "roughly four times cheaper per rung" and
declined to take it because it changes the surrogate's geometry. Measured:
**1.271 µs per build against 4.19 µs** in the anytime battery (3.30x), and
**4.34x** and **4.54x** in the equal-work battery, with **100% coverage and zero
fallbacks over 1.4 M builds** on three requests. It is not bit-identical — the
two orders miter on different snappings of Clipper's integer grid — so it was
given the matched-arm quality battery that licence required, and it **passed in
the favourable direction**: at equal work on twelve pinned parents it is
**0.040 mm better than the per-rung offset on 27 of 36 paired cells** with
design A armed, and 0.028 mm better on 18 of 36 with design B.

**The arming is sparse, and on two of three requests it costs literally
nothing.** A stall is a repair sweep that left the frontier infeasible and did
not lower the loss; at that point, and only then, the pieces the lane's own
violating-pair queue names are offered rungs for the rest of the step. Mean
episode width is **2.6 of 61 pieces**. On shapes-17 and triangle-20 the schedule
steps do not stall, so **zero episodes open and zero surrogates are built**,
against design A's 355,404 and 1,336,518 — and the armed slice returns to parity
with the unarmed one. That is the control flow, not a policy: the
request-adaptive disarm bit this round also built **never fired**, because it
requires an episode to have evidence about and there were none.

**The compound is the finding, and it is a null.** The armed mode-34 slice now
costs **1.064x** the unarmed one at ten seconds where design A cost 2.12x; rungs
are accepted **5.22x** as often per proposal (23.36% against 4.47%); the arm
produces **91% as many improvements from 17% as many proposals**; and it
publishes on 31 of 31 slices where design A published on 12 of 15. And at a
ten-second wall on mixed-61, over **six** seeds, the paired difference is
**−0.290 mm with a within-seed spread of 4.0 mm** — a favourable direction the
instrument cannot resolve. At thirty seconds it is resolvable and against the
operator: **+1.483 mm, base better on 5 of 6 seeds.** At equal work, where the
throughput question is removed, the arm is **−0.077 mm on 24 of 36 paired
cells**, which is real and is two orders of magnitude short of the gap.

**The witness was wired, priced, and is dominated.** Design C runs the rewritten
SE(2) certificate's one usable program when a stall outlives a step. It costs
**1.42 ms a call** — 0.18% of a slice, three orders of magnitude below the
diagnostic's four-program call — and the round's working hypothesis that a
slice-affordable iteration budget would not converge is **retracted**: at 64
iterations the witness is within 4e-5 mm of the witness at 20,000, with
`scale = 1.0`. It is accepted 6 to 16 times across twelve parents and buys up to
2.714 mm against the running incumbent. The final published depth moves on
**0 of 12 cells**. Everything the certificate can point at, the schedule's own
1,600 one-micron steps reach without it; at a trust radius small enough for the
linearization to hold, the translation column returns exactly the trust radius,
so the witness's answer *is the box*.

**What the round removes is an explanation.** For three chapters the rotation
operator's cost has been a sufficient account of why it loses. It is no longer:
the operator is nearly free, it is five times more likely to be right when it
proposes, and the depth still does not move. The residual is quality per action,
which is what Sol review 7 predicted in a sentence this round has now measured.

One caveat is load-bearing and is carried forward rather than buried. This
round's base arm reproduces the base commit **document for document at a work
budget, 9 of 9** — and its own ten-second wall median differs from the previous
chapter's by 2 mm on the same seed and 5 mm across a wider seed set. A
wall-budget median on this fixture is not stable to the three decimals this
campaign has been quoting it to, and every millimetre claim above is stated
against this session's own base arm for that reason.

Four pinned gates hit on **four** binaries — the base commit, this tree's gate
build, and both measurement builds — with the whole-document digest identical
across all four on all four, which is the check that matters because the
constructor refactor those two constructions share is not feature-gated.
Determinism across two processes with the operator armed is 9 of 9. Three
suites pass on first attempt, including the campaign's known flake.

Evidence, drivers and every battery: `docs/experiments/sparse-rotation/`.

## The shipping configuration is armed, and the ten-second number stopped being a distribution

Two rounds recommended a configuration and neither turned it on;
`docs/experiments/fast-contract-validator/` §13 says so of itself — *"no default
is flipped by this round — the recommendation is the deliverable"*. This round
flips them, and then does the thing that had to happen before any of the
campaign's millimetres could be quoted at all.

**The promotion, and it is a default inside a flag.** `m34pconfirm=1` and the
exact-clearance certificate are now the v3 coordinator's defaults whenever their
Cargo features are compiled — which they are not, by default, so the default
build and all four pinned gates are binaries in which the two fields do not
exist. The certificate needed a *disarm* rather than an arm, which is §13.2's
first condition: before this round the exact loop was unreachable from any spec
key in a release binary, and now `fcv=0` reaches it, scoped to one run and
restored on the way out. Both opt-outs stay, and §13.2(4)'s qualification is why:
`pconfirm` is worth its 1.5 mm only where there are spare cores.

Measured as documents, at a work budget, six arms per cell over 24 runs: the new
binary's default **is** the old binary's shipping configuration, field for field,
and `m34pconfirm=0` reproduces the old *default* field for field. Both promoted
levers are semantics-preserving in the work currency — neither touches
`Counter::ExactPairTests` — so what they buy is wall, and the microbenchmark says
how much: **0.2573 ms per accepted confirmation against the previous default's
0.7991 ms**, a **3.11x**, with the ordering of the two levers flipping between
mixed-61 and triangle-20.

**And then the finding that reframes the campaign's own record.** Twenty runs of
one command — mixed-61, seed 0, `wall=10000`, the shipping arm, one binary, one
afternoon — produced **three different depths**: 168.4836 thirteen times,
169.5878 six times, 171.111 once. Two of those three are numbers this campaign
has published as *separate results*: `rotation-tax` §4.2's **168.484** and
`fast-contract-validator` §12.1's loaded-box **171.111**, which that round
attributed to the box being busier and used to retract a verdict. A second
twenty-round battery the same afternoon found a **fourth**, 169.379. The 2.6 mm
that two chapters attributed to box load *between sessions* is reproducible
inside a single twenty-run window. The same battery's `wall=10000` arm also
**overran its own ten-second budget on 21 of 60 runs**.

**`plan=<ms>` is the answer, and it is a trade rather than a win.** A wall target
spent as a work budget the coordinator sizes from its own protected phase 0: one
clock read, floored onto a geometric ladder so two processes agree on it, and
reported so a caller can replay it exactly as `work=<units>`. Over the same
twenty rounds it chose **one plan per seed, 20 of 20**, produced **one depth and
one whole document per seed**, and **overran nothing, 0 of 60**, at a wall p95 of
8.282 s against the 10 s target.

The price is the unspent wall, and it is smaller than it looks. Over the
canonical table's **nine (fixture, budget) rows the median price of
reproducibility is +0.000 mm**: seven rows are at parity to within 0.012 mm, one
row is 1.074 mm *better*, and the entire cost is one row — mixed-61 at ten
seconds, at **+6.904 mm**. That row decomposes into a conservative bias constant
(≈3.74 mm; a single number cannot fit a phase-0 bias that ranges 1.12x–1.59x
across nine cells), the work counters a work budget carries by construction
(**1.882 mm**, measured directly at a fixed wall), and the ladder's floor
(1.281 mm). The first is the largest and it has a named fix — a second clock
reading at a deterministic work checkpoint — which is priced and declined in
§13.1 rather than done badly.

Two honest limits are published rather than buried. The plan mode's wall promise
holds at ten seconds — **0 of 9 cells over target, against the wall mode's 1** —
and fails at both ends for two different reasons: at three seconds because
neither mode can stop an action in flight, and at thirty because the fitted bias
rises with the budget and a constant fitted at ten is no longer conservative
(36.39 s on one cell — against the wall mode's **41.23 s** in the same cell). And
two of the table's 27 plan cells did not reproduce; both are **exactly one ladder
rung** apart, which is the only failure mode §7 predicts, and one of the two
produced the same layout anyway.

Four pinned gates hit with the **whole-document digest identical to the base
binary on all four**. Determinism across two processes is **9 of 9 at a work
budget and 9 of 9 in plan mode**, where the plan-mode gate is two claims and not
one: the two processes must choose the same plan, and then produce the same
document. Both suites pass. The canonical production table — three fixtures,
3/10/30 s, three seeds, two processes each, plan against wall — is
`docs/experiments/calibrated-plan/` §10, and its headline is one line:
**`plan` reproduced 25 of 27 cells; `wall` reproduced 0 of 27.** The gap to
Sparrow is untouched at **18.3 mm** on the `wall` arm at ten seconds, which
reproduces `fast-contract-validator` §13.3's 18.6 mm to inside the spread this
round measures.

Evidence, drivers and every battery: `docs/experiments/calibrated-plan/`.

## The multi-basin race never once picked a different basin, and three counters were measuring the wrong thing

Sol review 8 §4 item 3 and Grok review 3 §3 item 2 are the same spend named
twice — *"il rischio dominante è ancora entrare nel basin sbagliato"*, with the
best FCV arm spanning 165.656–174.280 mm at ten seconds — and both reviews put
three one-line fixes in front of it. This round does all four. Three of the four
are negative results and they are worth more than the positive one would have
been, because each one retires a claim the campaign was carrying.

**Fix (a): trigger B was not the trigger the document describes.** The disarm
compared the sweep's loss against the *step's historical minimum*, seeded with
the entry loss, rather than against the loss the sweep was handed. On
`10 → 8 → 9 → 8.5` the last sweep lowers the 9 it was handed — translation has
demonstrably resumed — and the old rule stayed armed because 8.5 is not below
the 8 the step touched two sweeps earlier. That is the normal shape of a
weighted repair, because `update_weights` moves the weights under the frontier
after every sweep. The rule is now one shared `StallDetector` that the serial
loop and every fan-out worker both call, and the regression test runs the old
rule beside the new one and asserts the two **disagree**, so a change that made
them agree again fails rather than going quiet.

**Fix (b): the disarm bit was reading the catalogue, and here is the control
arm.** `rotation_accepted_moves` counts any accepted move whose committed pose
differs from the incumbent's — including the random catalogue angles
`search_piece` draws as refinement starts. Twelve pinned mixed-61 parents at a
fixed 3,341,379-unit cap, three arms on one binary: the **control arm proposed
zero rungs and reported 3,841 `rotationAcceptedMoves` against 0 committed sparse
moves**, which is Sol's 11,523 cell reproduced on this tree. Design A, armed on
every piece, scores zero in the sparse column too — correctly, because its rungs
belong to no episode. Design B's own chain is 3,391 episodes → 104,244 sparse
rungs → 29,247 winners (28.06%) → **3,833 committed moves** (13.11% of winners)
in **2,322 distinct episodes**, so **68.48% of stalls converted** and the old
counter overstates the operator by **1.602×**. The bit now reads the committed
column, and its rule was extracted into a function so the test drives the
production code instead of a copy of it — the previous test re-implemented the
rule inline and would have passed straight through this bug.

**Fix (c): design C was a no-op on the published depth, and the wire changes
that.** Sol's diagnosis was that an accepted witness updated
`published_depth_mm/placements` and never `state`, `confirmed_state`, the floor
or the archive, so the round's 0/12 measured domination rather than composition.
Measured, on the same twelve parents at equal work: **the witness-on arm equals
the witness-off arm on 12 of 12 parents, to the digit**, having accepted **7
witnesses across 5 parents** worth a cumulative 0.173 mm. Not one micron
survived. With the accepted witness wired into a child frontier — both halves of
the schedule's snapshot, the floor deliberately untouched because a floor is
what a confirmation at the frontier leaves behind — the count is **2 of 12
descendant publications**. Sol's stopping rule is *"se resta 0/12 … taglio
witness/m33"*; 2/12 is not 0/12, so the rule does not fire and the null was the
instrument. It still does not pay: 2 better, **3 worse**, 7 tied, median
**0.000 mm**, and the counters say why — adopting the witness explodes the stall
count on the two worst cells (807 → 2,308 and 254 → 2,468 episodes), because the
child frontier's pairs are a layout the lane's weights know nothing about. The
adoption ships as a spec key, off. The claim *"the witness does not compose"* is
retired and replaced by *"the witness composes and the composition is not worth
the frontier it disturbs"*.

**The race: 0 of 18.** A phase in front of the v3 queue, spec-keyed and off.
Slot 0 is the **incumbent control**, which is what makes the equal-work gate
fair — its audition batch is the first mode-34 action the queue would have spent
anyway, so the race's price is the challengers alone, and a winner that is not
slot 0 is a basin the un-raced run would never have used. Challengers are salted
constructor draws (the ledger's own lesson: `construction_seed` derives from the
**target**, so a salted clamp is a different lottery and a salted seed is a
replica) descended by one m22 quantum, or — the cheap variant — the basins phase
0 already archived. Judged on Sol's three criteria with depth deliberately
excluded, ranked by rank sum so no weighting has to be tuned, with every tie
breaking toward the incumbent. Successive halving keeps `ceil(live/2)`, doubles
the rungs, and stops the moment there is nothing left to decide.

**It never moved the run.** Winner slot 0 in **18 of 18 cells** — both variants,
three fixtures, three seeds — so the whole depth delta is cost and there is no
quality upside to weigh against it. The arm rows say why, and it is a correction
to the criteria rather than bad luck. Over all **45 arms**, **`stability` takes
exactly one distinct value, 1.000**: the schedule steps by one canonical quantum,
so a confirmation attempt essentially always succeeds on these three requests and
the criterion has *zero variance*. And **`infeasibility` takes two values on the
incumbent — 0.000 on mixed-61 and triangle-20, 0.353 on shapes-17 — against
0.350–1.000 on the challengers, and strictly lower than every challenger of its
own cell in all fifteen cells that had one**, which is structural: the incumbent
is a published, exact-valid layout, so the proxy sees few or no violating
pairs. Two of the three criteria discriminate
among *peer* basins, and the arm set contains one arm that is not a peer.

**The equal-work gate fails, and one ratio explains it.** The plan is installed
at the end of phase 0, *before* the race, so both arms of a cell run the same
phase 0 and read the same bit-identical probe counter; the only differing input
is the one clock reading the ladder exists to absorb. The driver therefore
*checks* that the two arms bought the same integer budget rather than assuming
it, and the cells where the ladder straddled anyway are excluded rather than
averaged in. On
mixed-61, the only one of the three fixtures where a basin decision has room to
matter at ten seconds, the two equal-work cells are **+2.366** and **+2.934 mm**
worse. The cost decomposes to two rates in one phase of one run: **the work meter
prices a second of mode 34 at 6,628,431 units and a second of mode 20 at 92.7 —
a 71,500× spread.** The two constructor draws are **70.8% of the race phase's
wall and 0.0123% of its work**, so the race's share ceiling — enforced, like
everything else, in the budget's own currency — **cannot bound their wall**:
every mixed-61 cell exits the phase on `deadline` having spent 8.2–9.6 s, and
lands at 13.5–17.1 s against a ten-second target while the un-raced arm lands at
7.2–7.8 s. Grok review 3 §3 predicted exactly this — *"mode 20 è quasi gratis in
work units e il work budget lo sotto-prezza"* — and this is the number. It is
Sol review 8 §3 condition 4 arriving from a new direction: the same meter, blind
rather than expensive.

The archive variant is the control the round did not have to build: on shapes-17
the archive offered no distinct challenger, the race did nothing at all
(`rounds = 0`, race work **0**), and its equal-work cells came out at **exactly
0.0000 mm** with wall parity. A race with nothing to decide costs nothing
measurable, so the phase itself is not the tax — the challengers are.

Four pinned gates hit with the **whole-document digest identical to the base
binary on all four**. Determinism across two processes at a **work** budget — the
gate this round's own code is responsible for, because a work budget is a
function of counters and not of the clock — is **9 of 9 with the race on and 9 of
9 with it off**, so the race's decision, its eviction and its report all
reproduce. Both suites pass first attempt, exits 0 and 0 over 1,268 and 1,322
tests. The plan-mode rows are worse (7/9 and 5/9) on a box another agent was
saturating throughout; all six misses are *plan* disagreements rather than
document ones, and the base binary fails in the same manner, so they belong to
`docs/experiments/calibrated-plan/` §7's ladder straddle under load and not to
this round.

**Recommendation.** Keep the three fixes; they are what make any further
rotation or witness number readable. Keep the race off, and keep the code: the
round says exactly what would have to change for it to be worth arming, and it
is not the constants. Do not cut design C on the stated rule, and do not ship
its adoption.

Evidence, drivers and every battery: `docs/experiments/basin-race/`.

## The plan learned to re-price itself, mode 34 learned to stop, and the ten-second number turned out to be a quiet-box property

> **Corrected by the chapter after next ("The checkpoint was a report, not an
> interruption"), 2026-08-21.** The claim in this chapter's title that *mode 34
> learned to stop* is **withdrawn**, and with it the `m34cap` paragraph below.
> `ScheduleSliceRun::advance` recorded a checkpoint and left `finished = false`,
> and its caller looped `while !slice.finished` to the end of the monolith, so
> the coordinator never regained control at a checkpoint: the cap changed the
> checkpoint *report* and nothing else. Replayed at `work=30000000` on mixed-61
> seeds 0/1/2, `m34cap=0` and `m34cap=1` produce identical depth, fingerprint,
> work, operator-call count and per-slice step digest. What mode 34 learned in
> this round is to be **segmented**; it learned to *stop* in the later one.
> Everything else here — the concatenation gate, the struct, the step digest,
> the re-plan, the quiet-box retraction of `calibrated-plan` §8.2 — stands.

`docs/experiments/calibrated-plan/` §13.1 named the fix for the largest of its
three costs — *"install a provisional plan from phase 0, run to a deterministic
work checkpoint, then re-price the remaining wall at the rate the queue is
actually retiring units at"* — and declined to build it, because `v3_loop`'s
`run.deadline` and `Coordinator::protected_fraction` are both fractions of the
budget installed when the phase was entered. Sol review 8 §3 condition 4 named
the other half one level down — mode 34 is atomic and has no internal work cap —
and §4 spend 1 named the gate: **N concatenated batches must reproduce the
monolith at equal work.**

This round builds both, and the join is the point: **the deterministic work
checkpoint the re-plan needs is the batch boundary the slice needs.**

### Sol's gate, passed on two instruments

`drive_compression_schedule` is now a struct whose `advance` returns to its
caller between batches, and that shape is the load-bearing decision rather than a
tidy-up: the risk Sol names is that batching changes the trajectory, and a gate
can only find that if the implementation is *capable* of it. Everything the next
batch reads has to be a field — the frontier, the deepest-confirmed slot, the
lane's rng, its weights, every persistent worker's surrogate and pair-NFP cache.
Two things are deliberately not carried and both are correctness statements:
design B's per-step `stall_loss`, which does not cross a step boundary either,
and the tail confirmation, which belongs to the *slice* and would otherwise be
spent N times.

The gate needed a second instrument, because `ScheduleSliceReport` is an
aggregate — it drops the per-step rows, thousands per call — and a slice that
diverged at step 700 and re-converged by step 1,616 would pass a comparison made
on it. So the slice now computes a **step digest** over every row: the clamp, the
sweeps, the queries, the pair and boundary counts before and after, the
confirmation's three outcomes, the raw depth.

**21 cells, three batch sizes, two budgets, 1,741 batch boundaries, and every
cell equal as a whole document *and* as a per-step digest** — plus nine more
cells and 299 more boundaries on an earlier build of the same batching code. The
refactor itself is gated too: the resumable-slice binary reproduces the **base
commit's whole document** on all nine cells at a pinned work budget, with real
m34 slices in every one.

~~The consumer is `m34cap=1`: the coordinator hands each slice its own remaining
budget and the slice gives itself back at the first checkpoint past it, holding
its last exact-valid incumbent. At thirty seconds on mixed-61 that takes the p50
from **32.64 s to 25.91 s** and the overruns from 4 of 6 to 2 of 6, for
**3.089 mm** on one seed — the slices it stops are the ones that were paying for
that depth.~~ **Retracted; see the note at the head of this chapter.** The slice
never gave itself back, because the caller asked it again immediately. `m34cap`
computes a batch budget and the batch boundary then goes unanswered, so the two
arms are one trajectory measured twice. The claim that the checkpoint's
denomination in a counter makes the *stop* deterministic survives, because it is
about the mechanism the next round actually builds on.

### The re-plan, and two bugs it shipped and caught

A tranche reads the clock **once**, prices the remaining wall at the rate the
*queue* has been retiring units at — no bias divisor; that constant exists to
guess this very quantity — and snaps the new total onto the ladder the plan
already uses. The clock's influence is bounded by that ladder twice: on **size**,
because the total is a rung, and on **count**, because a tranche is refused
unless it clears the *next* rung. So a re-planning run whose remaining wall does
not buy a rung produces exactly the document a non-re-planning run produces.

Both of the round's constants were forced by failures its own gates found.
`PLAN_TRANCHE_HORIZON` exists because an unbounded tranche predicted 15.5 s of
queue from an 11.1 s window, the rate fell 42% below the reading, and a 30 s
target took **36.74 s**. Capping the horizon at the window then **stranded** a
run — mixed-61 seed 2 stopped with 5.7 s of ten unspent and three millimetres
behind the mode it improves, because a short first tranche leaves a window that
cannot justify a rung. A tranche may now always buy **one rung, and only one**, if
the remaining wall pays for it.

At ten seconds it works: **2.808 mm on mixed-61 seed 1, which is
`calibrated-plan` §9's ladder-floor cost for that seed to three decimals**, and
0.252 mm on the median of seed medians, spending 8.114 s of the target against
the single plan's 7.159 s, at the same 1-in-60 overrun count. Over six (fixture,
budget) rows of the anytime table the median gain is **0.000 mm** — one row moves
and five do not, and that is the honest summary.

At thirty seconds it does not work. The `plan` arm ran **41.15 s** against a 30 s
target and re-planning brought it to **37.14 s**: reduced by four seconds, better
on depth and on reproduction in the same cell, and still 24% over. The
first-tranche fraction introduced to fix that was measured and **rejected**:
`planfirst=0.6` bounds the worst case and moves the overrun into the median
(4 of 6 over at a p50 of 33.15 s against 1.0's 2 of 6 at 25.99 s), so the shipped
value is `1.0` and the sub-goal is recorded as a negative result.

### What the twenty-round battery says about the previous chapter

`calibrated-plan` §8.2's headline is *"one plan, one depth, one document per
seed"* over sixty runs. Re-run here, on a box that had a competing workload for
part of the window, **the same `plan=10000` arm produced 2 / 3 / 1 distinct
depths per seed**. Its modal depth still holds 85-100% of the runs, so the mode
is not broken — but *"a second process gets the same number"* is a **quiet-box**
property, and this is the first time the campaign has looked at it any other way.
The re-planning arm is worse on that axis (4 / 2 / 3, modal 80-90%) for a reason
the mechanism makes obvious: two clock readings can cross a rung where one could.
Neither is the `wall` arm, which produced **eight** distinct depths on one seed
at a modal share of 40%, twenty distinct documents on every seed, and 24 of 60
runs over target.

Four pinned gates hit with the **whole-document digest identical to the base
binary on all four**. Determinism across two processes is 9 of 9 at a work
budget, 9 of 9 in plan mode, and **8 of 9 as documents — 9 of 9 as layouts** with
re-planning, where the one document difference is an initial-plan rung straddle
that the re-plan then repaired to the same final budget and the same depth. Both
suites pass.

Evidence, drivers and every battery: `docs/experiments/replan/`.

## Consolidation: the counter tax was the clock reads, the wall stop bound one class of nine, and four claims were carrying corrections in the wrong place

> **This chapter skips three rounds, and the gap is the point.** The chapter
> above is `replan`'s. `robust-plan` (the `plancal` fix, and the 60/60 that *is*
> measured under load), `work-currency` (the 71,500x mispricing, and the
> currency's own null) and `real-interruption` (the `m34cap` retraction, and the
> interruption that is real) each produced a README and none produced a ledger
> chapter. Rather than write this one as though they had,
> `docs/shipped-surface.md` is the map that stands in for the missing three:
> every flag and key the campaign built, with a verdict and the evidence that
> earned it.

The owner's instruction after Grok review 5 (*"fermarsi e consolidare"*) and Sol
review 10 (*"il wall engineering può probabilmente portare 175 → circa 169; 169
→ 150 richiede una nuova azione di ricerca"*) was to close the shipped surface
before starting the active-contact block SE(2) research. Four items, and three
of them came back with a result the instruction did not expect.

### The millimetre was there, and the specified way to get it would not have worked

`calibrated-plan` §9 priced the work counters at **+1.882 mm** on mixed-61 at a
ten-second wall and closed with *"there is no version of this mode that avoids
it"*. `work-currency` §6 named the fix both reviewers then endorsed: lift
`surrogate_evaluations` onto the relaxed lane so modes 22 and 23 self-report,
and run the work budget with `profiling::set_enabled(false)`.

Measuring the meter before building anything is what killed that plan.
`work_units_from` is `CandidateQueries + 5 x ExactPairTests`, and on a measured
mixed-61 plan run those are 7,859,321 and 586,787 — so the **exact half is 27%
of the meter**, not the ~4% the compression schedule's own share suggested, and
it is counted in `kernel::exact`, a free function with no lane. A lane-local
candidate-query counter alone would have under-charged every work budget in the
engine by a quarter.

What was actually costing the millimetre is two lines apart. The lane's
`score_placement` opens a `Phase::ScorePlacement` span — two `Instant::now`
reads against a call that costs about a microsecond — and increments a counter,
one relaxed add on a thread-local block, and **one flag armed both**. Re-running
§9's own battery with a third arm the instrument of the day could not express
reproduces **+1.882 mm to four decimals** and splits it: the counting is
**+0.000 mm median on all three seeds** and the timing is the whole of it.

`profiling::metering_enabled` is the second flag and `lanedebit=1` is the
setting. The budget is numerically unchanged — same counters, same sites, same
amounts — which is what makes the A/B a comparison. Measured at a fixed `work=`
budget where the depth is identical by construction and the driver asserts it,
**documents identical field for field on 9 of 9 cells**, the debit retires the
same work in **84.9%** of the seconds at 24.9 M units and **82.5%** at 120 M.
That is `search::portfolio`'s own "~17% they cost" header, measured as a paired
ratio at identical work for the first time.

End to end it buys one of two things and the round reports both. Reading the
incumbent's calibration file: **identical depth and identical document on 3 of
3 seeds, p50 7.31 s → 6.27 s**. Reading a file a debit-armed calibration pass
wrote: **−1.108 mm** median of seed medians at ten seconds — one plan, one
depth, one whole document per seed, 0 of 9 over target, p95 8.87 s → 8.66 s.
The three per-seed deltas are 0.252 / 2.808 / 1.882 mm, which is one ladder
rung, which is why they are not a smooth 17%.

The cautionary arm is the debit with a *live* plan: same −1.108 mm, and seed 1
straddles the rung with two plans and two documents in three runs. A live plan
reads a clock, the debit moves the clock, and near a rung boundary that is a
coin toss. **The debit has to be calibrated into the file**, and the file a
profiler-armed pass wrote is not the right file.

### The wall stop binds eight more classes and still does not reach zero

`real-interruption` §13 named two reasons its thirty-second `wallstop` row still
crossed 3 of 9 times: the policy binds only the mode-34 checkpoint it is
consulted at, and it cannot shorten a batch already in flight.
`m34wallstopall` answers the **first** — an admission rule in seconds at the top
of the v3 queue and in `Coordinator::affordability`, exiting on a new `wallStop`
cause that is deliberately outside the re-plan loop's budget-bound set, because
a run out of seconds must not be handed more work.

On a forced overrun — `planhead=3.0` buys a plan a ten-second wall cannot pay
for — it is decisive: worst overrun **+26.63 s with no policy, +20.05 s with the
shipped checkpoint stop, +0.99 s with the queue rule**, 6 of 6 exact-valid, 6 of
6 exiting `wallStop`. The `checkpoint` row is the measurement of §13's first
sentence: it takes 6.6 s off a 26.6 s overrun and leaves 20.

On the calibrated thirty-second battery **the deliverable was 0 of 9 and it was
not met.** The count goes 3/9 → 4/9, which is inside the noise of a box running
at median load 9.57 with a spike to 21.7. What collapses is the *size*: worst
overrun **+12.38 s → +1.31 s**, wallMax **42.38 s → 31.31 s**, at **+0.000 mm**
of median depth and 9 of 9 exact-valid. The residue is exactly §13's second
sentence — the one action in flight when the deadline passes — and bounding it
needs an operator that can be interrupted mid-action, which is Sol review 10's
governor round and which the owner deferred.

The reserve dial is a **negative with a nameable cause**. `m34wallreserve=1`
refuses a class whose measured mean seconds would not fit, and it is worse than
the plain admission rule (+1.87 s against +0.99 s). The rows say why: at the same
plan it consistently runs one more action and 3–5 M more work, because
`m34wallstopall` arms the mode-34 checkpoint stop as well, so mode 34 is the one
class that can give its turn back mid-action — and the reserve, pricing classes
by mean seconds, refuses precisely that one near the deadline and buys an
uninterruptible class instead.

### The provenance break closes by rebuild, and four claims move to where they belong

Sol review 9 required *"clean rebuild + evidence rigenerata"* before any
promotion. A clean build of the base commit reproduces
`real-interruption/evidence/binaries.txt`'s two sha256s **byte for byte**, so
the previous round's binaries and the committed tree agree and every number it
published is comparable to every number here.

Four claims were carrying a correction three documents away, or none.
`calibrated-plan` §8.2's *"60 of 60"* now says at the claim that it is a
quiet-box property and points at `plancal`, whose own 60/60 is measured under
load. §9's *"no version of this mode that avoids it"* is struck. `replan` §3's
*"two slices with the same digest walked the same walk"* is struck, with
`real-interruption` §4's three SHA-256 fingerprints as the repair. `basin-race`
gets two banners: the race's verdict stands and §4.3's landslide explanation
does not — Sol review 9 names four defects each sufficient on its own, including
a ranker that is not dense and a `confirmations_attempted == 0` that scores the
maximum rather than the neutral value — and §5.4's *"the witness composes"*
overstates a 2/12 whose driver defines descendant as `final(adopt) <
final(publish)` on arms 2.8% apart in work. `grok-review-4`'s verbatim text is
left unedited with a header note, because it is a record of what was said.

`cargo fmt` was 158 diffs across 17 files, all of them files the campaign
touched, and it took two passes to converge. It is **not** whitespace-only —
trailing commas, one closure's braces, and `use` items reordered within their
group — so the claim made for it is *"the documents are identical"* and not
*"the bytes are"*: the four pinned gates hit on the reformatted binary with
whole-document digests identical to the base and pre-format binaries on all
four, and the work-budget equivalence gate is 9 of 9 with step digests equal.

Determinism across two processes is 9 of 9 at a work budget both unarmed and
with `lanedebit=1`. The suites pass, and neither passed first time: the combo
ran red on a race **this round introduced** - two tests in one process both
calling `profiling::reset()`, which the file had been handling by convention -
and the jagua on the campaign's known allocator-capacity flake. Both attempts
are reported.

One thing fell out of checking that the suite counts add up, and it is worth
more than the round that found it. The counts decompose to +1 and +2 exactly,
and the third test this round added is in neither suite: **`cargo test` builds
an example and does not run its test harness**, so no spec-key round-trip test
in this repository has ever been reachable from either suite the protocol names
- including the one the previous round added specifically to catch a key nobody
parses, which is the `m34cap` failure mode. A third suite now runs them
explicitly. That does not make them unmissable, and saying so is the point.

Evidence, drivers, binary hashes and every battery:
`docs/experiments/consolidation/`. The map every future session should start
from: `docs/shipped-surface.md`.
## Active-contact block SE(2): the last idea in the operator space, and the gate it fails

Sol review 10 §3 named one search action outside the closed space
`{m20, m22, m23, m26, m31, m33, m34 + continuous/sparse rotation + overlay +
race}`: build the graph of near-binding contacts touching the depth-setting
pieces, take a small connected component containing the setter, propose one
joint `(dx, dy, dtheta)` for the whole block inside a trust region, apply it as
ONE action, and re-derive the graph and repeat. Both reviewers agreed it was the
last idea in the current space and that its honest failure would complete the
map. It was built, gated, and it fails.

**The operator is real and it is not any of the three things it resembles.**
`search::general_micro_legalization::contact_block`, feature
`contact-block-se2` stacked on `se2-rigidity-certificate`, off by default and
reachable only through `POLYGON_NESTING_CONTACT_BLOCK` on the benchmark's
diagnostic door. Three variables per block piece and none per pinned piece, so a
five-piece block is a fifteen-variable program; rows built from the same exact
closest-approach witness and the same chord-error relaxation the SE(2)
certificate uses, with a row between a block piece and a pinned piece keeping the
block piece's coefficients and dropping the pinned one's. It is not m33, which
moves one piece and hopes the operators downstream compose an exit; it is not
`global_legalize`, which is translation-only; and it is not
`se2_witness_proposal`, which solves the whole layout in one shot at a radius
small enough that the answer is the box. The restriction to a block is what buys
the large radius and the round-by-round re-linearization, and it is the only
reason a strictly smaller feasible set could ever beat its own relaxation.

**THE FIRST PASS WAS WRONG AND THE RETRACTION IS THE MORE USEFUL HALF OF THE
ROUND.** The line search validated its steps with `validate_publication` — the
*contract* half of the engine's acceptance check, and exactly what
`se2_certificate`'s witness line search uses. The engine's authority is
`general_fast::validate_and_measure_placements`, which is that check plus
canonical-collision-grid admissibility, and `general_relaxed.rs:6413` runs it on
any parent handed to mode 34. At the certificate's 0.025 mm radius the two never
disagree; at this operator's 0.5 mm they disagree on every parent. The wrong gate
reported a median **0.506 mm** across twelve pinned parents with 12/12 seeds
moved — and **all twelve outputs were refused as parents**, "pieces ... overlap
on the canonical collision grid". The right gate reports **0.044 mm**. What did
not catch it was the round trip built to catch it: it read `exactValid`, got
`False`, compared against a control that also read `False`, and concluded the
output was judged as its parent was. The two `False`s had different causes — the
control's was a target the driver had set above the parent's own depth, the
operator's was outright refusal — and one boolean covered both. What caught it
was a composition test noticing that `blockThenM34` equalled `blockOnly` to the
last digit on all twelve seeds. The driver now asserts on the `failureReason`
prefix, and `BlockRound::contract_only_accepts` counts the difference per round
so the correction stays in the data. This is the same methodological failure the
joint-replacement negatives paid for once already, reintroduced in a new shape.

**The gate, corrected.** Twelve pinned from-request parents at 171.614-179.620,
block against the shipping m34 continuation from the same parent at
`past=1,rollback=0,work=W,lanes=1,pconfirm=0`, three cost axes because the arms
do not spend work in the same shape. Median block drop **0.0438 mm** at 1919
composite validations and 1.18 s; median control drop **1.1044 mm** at 287
confirmations and 3.46 s. Clause one, ">= 2/3 seeds moved": the block is
strictly shallower on **1 of 12**, and that one is seed 3 by 0.0005 mm and only
because the control found nothing there at all. Clause two, "net mm/work
improvement": the paired ratio of millimetres per composite validation is
**0.0030**. On the block's own best axis — both operators priced alone, the
block's `elapsedMs` against the slice's `repairMs + confirmationMs`, at 1180 ms
against 1125 ms - it buys 0.021 mm/s against the slice's 0.967 mm/s and leads on
**0 of 12** seeds. More budget cannot rescue it: the round budget is never
exhausted, the loop exits on the trust-region contraction floor, and
`rounds=4096` is *worse* than `rounds=64` while the control goes 0.000 -> 0.168
-> 1.104 mm as its cap triples.

**The decomposition, which is what a research round owes.** Of Sol's three
candidate explanations the answer is the second, sharpened to a mechanism. *No
components* is refuted: a component of at least two pieces is found on **99.8%**
of rounds, median size 5, median 5 contact edges. *Blocks rejected by exact* is
the answer: **44%** of rounds cannot take any step of any length, the model's
full-length vector survives the composite gate **one time in four**, and the
median accepted scale is **0.01** — one percent of the model's own vector, which
leaves sub-micron motion against a model upper bound of 0.18-0.23 mm. *Gains
dominated* is the second-order effect: 22-27% of rounds move the block a
validated distance and the published depth does not fall, and 12-20% of the
rounds that do move land exactly at their own headroom, the published depth
minus the deepest piece the block does not contain, whose median is 0.0036 mm.
The mechanism in one sentence: **the binding constraint is the canonical
collision-envelope grid, not the material contract, and the program models the
envelope gate as a first-order contact at zero clearance, so it has no margin to
linearize.** The knob sweep confirms it as a trend rather than an average — eight
times the solver iterations makes one parent *worse*, because the better the
linearization is optimized the further outside the true envelope constraint its
optimum sits.

**What the round leaves standing.** Four pinned gates hit on both binaries with
every field of all four documents identical except `executableSha256` — 3246-3265
fields compared per gate. Determinism across two processes is **12 of 12** as
whole documents including the moved placements. All three suites pass with zero
failures - 1293, 1356 and 1377 tests - and the campaign's known-flaky
`free_material_multi_eviction...` passed first time in each. Of the twelve parents eleven
produce a proposal at all, and of those eleven **zero are refused as parents**
and **eleven have their depth reproduced by the engine's own independent
re-derivation to the 1 um grid**. This crate does not build bit-reproducibly, so
the evidence's provenance is established by re-deriving rather than by comparing
hashes: a fresh build of the committed tree reproduces all twelve depths **to the
last digit of the f64**. Composition is the only axis with a positive
sign and it is small: block-then-slice is shallower than slice-alone on 8 of 11
seeds, median **-0.030 mm** at about 3% more work, with a spread from -0.395 mm
to **+0.659 mm** on the seed where the slice stalls completely on the block's
output and republishes it unchanged.

**What it does not close, stated so nobody has to re-derive it.** A block program
whose envelope rows carry a real margin - asking the envelopes to separate by
some epsilon rather than merely to miss - would have slack to linearize and might
take steps the composite gate accepts. That is a different program from the one
Sol specified, it was not what this round was funded for, and §4's mechanism
points straight at it.

Evidence, drivers, both passes and the retraction:
`docs/experiments/contact-block/`.

## Gate A: Sparrow's 150.165 is contract-legal, the miter envelope refuses it, and the join is 100% of the reason

Grok review 6 §2 named a one-round diagnostic that nobody had ever run: import
the committed Sparrow 10-second x86 solution through the committed converter and
take three verdicts on the *same poses* — contract only, composite miter (the
acceptance authority), composite round — then read the answer off its own
interpretation table. This is that round. No default changed, nothing was
promoted, no search path was touched; the deliverable is three verdicts and what
they mean. Kimi review 1's margin note is settled in the same round.

**The verdicts.** On the imported pose set, at the from-request envelope radius
2.502 mm: the raw-source contract validator **accepts**; the composite miter
validator - `validate_and_measure_placements`, what HEAD accepts with - **rejects**
on 37 of 1830 pairs and 4 of 61 boundaries; an experimental round-join envelope at
the same radius rejects only 2 pairs and no boundary. At the *contract* radius,
`total_padding/2 = 2.5 mm` - the radius Sol review 11 and Grok's §A.1 both name -
the round envelope **accepts the layout entirely**, 0 of 1830 and 0 of 61, while
the miter still refuses 31 pairs and 2 boundaries. **31 of 31 pair refusals and 2
of 2 boundary refusals at the contract radius are caused by the miter join shape
alone; not one is caused by the envelope radius.** The two extra refusals at the
shipping radius *are* radius-caused - their material clearance, 5.000840 mm and
5.002879 mm, is under `2 x 2.502` - and that is a separate and much smaller tax:
0.004 mm of pair clearance against the join's 0.5057 mm median on a refused
pair, a factor of 126. It is the only part a round kernel would not remove.

**The instrument, and why a boolean was not the deliverable.** "The composite
rejects" is compatible with the join rejecting and with the radius rejecting, and
those demand opposite spends, so the shadow measures a quantity that separates
them: the **critical radius** `r*(i, j)`, the largest integer-micrometre radius at
which a pair's two envelopes still have zero intersection area. Offsets are
nested and increasing in `r`, so it bisects; the canonical grid step *is* one
micrometre, so it is exact rather than interpolated. For an exact disc join
`2 r*` **is** the material clearance; for any join containing the disc the deficit
`d - 2 r*` is the material clearance the representation spends on that pair and
cannot give back. Priced that way the miter costs, on the 31 pairs it refuses at
the contract radius, a **median 0.5057 mm and a maximum 2.3343 mm**; the round
join's own figure over the same rows spans [-0.0012, +0.0022] mm, entirely
inside the grid's derived quantization budget of 0.0036 mm - so a round envelope
costs the material nothing the grid can measure, and the miter's median cost on
a refused pair is 140 times that whole budget.
The worst row is not a tight one: items 21 and 57 carry **7.0843 mm** of material
between them - 2.08 mm more than the contract asks - and the miter grid credits
them **4.750 mm**, less than the 5.0 mm the contract itself requires.

**The boundary asymmetry, which Grok asked for by name.** The contract demands
5.0 mm of material edge clearance on all four sheet edges, which is exactly what
Sparrow was validated at. A round envelope demands `inset + radius`, flat -
5.002 mm at the from-request allowance. A miter envelope demands
`inset + k * radius` where `k = 1/sin(half-angle)` capped at
`CLIPPER_MITER_LIMIT = 2.0`, and **k is a property of the pose, not of the
contract**: measured on this layout it is 1.19973 and 1.22318 on the two refused
placements, so the demand is **5.5017 mm and 5.5604 mm** where the contract says
5.0, with a structural ceiling of `inset + 2 x radius = 7.504 mm`. The allowance
asymmetry is +0.002 mm; the join asymmetry is +0.56 mm.

**Grok's case 3 obtains** - miter rejects, contract accepts - so the
representation *is* the residual, and Sol review 11's Certified Round-Envelope
Kernel is the named spend. Case 4 (miter accepts, only an engine replacement
remains) and case 5 (contract rejects, 150.165 stops being comparable) are both
excluded by measurement. What this does **not** license, stated so nobody has to
re-derive it: a round authority would stop *forbidding* 150.165, not start
*finding* it - Grok's §1 is untouched, the constructor still saturates near
180 mm in 1.4 s and 40 M -> 120 M work still buys 5.964 mm. Nor does it
discharge Sol's own gate: Sol's item 1 asks for the round shadow against the
source-ring validator on three populations - the canonical corpus, the committed
material-valid/canonical-invalid proposals, and a +-1 um boundary sweep - at zero
false accepts, and this round ran one pose set that offered no opportunity for a
false accept at all, because the contract accepts every row of it. And the grid
is the floor under all of it: at radius 2.5 the round envelope admits the layout with
**exactly zero** grid margin on pair 38-39, whose 5.000840 mm of material leaves
0.42 um of radius margin - below the 1 um grid step - so Sol's outward-only
discretisation with the error inside the margin would refuse that one pair. The
miter join is the whole of the multi-millimetre refusal; the last micrometre
belongs to the grid.

**It is one layout** - n = 1 pose set, 61 placements, 1830 pairs. That is enough
to falsify "the legal set already contains Sparrow", one counterexample being
all that takes, and enough to price the join on the rows it refuses; it is not a
distribution over layouts and nothing here reports one. What generalises is the
mechanism - `offset_miter(P, e)` strictly contains `P (+) disc(e)` at every
convex corner - and that it is worth millimetres rather than micrometres where
it bites. What does not generalise is "31 pairs".

**Legality is not reachability, and the second barrier is already in the tree.**
Sparrow ran with continuous rotations, and **57 of the 61 imported poses are off
the engine's 2.5 degree surrogate-angle lattice** (`SURROGATE_ANGLE_STEP_DEG`,
`general_relaxed.rs:75`; `canonical_angle` snaps to it), worst deviation
1.24586 degrees, 59 distinct rotations over 61 pieces. A round envelope would
stop *forbidding* these poses; the default relaxed lane still could not
*propose* them. That is orthogonal to this round's finding and to Sol's kernel,
and its own arm is already priced - `continuous-rotation` measured -3.7 mm at
ten seconds, `sparse-rotation` a null. Anyone costing the round-envelope kernel
should cost this alongside it rather than after it.

**The import was audited before it was used**, because a conversion artefact
would have faked all three verdicts. The committed converter is loaded as a
module and pointed at the 10-second solution rather than re-implemented; every
placement is then re-derived independently. Sparrow `items[i].id == i` and
`items[i].dxf == request.pieces[i].id` on all 61, every ring is vertex-for-vertex
the request's own source chain (neither frame recentres), the rigid map
`(x_e, y_e) = (2000 - y_s, x_s)` has determinant +1 and no mirror, and the worst
vertex error over all 61 transformed rings is **2.27e-13 mm**. Against the
committed validation: the minimum pair is `[38, 39]` in both, at
5.000840472766861 here against 5.000840472766719 there (**1.4e-13 mm**), a second
hand-verified pair `[50, 52]` agrees to 1.0e-13 mm measured in *both* frames, and
the bounding extents agree to 7.1e-15 mm. One committed number does not transfer
and the reason is recorded: Sparrow's `minimumBoundaryDistance` 5.00096 is its
*far* strip edge, which maps into the interior of the engine's 2700 mm sheet, so
the engine-frame binding edge is the long-axis origin at 5.002254 mm. In the
engine's published convention the imported pose set measures **150.16451 mm**,
0.00096 mm under Sparrow's reported strip width.

**Kimi's stale bound, re-pinned.** `depth-lower-bound-evidence.json`'s
131.97838540260466 mm belongs to the retired 5.5/5.25 contract. The same
construction under exact-clearance 5.0/5.0 gives **130.19990218310795 mm**
strengthened (125.19990218310794 plain), and **130.2140326353513 mm** for the
composite authority that actually publishes - so the envelope adds 0.0141 mm at
the bound level and nothing else. The check that says only the constants moved is
that the certified `r = 2.5` inflated area agrees with the retired file's own
`sparrow_bound_mm x 2000` to **0.0 mm^2**. Kimi's suggested replacement, 124.887,
is *not* the right figure either: it is `SUM_2.5 / 2000`, the full width with no
boundary term and no depth-metric term, written as a calibration of an outside
packer. And the finding that dies with the old contract is the 7.09 mm of
"contract overhead": Sparrow's separation and this branch's are now the same
5.0 mm, so there is one bound and not two. The bound has never been binding on
this instance and still is not - Sparrow sits 19.965 mm above it, the record
25.064 mm.

**What the round leaves standing.** The shadow is `#[cfg(feature =
"import-gate-shadow")]`, off by default, named by nothing in `src/` outside
itself, and reaching no search, scoring or publication route;
`PolygonSet::offset` is byte-for-byte unchanged and the shadow **asserts at run
time** that its own miter configuration reproduces it on every piece it measures
before trusting a round number - which held on all three radii. The strongest
check is a different one: `validate_and_measure_placements` short-circuits and
names the *lowest-indexed* placement whose envelope leaves the inset sheet,
while the shadow enumerates, so the shadow's lowest-indexed boundary failure has
to be the piece the real validator names - and it is, on all three radii. That
is what says the shadow's envelope half **is** the composite's envelope half
rather than a second implementation agreeing in aggregate. Three more soundness
checks ran on every row: containment (`r*_miter <= r*_round`) holds,
with 9 rows inverting by *exactly* one 0.001 mm grid step and none more, which is
the inscribed-arc floor; the disc identity `d - 2 r*_round` lands in
[-0.001243, +0.002226] mm against a derived quantization budget of 0.003614 mm;
and the failure counts are monotone in the radius. Clipper's default round-join
arc tolerance would have been `radius/500` = 5 um - five grid steps, six times
the margin under measurement - so it is set explicitly to 0.0001 mm and the round
envelopes carry 20 601-20 669 vertices against the miter's 377. Rows whose
bisection saturates its ceiling are labelled and excluded from every statistic;
no pair row saturates. Four pinned gates hit on a binary rebuilt from this tree,
and all four suites pass - **1294**, **1358**, **19** and **1299** tests, zero
failures, the fourth being suite 1 plus the five unit tests this round added to
the shadow. The campaign's known-flaky
`free_material_multi_eviction_shrinks_retained_container_capacity` tripped once
here (`assertion failed: cache.entries.capacity() < entries_capacity_before` -
an allocator property, not a search one) and passed on the protocol's rerun;
both runs are kept, and `run-suites.sh` now performs that rerun itself. One
pre-existing condition is recorded rather than worked around: suite 4 is stacked
on `jagua-experimental` because `examples/general_request_benchmark.rs` names
`search::portfolio` and declares no `required-features`, so **any** feature set
without `jagua-experimental` fails to compile it - verified on the base commit
with `cargo check --features shadow-rescore --examples`.

Evidence, drivers, the audited fixture and the named refusals:
`docs/experiments/gate-a-sparrow-import/`. Re-pin:
`docs/experiments/depth-lower-bound/depth-lower-bound-exact-clearance-evidence.json`.

## The m26 band audition: the ladder reproduces, loses to its own shipped port by 8.4x, and is cut

Kimi review 1 §1 named the one cell in this campaign's map with a measured
multi-millimetre gain and no matched-arm gate: mode 26's short ladder in the
171-179 mm band, where the plan's arm C published -4.9571 and -4.3170 mm on two
of three seeds and where "non esiste ne un positivo gated ne un negativo gated".
It pre-committed the gate, the kill rule and a ~50/50 expectation. The gate ran.
**The arm is cut, at five of five control budgets, 0 of 12 parents below the
control at matched work** - and the interesting half is why, because the
mechanism claim was right.

**Arm C reproduces, lifted out of the coordinator.** Grok review 5 archived arm
C as an artefact of its precondition, a saturated archive after ~16 s of v2. Run
from the twelve pinned from-request parents instead, the uncapped drop-1.0
ladder publishes **-5.7266 mm on seed 0 and -8.2890 mm on seed 1** against arm
C's -4.9571 and -4.3170, and nothing on seed 2 - the same two-of-three, larger.
The precondition argument is falsified: the gain survives the lift.

**And it loses anyway, to the control, at every budget.** The arm is one m26
rung, which is exactly the drop-1.0 ladder truncated at the anatomy's own
equal-budget figure of 33,413,789 work units, because `ladder_compression_bounds`
turns any drop from 0.175 to 1.4 mm at a 174 mm parent into the same 0.174208 mm
rung-1 bound. Median drop **0.2332 mm against the work-matched control's
7.0129 mm**; in the currency Kimi insisted the arm be priced in, **0.1547 against
1.2991 mm per million coordinator work units - 8.4x**. The control at that
budget spends 4.5% *less* work than the arm. At one tenth of the arm's work the
control's median is *still* better, 0.2534 against 0.2332. The uncapped six-rung
ladder buys 3.3784 mm for 45.5 M operator units where the control buys 12.1095
for 17.5 M. There is no budget in the measured range where the ladder is the
better use of the coordinator's work, and the ladder's yield per work unit
*falls* as rungs are added (0.1547 -> 0.0756), so a longer one moves away from
the control rather than toward it.

**The reason is that the follow-up the review reserved for a pass had already
shipped, and it is the control.** Kimi §1 closes that even a passing arm would
need "il porting del rung ... il design di `mode26-rung-anatomy` §3". That port
is `compression_schedule.rs` - its own module documentation says so - and it is
reached as mode 34, which is what `matched.py`'s control arm runs. The audition
was therefore never a new action against the shipping schedule; it was **mode 26
against its own port**, and the port dominates its parent mechanism on the band
the parent mechanism was best at. There is no porting price to state because
there is no porting left to do.

**Three measurements the round corrects, all against itself.** First, the
anatomy's 85.4% rollback-tracker abort rate does **not** transfer: 1 of 14 rung
arms here (7.1%), 23 of 113 over the six-rung ladders (20.4%), and 6 of 14 arms
producing an exact-valid state against the anatomy's 0 of 171. A rung at 174 mm
is also 8x cheaper than one at 159 mm, 4.09 M operator units against 33.4 M,
because it publishes on its first arm instead of grinding through 4.9. The
honest consequence is that the arm's loss cannot be blamed on its known bug:
repairing the one-f64-ulp rule recovers a fifth of a 20% abort rate against an
8.4x deficit. Second, **a sub-grid difference in the rung's bound is worth
6.5 mm of outcome.** The round's first pass derived the single-rung target from
`rawDepthMm` where the ladder uses the grid-snapped `independentDepthMm`; on
seed 10 both passes still ran exactly one rung, with bounds 2.235e-4 mm apart -
22% of one canonical grid unit - and published 169.5948 against 176.0952. That is
the review's own §0 thesis, the endpoint dominated by trajectory luck, measured
on the review's own mechanism, and it is why no verdict here is read off a single
m26 cell. Third, `matched.py`'s process work meter carries a **9 M-unit floor**
that belongs to neither arm: a mode-34 process that refuses its mode and runs no
search at all burns 6.84-11.91 M units in phase 0. Read raw, the arm's 13.00 M
against the control's 12.39 M looks like a fair fight; net of the floor it is
4.09 M against 3.91 M and the control is the one being starved. Any future
matched-arm gate on this harness should subtract it; it costs one 2.5 s process
per seed to measure.

**What the round leaves standing.** No engine code changed - the diff is
`docs/experiments/m26-band-audition/` and nothing else - so the four pinned gates
on a rebuilt `jagua-experimental` binary are a check that the measurement was
taken on the tree it claims: 206.869/`8a7737381238fa4d`,
159.09233022733062/`fa01012af1d559ae`, 159.07876040364795/`e28fba007f8031d4`,
164.0375677990678/`49f094d7e59a9008`, all four hit - and they were run twice on
the same binary, either side of the commit, with all four documents identical
field for field across 3246-3265 scalars except `engineCommit` and
`engineWorktreeDirty`. All twelve parents clear the
authoritative publication gate and reproduce their pinned depth and fingerprint.
Determinism across two processes is **6 of 6** whole documents on three cells for
both arms, placements and work counters included - after three `searchProfile`
fields had to be added to the strip list by name, since `gatelib.strip_times`
misses `milliseconds`, `leafMilliseconds` and `leafSharePercent` and the first
run reported 0 of 6 on wall clocks alone while every depth, fingerprint and
counter already agreed. All three suites pass with zero failures - 1293, 1357 and
19 tests, the third being the example harness the other two do not run - and the
campaign's known-flaky `free_material_multi_eviction...` passed first time in
both library suites.

**What it does not close.** The brief pre-committed drop 1.0, six of the eight
rungs `LADDER_COMPRESSION_STEPS` allows; a ~1.4 mm drop would buy the other two
and was not run, on the argument above that added rungs are worse per work unit,
not better. And the retirement is of m26 *as a spend in the 10 s band on this
request*: the mechanism is not deleted, it is priced, and re-opening it needs a
new mechanism rather than another sweep.

Evidence, drivers, both passes and the retirement entry for
`shipped-surface.md`'s board: `docs/experiments/m26-band-audition/`.

## The dual review of Gate A and the audition: five overclaims corrected, the product question named first

Sol 12 and Grok 7 reviewed the merged round from the raw evidence (both
verified the counts, the import fidelity at 2.27e-13 mm, and the re-pin
digit-for-digit; Grok re-derived the refusal-cause attribution from
`summarize.py:decompose` and called it sound). Five formulations were too
strong and are corrected in place — `d−2·r*` renamed an effective quantized
tax with the cause resting on the full-scan counterfactual; the round
deviation measured on 15 of 31 rows, not all; the grid budget re-derived at
≈5.0 µm (observed 2.2 still passes); the audition kill tally stated exactly
(strict 5/5, weak 4/5, designated control both); the Grok-5 falsification
narrowed to the in-process-artefact half, with the economic precondition
confirmed. Errata sections in both READMEs; the m26 retirement row applied to
`shipped-surface.md` §3 (it had been drafted in the audition README and never
landed — Grok caught the gap).

Both reviewers rank the same next step and it is not engine work: **the
product question — is `JoinType::Miter` at limit 2.0 an immutable half of the
publication AND?** Written answer required before any spend. If immutable:
stop on 150@10s (the operator space is measured out and the legal set
provably excludes Sparrow-class layouts). If the join may change: Sol's
certified kernel with unmodified gates — zero false accepts on three
populations plus ±1 µm cases, ≤1.25x cost, ≥8/12 at equal operator-wall — and
Sol adds the economics kill nobody had priced: the shadow round offset
produces 20.6k vertices against miter's 377 (55x); a shippable kernel is a
hybrid (cheap broad phase + analytic arc distance at the margin), not
Clipper-round-in-the-candidate-loop. Off-lattice reachability is costed in
the same round as a co-requirement, not a sequel (57/61 poses off both the
2.5 and 1.0 degree lattices; crot remains a measured negative). Neither
reviewer funds an m26 follow-up, a crot revival, or a promotion A/B on the
inscribed shadow. Three barriers stand between a legal round envelope and
150@10s: legality (this round), reachability, and search economics (the 13 mm
from wall 168.5 to record 155.264 lies *inside* the current legal set and is
a budget/basin problem 10s does not buy).

Reviews verbatim: `docs/sol-review-12-the-effective-tax-and-the-hybrid-kernel.md`,
`docs/grok-review-7-the-product-question-first.md`.

## The certified round-envelope kernel: exact, zero false accepts, 8x cheaper — and the grid step it exposes is the miter's

Sol review 11's kernel, built to Sol 12 §3.2's kills with Grok 7 §2's addition
(*"inscribed Clipper round is not this kernel"*). It replaces the composite's
**envelope half only** — `P ⊕ disc(r)` instead of a Clipper miter offset at limit
2.0 — and the material contract validator is untouched and still final.

**It is exact rather than outward-discretized, and that is a correction to Sol's
own specification.** Gate A measured the row an outward approximation cannot
serve: Sparrow pair 38·39 has 0.42 µm of radius margin at `r = 2.5`, below the
1 µm grid step. So there is no approximating polygon at all: canonical rings are
integers on a 1 µm grid, squared segment distances between them are rationals,
and `d² ≥ (2r)²` is one `i128` comparison after cross-multiplication. No `f64`
enters a decision; the verdict is a function of the integer grid alone, with no
rounding mode and no error budget. Containment is asked separately, because
minimum boundary distance alone false-accepts a contained piece. The domain
bound is evaluated in a unit test at its own extreme rather than argued, and the
kernel fail-closes (to the miter authority) outside it and at zero expansion.

**The battery.** Zero false accepts: 194 material-valid / canonical-invalid
proposals, 82 kernel accepts, 0 below the contract validator's own source-ring
clearance outside the derived √2 µm canonicalization band (3 inside it, reported
as their own count). The Sparrow differential meets all four pre-committed
expectations — at `r = 2.500` the kernel accepts all 1830 pairs and all 61
boundaries and **pair 38·39 with it**; at `r = 2.502` it refuses exactly the two
radius-caused pairs Gate A named and zero boundaries. Thirty ±1 µm sweeps,
windows centred on the located crossing: 30/30 monotone in the material
clearance, 0 steps disagreeing outside the band, flip within one grid step on
30/30.

**The finding.** On the canonical corpus — 17 layouts × 3 radii, 93 330 pair rows
and 3 111 boundary rows against `PolygonSet::offset` itself — the exclusive
kernel loses 13 rows. Every one at exactly **−1 µm**, every one with the miter
envelopes' measured intersection area exactly `0.0` mm², and on 11 of the 13 the
untouched source-ring clearance is *itself* below the composite's own demanded
`2r`. The attribution is a proof: `offset_miter(P, r) ⊇ P ⊕ disc(r)` exactly, so
an exact distance below `2r` means the **true** miter envelopes overlap — and the
only thing between that and a measured zero is `do_round()` re-quantizing the
offset output to the canonical grid. **The shipped miter authority is one grid
step permissive of its own declared envelope at contact, and the
short-side-first constructor places pieces exactly at contact**: with the
exclusive kernel armed, a bare-request run aborts inside
`general_fast::validate_result`, the constructor's own self-check refusing the
layout the constructor just built.

**So the shippable form is the hybrid Sol 12 §3.2 asked for.** `rek=1`
(`KernelMode::Union`) admits what either half admits: it cannot lose a
canonical-valid layout (0 of 51 cells, all three radii), adds no false-accept
surface beyond HEAD's own, and asks the cheap authority first. `rek=2`
(exclusive) stays as the certified-exact measurement arm.

**Economy.** The envelope half is **0.122x** median (8.2x cheaper, worst cell
0.136x): no offset is built, 290 canonical integer vertices against the miter's
377 and against the 20.6k of Gate A's inscribed shadow, and an integer box gap
certifies 95.34% of pairs. A whole confirmation is 0.905x (exclusive) / 1.008x
(union); with `fast-contract-validator` armed — the shipping-relevant
configuration, since the contract half is ~90% of a confirmation — it is
**0.471x**, a median 0.8126 → 0.3737 ms. Sol's ≤1.25x is met in every reading.

**Gates and suites.** All four pinned gates hit on a fresh feature-off build
*and* on a build carrying the feature compiled but unarmed, with the two
binaries' whole-document digests identical per gate. Four `--release` suites
green (1293 / 1357 / 19 / 1307) plus a supplementary debug run of the kernel's
14 tests, which is the only profile that compiles its domain `debug_assert!` in.
Determinism holds across two processes and, separately, across two *binaries*
built with different feature sets: `equiv.py` requires `battery.json` and
`battery-fcv.json` to agree on every verdict and reports 0 differences, which is
also this round's own confirmation that the contract certificate is
verdict-preserving.

**What it does not close.** No search ran. The twelve-parent matched gate at
equal operator wall — Sol 12 §3.2's remaining kill, ≥8/12 and ≥1 mm median — is
untouched and is the next round's assignment; the handover, including the fact
that an armed run's `used_long_axis_depth_mm` moves to the round envelope's
basis while the raw source depth does not, is `README.md` §7. Legality is still
not reachability. 464 insertions and 0 deletions against the seven pre-existing
files.

Evidence, drivers and the full caveat list: `docs/experiments/round-envelope-kernel/`.
