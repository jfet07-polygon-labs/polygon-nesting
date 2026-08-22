# The overlap-ICS converged spec (2026-08-22)

The three-way consultation (Sol review 14, Grok review 9 — two rounds each,
with full cross-exchange) converged on one implementation spec for the
overlap-tolerant continuous engine. **The spec of record is Sol review 14
Round 2 §4 ("Single converged implementation spec") plus §3 (the two-tier test
discipline), with the arbitrations below.** Grok review 9 Round 2 §4 is the
same engine with three residual deltas; the main session arbitrated them.

## What converged without arbitration

- Family: locked-strip-then-shrink ICS (Egeblad/Imamichi lineage), designed
  from the literature, never from Sparrow's code; jagua-rs stays unused and
  the build script fails if it appears in the `overlap-ics` feature tree.
- Module `search/overlap_ics/`, feature `overlap-ics = ["round-envelope-kernel",
  "fast-contract-validator"]`, example-only driver, default build untouched.
- Φ (the continuous overlap measure): allocation-free signed convex gap —
  streamed SAT MTV while overlapping, closest material feature while
  separated, triangle-cell maximum for nonconvex pairs; the integer kernel
  NEVER participates in Φ (publication judge only). Both proposed primitives
  are retained: Grok's SAT/closest-feature as the hot path, Sol's nine-point
  triangle Minkowski as the independent test oracle.
- Solver: deterministic damped PGS (translation + continuous θ from the first
  sweep, no 2.5° catalogue), guided integer contact weights after one stalled
  sweep, ONE topology jump (16 deterministic low-discrepancy relocations of
  the highest-pressure piece) after two guided stalls. No swaps, mirrors,
  restarts in Round 1. Exact geometry never truncates a move.
- Start: full fast constructor once (~1.4 s) → exact anytime floor; the ICS
  state is an affinely compressed COPY of its poses (the constructor
  fingerprint is never a child). Strip homotopy: T₀ = D* − 0.10(D*−L), eight
  equal-work epochs, on failure T ← (T+D*)/2 keeping the infeasible state.
- Publication: continuous rings → single canonicalization via `GridSet::of`
  (no pose pre-snap) → request-scoped kernel **Exclusive at r=2.500,
  allowance 0** → untouched material contract validator → protected
  `best_exact`. Repair: frozen-θ, same-strip, ≤4n row corrections, ≤16 µm
  cumulative per piece, target immutable. **No millimetre-scale legalization
  ever** — a source-faithful Φ at zero may disagree with exact geometry only
  at grid scale.
- Determinism: fixed-work trajectories, bit-identical poses/checkpoints/
  publications per (request, seed, binary, x86, toolchain, libm, features,
  workers, quota); serial Round 1; Round 2 = eight deterministic independent
  trajectories with ordinal merge; wall mode reads the clock only between
  batches and returns `best_exact`.
- Two-tier test discipline (the owner's requirement): FAST tier every
  iteration — default-build compile check (`--no-default-features --lib`),
  dependency-hygiene check, one release combo (`overlap-ics`), the module's
  unit vectors, the 1,000-state contact corpus, and the two-process
  fixed-work smoke with the pinned S0 canary (Sparrow fixture: Φ bits == 0,
  raw depth 150.16451, dual-valid, zero repair). HEAVY tier at round
  boundaries only — 4 gates on both builds, 4 suites, the 10,000-state
  corpus, the full inflation-probe battery, two-binary determinism, nine
  distinct seeds at 3/10/30 s, transfer corpora, contemporaneous interleaved
  controls, no interpolation anywhere. The never-defer list is Sol R2 §3.
- Round-2 kill (BOTH agree, Sol withdrew 175.388): fixed-work treatment
  calibrated to p95 wall ≤10 s must reach **median ≤168.484, ≥6/9 seeds
  ≤168.484, contemporaneous paired win vs the wall arm**, all publications
  dual-valid, transfer within 1 mm — or the 10-second program for this
  family dies. Round 3 aspiration: median ≤160, one seed ≤155; missing
  150.165 alone is not a kill (owner's go/no-go).

## The three arbitrated deltas

1. **Is the shocked-constructor-into-168.484 cell (C168) fatal pre-loop?**
   Grok: yes (his "wasted-month detector"). Sol: diagnostic, with C175 (the
   10%-residual shock, strict child in 2 solver seconds on 3 fixed seeds)
   fatal instead. **Arbitration: Sol's structure, Grok's deadline.** Gate 0
   fatal cells = S0, S1, C175, triangle-20 canary, numeric soundness,
   throughput (≥1M cell gap evals/s, ≥100K piece proposals projected into
   8 s, cold Φ ≤200 µs, row rebuild ≤20 µs). C168, S2 and random-T run in
   the same battery as diagnostics — but Round 1's 30-second clause
   (≤168.484 on ≥3/9, single-thread, kill BEFORE any parallel work) is the
   same detector with one round's patience instead of two seconds', and the
   agreed Round-2 kill bounds the total exposure at two rounds. Grok's fear
   (a month of choreography) is structurally impossible under this ladder.
2. **Random throw as a family kill.** Grok proposed it in Round 1 and
   retargeted it to the shocked constructor in Round 2; Sol refuses fatality
   for the uniform throw (it confounds initialization with separation).
   **Arbitration: diagnostic only**, per both Round-2 texts.
3. **Naming.** Feature `overlap-ics`, module `search/overlap_ics/` (Sol R2,
   most specific); Grok's `continuous-overlap` naming is superseded.

## Round structure of record

- **Gate 0** (before schedule/parallel work): fast corpus + inflation cells;
  fatal set above. Half-a-day-to-days of machine time, most of it reusable
  as the permanent FAST tier.
- **Round 1** (single-thread, nine distinct seeds, full 3/10/30 curve):
  ≥6/9 strict dual-valid non-constructor children by 10 s; ≥3/9 at ≤168.484
  by 30 s; median repair giveback ≤0.050 mm; every publication dual-valid.
  Kill before parallelization on either mechanism clause.
- **Round 2** (eight workers): the agreed 168.484@10s kill above.
- **Round 3**: ≤160 median / one seed ≤155 / transfer — then the owner
  decides on the 150 program.

Chinese wall: the Sparrow pose fixture is the S0 correctness pin and a
post-gate holdout for reachability diagnosis; never a seed, never a
parameter source; constants freeze before any diagnosis run.
