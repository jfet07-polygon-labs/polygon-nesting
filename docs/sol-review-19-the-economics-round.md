# Sol review 19 — the economics round (2026-08-23, three rounds)

Round-2 design consultation for the CutCloseRelocate economics round, now a
THREE-model quorum (Sol, Grok, ox-alpha — the owner added ox-alpha and
mandated full convergence). Round 1 = independent design (proposed the
work-denominated strike quanta); round 2 = after reading Grok, REVERSED to
frozen literals ("one master iteration is an algorithmic event");
round 3 = signed the two-arm resolution. Verbatim.

# ROUND 1

Verdict: fund three changes only—work-denominated strikes, a persistent executor if profiling clears its gate, and a persisted calibrated-work pacer. Freeze the operator and 80/20 compression regime. The old 2/9 verdict remains closed; this is a newly authorized economics round.

## 1. Diagnosis of the 10-second loss

Ranked by expected millimetres at 10 seconds:

1. **Strike economics / escape latency — dominant.**

   Six seeds stop at the same bite-22 shelf. They reach 809–1,408 iterations but only 0–2 strikes; three strikes require at least 600 non-improving *master iterations*, so no disruption occurs before the explore deadline. This is explicit in the [bite-22 table](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-rerun/README.md:365>).

   The critical mismatch is that one of our iterations is not one Sparrow iteration. Sparrow measured 3.742M evaluations/s at 460 iterations/s: about 8,135 evaluations/iteration, so its 200-iteration strike costs about **1.63M candidate evaluations**. Our stuck cells spend roughly 11K–19K evaluations/master iteration, so “200” presently buys approximately 2.2–3.8M evaluations before a strike.

   This is the shortest direct path to moving probability mass across the ~10–12mm shelf.

2. **Executor overhead — material, not yet attributable specifically to thread creation.**

   Every master iteration allocates eight slots, clones eight `IcsState`/`Descent` pairs, then creates and joins eight OS threads at [mod.rs:646](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:646>). Sparrow constructs one persistent Rayon pool at [separator.rs:52](</var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:52>).

   From the raw 10-second rows, effective live throughput is about **2.73M sample evaluations/s median**. The single-worker throughput cell implies an ideal eight-worker ceiling of 5.72M/s, so observed parallel efficiency is about 48%. That gives a maximum theoretical speedup of 2.09×, but it does **not** prove a 2.09× thread-spawn tax: cloning, memory bandwidth, synchronization and unequal sweep cost are all inside that gap.

   A more defensible comparator is Sparrow’s 3.742M/s: CutCloseRelocate needs about **1.37×** throughput to reach it. The old m22 “substrate parity” claim does not transfer to this engine.

3. **The 80/20 split — real opportunity, but unsafe to change now.**

   My reconstruction from [wall.json](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-rerun/evidence/wall.json>):

   - On the six shelf-stuck seeds, the ~1.54-second compression phase buys only 0.0896–0.0935mm.
   - On seeds that escaped, compression buys 0.65, 2.91 and 3.83mm.

   So compression is almost wasted before escape and highly productive after escape. There is no request-independent trigger yet that distinguishes those cases without fitting mixed-61. Earlier compression is not supported: it already receives the shelf parent and does not escape it.

4. **Wall lottery — zero expected millimetres, high evidentiary cost.**

   The same binary/seed changed by 1.9mm and 4.1mm between processes [README:551](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-rerun/README.md:551>). A fixed plan makes the answer reproducible; it does not improve its expectation.

Two additional measurements are missing:

- The current FAST throughput pin is a single-worker canary, not an eight-worker executor test [README:424](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-rerun/README.md:424>).
- Constructor time is now 2.31–2.35s [README:604](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-rerun/README.md:604>). The historical 1.4s was a quality-saturation observation, not necessarily the same call boundary, so do not book 0.9s savings without a paired profile.

## 2. Round-2 design

### P0 — profile the executor first

On frozen easy and bite-22 hard states, time separately:

- slot/state/descent preparation;
- thread creation and join;
- worker sweeps;
- merge plus GLS;
- exact attempts and repair.

Run workers 1/2/4/8 at identical fixed work. Instrument only the benchmark path.

**Build the persistent executor iff** preparation plus dispatch/join is at least 10% of hard-state wall. Promote it only if:

- fixed-work trajectory is bit-identical to the ephemeral executor at every batch;
- p50 speedup is at least 1.15× on mixed-61’s shelf;
- geometric-mean speedup is at least 1.10× over mixed-61, shapes-17 and triangle-20;
- no fixture regresses more than 5% and peak RSS grows no more than 10%.

Implementation: one local Rayon pool, persistent preallocated worker slots, `clone_from` to reuse allocations, ordinal merge unchanged. Rayon is already a dependency; no new substrate is needed.

### P1 — replace iteration-denominated strikes with work-denominated strikes

Freeze these source-derived quanta before any quality run:

- Explore no-improvement quantum: **1.63M sample evaluations**.
- Compress quantum: **0.815M sample evaluations**.
- Strike counts stay **3/5**.
- The 2% substantial/marginal/none semantics stay unchanged.

Derivation: Sparrow’s same-machine 3.742M evaluations/s ÷ 460 iterations/s × 200 or 100 [Sparrow addendum](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparrow-mixed61/README.md:72>). This is cost normalization, not tuning against our depths.

For each master batch:

- `Substantial`: reset accumulated no-improvement work.
- `Marginal`: update the minimum snapshot but add no work.
- `None`: add that batch’s `sample_evaluations` from **all eight workers**.
- Strike when the quantum is reached; overshoot by at most one batch.

The source’s iteration loop and 2% rule are at [separator.rs:89](</var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:89>). Our work vector already explicitly charges all workers and names candidate evaluations at [diagnostics.rs:35](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/diagnostics.rs:35>).

At current shelf economics this should move a strike from roughly 200 batches to about 85–145, making a disruption reachable during the existing explore slice. Seeds 7/8 remaining stuck after five disruptions at 30 seconds prove that this will not make 9/9 automatic [README:347](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-rerun/README.md:347>).

### P2 — calibrated total-work plan

Do this only after the executor and strike policy are frozen.

The existing `Budget::FixedWork` is not sufficient: it limits successful bites, attempts and per-separation iterations—not total work [mod.rs:1395](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1395>). A hard shelf can therefore consume radically different work under the same nominal setting.

Add a versioned currency:

```text
U =
    sample_evaluations
  + B × master_batches
  + E × actual_publication_attempt_calls
  + R × repair_rows
  + D × disruption_moves
```

Derive `B/E/R/D` from timing-only microbenchmarks on all three fixtures, convert them to equivalent sample-evaluation units, and round conservatively. Reject this currency if its wall prediction error exceeds 10% on any transfer fixture.

The calibration file must pin:

- request hash and contract;
- currency version;
- binary/feature semantic key;
- workers=8;
- executor implementation;
- per-phase safe units/s.

Read and write must remain separate, as already established by [robust-plan](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/robust-plan/README.md:173>). No live probe may influence a gated trajectory.

Allocate calibrated units 80/20 between explore and compress. Compression decay advances by consumed compress-work fraction, not by a live clock. The trajectory stops only between master batches.

Call this a **10-second calibrated work plan**, not “deterministic 10 seconds”: quality is deterministic; wall remains a distribution. Without a hard-wall governor, arbitrary external load can still overrun.

### P3 — compression

Freeze 80/20 and all compression behavior this round. Add shadow-only counters:

- exact-valid mm/unit per phase;
- work since last explore publication;
- unresolved-bite work at the phase boundary;
- time/work to first strike, disruption and shelf publication.

Do not introduce a mixed-61-specific readiness threshold. If the new deterministic evidence repeats “compression ≈0.09mm when unresolved, multi-mm after escape” on transfer fixtures, conditional allocation becomes a separately funded round.

## 3. Pre-committed gates

### Mechanical gates

- Persistent and ephemeral executors: identical winner ordinal, guided bits, state fingerprint, full per-batch work delta and final document for at least 1,024 batches, including a strike, pool restore and disruption.
- Work-strike vectors: exact reset/pause/debit behavior, thresholds above, and at most one-batch overshoot.
- Split `exactAttempts` into actual call count and bites-with-attempt; their sums must reconcile. The present name hides seed 2’s 1,313 calls behind 174 bite rows [README:457](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-rerun/README.md:457>).
- Calibrated plan: two-process bit identity, calibration hit/miss/version tests, no clock read affecting a hit trajectory.
- Existing S0, S1, first-bite, 10k soundness and four protected gates remain mandatory.

### End-to-end 10-second clause

Bare request, seeds 0–8, workers=8, frozen calibrated plan:

**PASS time axis iff all hold:**

- at least **5/9** seeds publish exact-valid depth ≤168.484mm;
- median therefore also ≤168.484mm;
- every publication passes Exclusive r=2.500 and the untouched contract validator;
- each seed is bit-identical across two processes;
- end-to-end quiet-box p95 is ≤10.000s over 5 repetitions × 9 seeds.

Run the frozen iteration-strike policy as a paired equal-work control. Treatment must gain at least two qualifying seeds or at least 1mm paired median; otherwise the absolute result is not attributable to the new patience policy.

A controlled-load battery reports overruns and must preserve documents, but cannot honestly promise a wall bound without the excluded governor.

### Thirty-second quality clause

Current median is 164.00461mm. Require:

- median exact-valid depth **≤163.00461mm**;
- paired equal-work median improvement ≥1.000mm over the frozen control;
- no shapes-17 or triangle-20 median regression greater than 1mm at equal work;
- zero invalid publications.

Report 3/10/30 plus a nine-seed 60-second diagnostic and time-to-depth survival curves. Sixty seconds is not a gate.

**150.165 remains the horizon, not a clause.** A performance/economics round should first make ≤168 deterministic and move the 30-second median materially below 164.

## 4. Explicitly frozen

No changes to:

- relocate/sample operator: 25+50 samples, 16 launch angles, three finalists, coordinate descent, accept-equal;
- disruption, pool selection or follower semantics;
- GLS multipliers/update schedule;
- 0.1% explore bites;
- compression range, cut semantics or 80/20 share;
- worker count;
- constructor algorithm or start state;
- publication band, repair cap, allowance, Exclusive kernel or contract validator;
- old-stack lanes or new operators.

The only quality-semantic change is replacing “200/100 master batches” with the pre-derived equivalent candidate-work quanta. Executor and planning changes must be trajectory-preserving at fixed work.

## 5. Workflow and pre-named defect

Four roles, three waves:

1. **Spec/profile agent:** commits formulas, thresholds and timing census before implementation.
2. **Parallel module work:**
   - executor agent owns a new persistent executor module and its equivalence tests;
   - meter agent owns strike-work and calibrated-budget primitives, not `mod.rs`.
3. **One integration agent:** sole owner of `mod.rs`, Pacer wiring and example schema.
4. **Evidence agent:** may run drivers and commit evidence, but may not edit engine code after the gate text is frozen.

FAST additions:

- ephemeral/persistent K=1,024 equivalence on a hard shelf;
- forced scheduling-order perturbation with identical ordinal merge;
- two consecutive persistent batches with exact per-slot delta accounting;
- strike meter with variable batch costs;
- calibrated-plan hit/miss and clock-poison tests;
- eight-worker hard-shelf throughput, not just the single-worker canary;
- actual-attempt counter reconciliation;
- existing S0/first-bite/default-build isolation.

The most likely defect is **persistent-slot leakage**: a reused non-winner retains poses, GLS weights, descent stream state or its previous `WorkVector`. The particularly dangerous form is deterministic double-debit—`saturating_add` currently consumes fresh local vectors at [mod.rs:680](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:680>); a persistent vector not zeroed would charge prior batches again and make work-strike/plancal “stable” but false.

Pre-commit the red/green vector: batch two’s aggregate must equal the sum of the eight **batch-two deltas**, not cumulative slot totals, while its winner/state/work fingerprint exactly matches the ephemeral executor.


# ROUND 2 — exchange

Bottom line: I converge on literal `200/3/100/5/0.98`, a measured persistent executor, the narrow compress exception, and a shelf-calibrated deterministic work plan. I withdraw my claim that 1.63M/0.815M evaluations preserve the same strike semantics: they are a defensible but distinct impatient-strike policy.

## 1. Strike policy

**AGREE with Grok; DISAGREE with my previous characterization.**

One master iteration is an algorithmic event, not merely a cost container:

- one complete eight-worker relocate tournament;
- one winner installation;
- one all-row GLS update.

Sparrow increments `n_iter_no_improvement` around exactly that event at [separator.rs:89](</var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:89>). Replacing 200 such GLS/search rounds with however many rounds consume 1.63M evaluations changes:

- the number of GLS updates before rollback;
- the number of colliding-set sweeps;
- dependence of patience on active-piece cardinality and geometry.

Therefore 1.63M/0.815M is a pre-derived, non-cherry-picked **retune**, but not “the same frozen semantics in equal-work units.”

For this round:

- freeze `200/3/100/5/0.98`;
- retain sample evaluations beside iterations as diagnostics;
- do not ship an eval-denominated strike arm;
- record `impatient-strike` as a separately funded policy if ever requested.

The strike-specific paired control is consequently unnecessary. A paired end-to-end control remains mandatory for the executor/schedule package.

## 2. Persistent executor

**KNOB on backend; pick Grok’s eight parked OS threads for the signed spec.**

Rayon itself would not make completion order observable if workers wrote fixed ordinal slots and the coordinator scanned those slots serially; it is also already a dependency. Nevertheless, backend choice is not the mechanism, and parked threads give us the smallest disputed surface.

Signed executor contract:

- eight threads created once per `run_cutclose`;
- one permanent slot and ordinal per thread;
- generation barrier around each tournament;
- workers write only their ordinal slot;
- coordinator waits for all eight, then scans `0..7`;
- stable `(guided_loss, ordinal)` winner;
- no channel-arrival or completion ordering;
- all eight current-batch work deltas charged exactly once.

## 3. Executor build rule

**AGREE, merged as a two-stage rule.**

First, a 200-master-iteration shelf census measures:

- allocation/copy preparation;
- spawn/join;
- worker sweep;
- merge/GLS;
- total barrier wall.

Build the persistent candidate iff either:

- spawn+join ≥8% of barrier wall; or
- clone/allocation ≥15%.

Promote it only if all hold:

- per-batch and final trajectory bit-identical to `thread::scope`;
- shelf p50 speedup ≥1.15×;
- geomean speedup ≥1.10× over mixed-61, shapes-17 and triangle-20;
- no fixture slower by >5%;
- peak RSS increase ≤10%.

Thus a measured 8% opportunity can license construction, but a disappointing implementation does not automatically ship.

If the executor is skipped, the final quality gate remains 5/9. I reject a conditional 4/9 fallback: that weakens the owner’s time objective in response to a mechanism not materializing.

## 4. Compression exception

**AGREE with Grok, with a fixed-work clarification.**

Keep 80/20 and every bite parameter frozen. At the explore boundary only:

> If the outstanding unpublished explore bite has `masterIterations ≥ 200` or `strikes ≥ 1` or `disruptions ≥ 1`, continue that same bite through the nominal compression allocation. If it publishes, compression receives whatever budget remains. Otherwise the run ends without compression.

In calibrated-work mode, “remaining time” means the remaining total master-iteration allocation; no clock enters the trajectory.

This is supported by the three regimes:

- seeds 2/3 reach the next failed bite with only two iterations and therefore retain compression;
- seeds 0/4/7/8 are already at two strikes and receive the escape opportunity;
- seed 5 has substantial accumulated work despite zero strikes and is caught by `iterations ≥ 200`.

Mandatory tripwires:

- unpublished at 2 iterations → compression still starts;
- unpublished at 200 iterations → explore continues;
- publication during the transferred allocation → compression receives the exact remainder.

## 5. Final gate union

### Calibrated work plan

**AGREE with Grok’s shelf probe; amend the current fixed-work implementation.**

The current `Budget::FixedWork` limits bites and per-separation iterations, not total trajectory work [mod.rs:1631](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1631>). Round 2 adds total explore/compress master-iteration allocations.

Calibration:

- fixed 400-iteration probe at the 179 shelf, never on cheap bites 1–21;
- persisted offline, read/write paths separate;
- key includes request, seed, binary/feature identity, workers, executor kind and calibration version;
- headroom 0.97;
- constructor fully charged and uncapped;
- quality battery reads a frozen file; no live measurement changes its cap;
- `sampleEvaluations` and other work counters reported beside master iterations.

Master iteration is the v1 plan currency because it preserves the frozen member. Its variable wall cost is policed by the p95 timing clause; if that clause fails, the calibration fails rather than silently inventing another exchange rate.

### Ten-second clause

Bare mixed-61, seeds 0–8, eight workers, strict non-constructor publications:

**PASS iff all hold:**

- ≥5/9 seeds publish exact-valid raw depth ≤168.484mm;
- every publication passes Exclusive r=2.500 and the untouched contract validator;
- each seed’s calibrated-work document is bit-identical across two processes;
- quiet-box end-to-end p95 ≤10.000s over 5 repetitions × 9 seeds;
- zero invalid publications.

No 4/9 fallback.

Run a frozen control with the old ephemeral executor and no compression exception, using its own persisted p95-10s plan. Pair by seed. For attribution, treatment must improve the qualifying count by at least two or improve paired median depth by at least 1mm.

This control is equal **calibrated time**, not equal work: after rejecting the impatient-strike treatment, equal-work comparison would intentionally erase the executor’s benefit.

The ordinary wall 3/10/30 curve remains reported and non-interpolated. It cannot rescue a failed deterministic quality clause, but p95 wall remains a gate because otherwise this round would not have a time axis.

### Thirty-second clause

Using independently calibrated p95-30s plans:

- median raw depth ≤163.00mm;
- ≥7/9 seeds ≤168.484mm;
- paired median improvement ≥1mm against the closed control’s own p95-30s plan;
- shapes-17 and triangle-20 median regression ≤1mm;
- zero invalid publications.

Report all nine 60-second cells, but do not gate them.

150.165 remains a horizon only.

## 6. Fresh facts folded in

**AGREE, with two cautions.**

- Constructor cost is 2.31–2.35s and remains charged. The historical 1.4s described quality saturation, not necessarily the same timing boundary, so no 0.9s saving is budgeted without a paired profile.
- Live eight-worker throughput is about 2.73M evaluations/s median versus a 5.72M/s ideal extrapolation: approximately 48% scaling efficiency. Only measurement may attribute that gap to spawning or cloning.
- Reaching Sparrow’s 3.742M/s needs about 1.37× throughput.
- Seed 0’s “16 iterations short” compares separate wall processes, so it is a strong opportunity signal, not a deterministic proof.
- Seed 5’s 288-iteration shortage makes the compression-transfer exception directly relevant.
- Seeds 7/8 are a different basin. No 10-second clause requires them; 5/9 can pass through seeds 0/2/3/5/6.
- The 30-second ≥7/9 clause merely prevents losing the already demonstrated population; it does not demand 7/8 themselves escape.

## 7. Workflow and FAST union

**AGREE on four waves.**

1. **Spec/evidence agent**

   Commits §0, shelf-profiler and calibration schema before engine changes.

2. **Parallel implementation**

   - Executor agent: parked threads, persistent slots, barrier.
   - Schedule agent: total-iteration pacer, persisted calibration primitives, compression exception, exact-attempt counter split.

   Neither independently edits the central loop.

3. **Single integrator**

   Sole owner of `run_cutclose`, Pacer wiring and example/document schema.

4. **Evidence/red-team agent**

   Runs FAST, seals calibration, then runs the frozen 3/10/30/60 batteries. No engine edits after calibration is sealed.

FAST is the existing floor plus:

- 200-iteration shelf tax decomposition;
- ephemeral versus persistent per-master identity;
- forced completion-order perturbation;
- batch-two work-delta vector;
- calibration hit/miss/version/read-write isolation;
- assertion that the probe reached shelf density;
- two-process calibrated-work identity;
- compression 2-iteration/200-iteration tripwires;
- strike predicate regression;
- actual publication calls versus bites-with-attempts reconciliation.

Pre-named defects, in order:

1. **Persistent-slot leakage/double debit.** Batch two accidentally carries batch-one poses, weights, descent stream or work totals.
2. **Completion-order leak.** Coordinator consumes notification order instead of ordinal slots.
3. **Cheap-probe calibration.** The plan is priced on bites 1–21 and starves at bite 22.
4. **Compression steal.** A two-iteration cascade child is misclassified as “already striking.”

The decisive red/green vector remains: batch two’s charged work equals the sum of the eight batch-two deltas—not cumulative slot totals—while winner, state and work fingerprint exactly match the ephemeral executor.

## Single converged spec

I would sign this:

> Preserve the literal CutCloseRelocate member, including `200/3/100/5/0.98`. Measure a 200-iteration 179-shelf cell; build eight persistent ordinal worker threads only if spawn/join ≥8% or clone/allocation ≥15%, and promote only with bit-identical trajectories, ≥1.15× shelf speedup, ≥1.10× three-fixture geomean, ≤5% regression and ≤10% RSS growth. Add a persisted, shelf-probed, master-iteration work plan with no clock in the trajectory. Preserve 80/20 except that an unpublished explore bite already at ≥200 iterations, ≥1 strike or ≥1 disruption may consume the remaining allocation before compression; a two-iteration cascade child must still compress. Gate at ≥5/9 exact-valid ≤168.484mm and quiet-box p95≤10s; at 30s require median≤163.00, ≥7/9≤168.484, paired ≥1mm, and transfer within 1mm. Report 60s; keep 150.165 as horizon. No new operator, bite, GLS, rotation, publication or worker-count change. One concrete line-level defect licenses one identical rerun; otherwise the funding closes.

The only thing I refuse to preserve from my first reply is calling the 1.63M-evaluation strike quantum “the same frozen semantics.” It is not.


# ROUND 3 — the closing vote

YES. This resolves my objection: iteration semantics remain intact in the control, while 1,630,000/815,000 are explicitly a distinct, pre-frozen impatient-strike treatment. The paired attribution clause prevents an absolute 5/9 lottery from promoting it, and failed attribution permits neither retuning nor a second guess.

— Sol, signed
