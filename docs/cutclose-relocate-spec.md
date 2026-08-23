# CutCloseRelocate — the quorum spec (2026-08-23)

Both consultants read Sparrow's source (rev `14f4868f`, upstream-identical,
paper arXiv:2509.13329) under the owner's authorization, designed
independently, exchanged verbatim, and **signed the same member**: Sol
withdrew his coordinated-projection design in round 2 ("Grok is right on the
main member"); Grok's M1 refusal targeted that withdrawn design and is moot.
**The spec of record is Grok review 12 Round 2 §6 ("The spec we would both
sign") as amended by Sol review 17 Round 2 §4-§5, with the four arbitrations
below.** The no-copying ruling both sign: the member implements the PUBLISHED
Algorithms 4-13 of the Sparrow paper, cited, on OUR state types, OUR
deterministic sampler, OUR source-ring signed-gap Φ, OUR exact round kernel
and untouched contract validator, no jagua-rs, no copied source text — the
same literature-derived practice as Egeblad/Imamichi. A provenance table
(concept → paper algorithm → source-confirmed default → our difference) is
part of the spec commit. Sparrow's own timer EXCLUDES import+LBF; ours starts
at the bare request — documented, not compensated.

## The member (frozen)

- **Relocate** (Alg. 5-6 analogue), colliding pieces only: 25 focused samples
  (current AABB) + 50 container-wide (usable strip at current T), current
  pose always in the pool, 16 sampled orientations + continuous CD wiggle,
  3 unique finalists (0.05·min_dim, 1°), two-stage axis CD on incident
  weighted Φ with accept-equal, commit the best — **no leftover
  after<before, no ladder_top cap on a relocate**.
- **GLS** (Alg. 8), all rows every master iteration, published multipliers on
  our v: active w *= 1.2 + 0.8·(v/v_max); inactive w *= 0.95, floor 1.0.
  One dialect; the stall-only integer increment dies.
- **Disruption** (Alg. 12 fail path only): swap two large pieces (cumulative
  convex-hull-area cutoff 75%, 1% area/diameter distinctness), followers by
  **guaranteed interior witness** (arbitration 1) with the same rigid map,
  follower cap n.
- **Eight competitive workers** (Alg. 10 tournament): identical clone per
  master iteration, counter-derived permutations and sample streams, equal
  work, barrier, min total weighted Φ, stable ordinal tie, serial merge.
- The strip/ball jump, joint PGS, component-Y: **absent from this round.**

## The regime (frozen)

- Start: our constructor's dual-valid layout (182.976-class; their LBF is
  31 mm worse — the win is the loop). Constructor runs its frozen
  deterministic configuration, wall charged, **no internal wall cap**
  (arbitration 3).
- Explore: W ← W·(1−0.001), centre cut, far-side pieces translate by δ
  (t_y only), separate through infeasibility; **shrink advances only on a
  dual-valid publication at the new W** (stricter than Sparrow's proxy —
  deliberate). Fail: persist at W, least-infeasible pool (Normal-biased
  deterministic draw), disrupt, retry; never grow W, never restore-to-skip.
  A Φ=0 state whose publication is refused counts as a failed separation.
- Compress: restore last dual-valid parent (installed poses), uniform-Y cut,
  TimeBased step 0.0005→0.00001 vs phase-elapsed, discard failed children.
- Time: 80% of post-constructor remaining wall to explore, 20% to compress;
  clock read at **worker-sweep barriers** (arbitration 2), never inside a
  sweep/relocate/CD; publications after a checkpoint don't count for it.
  3/10/30 s are separate budget-response cells.
- **Publication poses install atomically as the next legal parent** (state +
  caches rebuilt; next D = published raw depth) — the exact-parent-drift
  defect (mod.rs:295) is fixed and FAST-vectored.

## The gate (pre-committed, the only judge)

From the bare mixed-61 request, one release binary (feature overlap-ics),
8 workers, seeds 0..=8, 10.000 s wall: **PASS iff ≥3/9 seeds publish a
strict non-constructor child with exact-valid raw-source depth ≤168.484 mm**,
every publication of every seed passing Exclusive r=2.500 (allowance 0) and
the untouched contract validator. Full non-interpolated 3/10/30 curve, all
nine seeds. Interleaved AB/BA wall-arm control cells, diagnostic only —
168.484 is absolute, the control can neither rescue nor kill. Regression
floor: S0 bit-for-bit, 1k/10k soundness zeros, literal old throughput
thresholds (new relocate metrics get NEW names — arbitration 4), four pinned
engine gates on default and feature-compiled-unarmed, default-build
isolation, jagua-rs/Xoshiro/rand:: absent from the tree. Forbidden rescues
and the failure license: Grok R2 §6.7 verbatim (one named line-level repair
with red/green vector, or — children exist in a tight band above the bar
with first bites publishing — nothing; the member closes; any other family
is separately funded). S1/triangle-20 become locked-T relocate regressions
(same pins, relocate-eval quotas).

## The four arbitrations

1. **Disruption followers use Sol's guaranteed interior witness** (centroid
   of the first positive-area ear-clipped cell, stored in PieceSource) —
   an area centroid can lie outside nonconvex material; Grok himself flagged
   the POI→centroid gap.
2. **Wall checks at worker-sweep barriers** (Sol) — bounds deadline overrun
   (the pivot round measured 2.223 s on a 2 s clause); still no clock inside
   any sweep.
3. **No internal wall cap in the constructor** (Sol) — a load-dependent
   start would break the determinism contract; the ~1.4 s is charged, not
   enforced.
4. **Throughput continuity** (Sol): the committed cold-Φ / row-rebuild /
   cell-gap thresholds stay literal; relocate economics get new counters
   (sampleEvaluations, relocatesPerSecond, containerWinners/focusedWinners/
   stayPutWinners, containerCommits) — no silent renaming of the 100K pin.

## Workflow (both signed)

Spec commit first (this document + the consultation docs + the fast.sh root
fix — no engine code), then core agent (relocate.rs, disrupt.rs, GLS,
descent strip), then schedule agent (homotopy.rs replace, Engine::run loop,
8-worker barrier, exact-parent install), then evidence/red-team agent
(driver, FAST union with the named tripwires — neutered-relocate,
Φ=0-does-not-bite, cut-close bits, exact-parent drift, first-bite canary,
K=8 two-process bite sequence, 8-worker merge determinism — then the frozen
9-seed wall). The most likely defect, pre-named by both: **the neutered
relocate** (a leftover strict filter or step cap silently rejecting every
sample that leaves the neighbourhood); second: exact-parent drift.
