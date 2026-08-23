# Sol review 17 — reading Sparrow (2026-08-23, two rounds)

Same brief, same source, same revision. Round 2 opens with a position change:
"Grok is right on the main member" — the coordinated-projection design is
withdrawn in favor of the paper-derived relocate member. Verbatim.

# ROUND 1

Verdict: the round is justified, but the rationale changes after reading Sparrow `14f4868f`. The evidence strongly supports the small-bite/cut-close regime. It does not support the claim that coordinated PGS is Sparrow’s mechanism: that remains our independent solver hypothesis and must live or die by the funded end-to-end gate.

## 1. What Sparrow actually does

### Their code shows

- `--global-time 10` becomes 8 seconds exploration plus 2 seconds compression. Critically, import and the LBF constructor run before either timeout is armed. Sparrow’s published “10 s” is therefore not a bare-request end-to-end wall budget. [main.rs](/var/lib/t3/tmp/sparrow-bench/src/main.rs:38), [optimizer/mod.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/mod.rs:33)

- Exploration repeatedly takes a legal layout, reduces strip width by exactly `0.001 = 0.1%`, allows the resulting state to be infeasible, and runs the separator until loss becomes zero. Only then is the child retained and the next bite taken. [config.rs](/var/lib/t3/tmp/sparrow-bench/src/config.rs:55), [explore.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/explore.rs:32)

- Shrink is not affine compression. It chooses a cut across the strip and translates every piece whose centroid lies beyond the cut by the whole width delta. Exploration uses the centre cut; compression uses a random cut. This is its routine coordinated move. [separator.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:223), [compress.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/compress.rs:57), [jagua problem.rs](/var/lib/t3/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/jagua-rs-0.7.2/src/probs/spp/entities/problem.rs:32)

- Routine separation is single-piece search:

  - collect currently colliding pieces;
  - shuffle their order;
  - for each, evaluate 25 focused plus 50 container-wide poses;
  - refine three finalists with coordinate descent;
  - commit the best pose.  
    [worker.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/worker.rs:34), [search.rs](/var/lib/t3/tmp/sparrow-bench/src/sample/search.rs:20)

- Acceptance is weighted-loss non-increase for the moving piece. The current pose is a candidate, so the result cannot be worse under the current weights. Coordinate descent explicitly accepts equal evaluations, allowing neutral drift. [worker.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/worker.rs:70), [coord_descent.rs](/var/lib/t3/tmp/sparrow-bench/src/sample/coord_descent.rs:114)

- Eight workers are not eight long-lived basins and do not jointly move neighbours. Every separator iteration clones the same master state into eight workers, lets each execute a different shuffled single-piece sweep, and retains only the worker with minimum total weighted loss. [separator.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:145)

- GLS updates every pair/container row after every iteration:

  - active rows multiply by `1.2 … 2.0`, proportional to their loss relative to the maximum;
  - inactive rows decay by `0.95`, floored at one;
  - rollback restores the minimum-raw-loss pose state while retaining the evolved weights.  
    [tracker.rs](/var/lib/t3/tmp/sparrow-bench/src/quantify/tracker.rs:85), [tracker.rs](/var/lib/t3/tmp/sparrow-bench/src/quantify/tracker.rs:112), [separator.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:102)

- Rotation is continuous only after sampling. Container/focused proposals initially use 16 evenly spaced orientations, 22.5° apart; coordinate descent then wiggles continuously at 5°→1° and 0.5°→0.05° scales. [uniform_sampler.rs](/var/lib/t3/tmp/sparrow-bench/src/sample/uniform_sampler.rs:13), [search.rs](/var/lib/t3/tmp/sparrow-bench/src/sample/search.rs:79)

- At separator failure, exploration keeps a pool of least-infeasible states, selects one with a bias toward low loss, swaps two large pieces, and moves pieces practically contained by the swapped shapes with them. This is the rare topology disruption, not the normal move. [explore.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/explore.rs:51), [explore.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/explore.rs:89)

- Compression always restarts an attempt from the best exact internal legal state, uses a bite that decays from 0.05% toward 0.001%, and discards a failed attempt. [config.rs](/var/lib/t3/tmp/sparrow-bench/src/config.rs:75), [compress.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/compress.rs:19)

- Its search authority is a simplified/concavity-closed collision representation with pole-based overlap loss, not our source-ring signed gap and not an exact publication validator. [config.rs](/var/lib/t3/tmp/sparrow-bench/src/config.rs:91), [overlap_proxy.rs](/var/lib/t3/tmp/sparrow-bench/src/quantify/overlap_proxy.rs:5), [specialized_jaguars_pipeline.rs](/var/lib/t3/tmp/sparrow-bench/src/eval/specialized_jaguars_pipeline.rs:24)

The retained x86 log is unusually decisive: 351 exploration shrinks, 350 legal children, one terminal failure, and one disruption. Exploration reaches 150.796 mm; eleven compression successes then reach 150.165 mm. [log-10s-x86.txt](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparrow-mixed61/log-10s-x86.txt:2528)

### What Sparrow does not have

It has no contact-gradient PGS, no two-endpoint pair projection, no chain linear solver, no force/torque field, no routine swap, no acceptance of worse weighted poses, and no exact validator at every legal endpoint. It also does not keep one uninterrupted state forever: it rolls back within separation, restarts compression attempts from the legal parent, and can disrupt failed exploration states.

### I infer

- The strongest transferable fact is the injection geometry: a 0.1% cut-close bite creates about 0.15–0.21 mm of localized infeasibility, not C175’s distributed 5–7 mm shock. This is directly inside the scale where our corrected S1 solver succeeds.

- Sparrow’s result does not prove that our coordinated projection will work. In fact, its success is evidence for small bites plus broad single-piece sampling and aggressive GLS, not for joint PGS.

- The lone exploration disruption cannot explain this 150.165 result: it occurs after the best 150.796 legal exploration state is already fixed, and compression immediately restores that legal state.

- The source’s smooth pole loss and global relocation candidates may still be essential. Our worst-cell signed-gap field is more nonsmooth. If the funded member fails, we must not conclude that every overlap-tolerant engine fails.

These observations must be committed with source revision and file anchors before implementation. No source text or implementation should be copied.

## 2. The funded member

Call it `JointSmallBiteSeparator`. Keep one frozen feature combination.

### What survives

Keep:

- continuous `Pose`, source decomposition, SoA transformed geometry, request-derived contract split, and protected exact incumbent from [state.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/state.rs:125);
- signed contact, witnesses, and deterministic normals from [contact.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/contact.rs:29);
- broad phase, incremental row cache, and fixed-order cold-rescore audit from [energy.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:1);
- the current publication band and capped repair;
- Exclusive `r=2.500`, then untouched contract validation, with no Union or allowance, from [publish.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:245);
- exact-only quality history and work-vector diagnostics from [diagnostics.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/diagnostics.rs:1).

### Routine move: true joint projection

For an active pair row with violation \(g>0\), normal \(n\), witnesses \(p_i,p_j\), and centroids \(c_i,c_j\):

\[
J_i=(n_x,n_y,(p_i-c_i)\times n)
\]

\[
J_j=(-n_x,-n_y,(p_j-c_j)\times(-n))
\]

Use the SE(2) metric

\[
M_i=\operatorname{diag}(1,1,R_i^2)
\]

where \(R_i\) is the existing request-derived piece radius. Then:

\[
\lambda =
\frac{g}
{J_iM_i^{-1}J_i^\top+J_jM_j^{-1}J_j^\top}
\]

\[
\Delta q_i=\lambda M_i^{-1}J_i^\top,\qquad
\Delta q_j=\lambda M_j^{-1}J_j^\top
\]

Boundary rows use the same formula with one endpoint. Rotation is applied about the transformed centroid through the already-corrected `compose_proposal`.

Semantics:

- apply both endpoints atomically;
- transform both pieces first, then rebuild the union of their incident rows once;
- immediately remeasure before the next row;
- no per-piece incident-energy veto;
- relaxation is exactly 1.0 in this round;
- no θ lattice and no mirror changes.

This requires a dedicated `rebuild_two_piece_rows`; calling the current one-piece rebuild twice risks precisely the stale/shared-row ownership defect already found in the relaxed tracker.

### Sweep acceptance and GLS

A full PGS sweep is allowed to get temporarily worse. The legal parent remains protected.

- Active rows are ordered by descending weighted violation, stable row-id tie.
- Joint projections commit unconditionally if finite.
- Track the best continuous snapshot lexicographically by `(max_g, raw_phi, fingerprint)`.
- A sweep with no new best updates all row weights, not one row.

Use our own normalized multiplicative GLS, not Sparrow’s formula verbatim:

\[
w_r\leftarrow w_r(1+g_r/g_{\max})\quad\text{if active}
\]

\[
w_r\leftarrow 1+\tfrac78(w_r-1)\quad\text{if inactive}
\]

Cap at \(2^{20}\). Weights affect row priority and the component objective, not whether an individual projection is permitted. Retain weights across rollback inside one bite; reset them when starting a different bite from the exact parent.

### Component Y move

After two complete sweeps with no new best:

1. Build connected components from active pair rows; attach top/bottom boundary rows to their pieces.
2. Freeze their contacts.
3. Compute every row’s translation-only joint correction.
4. Accumulate the \(y\) corrections for every piece and divide by active degree: a simultaneous Jacobi chain displacement.
5. Evaluate damping values `{1, 1/2, 1/4}` on the frozen linear model, without a stay-put candidate.
6. Install the best predicted component vector atomically, capped at one current bite \(|D-T|\) per piece.
7. Run another full joint PGS sweep.

The component move is committed for that strike even if nonlinear Φ rises; the exact parent protects the run. Three component strikes without a new best fail the bite.

### Remove from the new trajectory

Do not call the existing strip/ball relocation jump. Its installed C175 moves raised guided Φ by 21×–207×, and Sparrow’s corresponding disruption did not contribute to the retained 10-second solution. Keep it only for historical Gate-0 replay if needed.

Do not add Sparrow-style 75-pose sampling in this round. That would confound the funded coordinated-member test and come uncomfortably close to recreating its optimizer structure.

### Eight-core execution

Use eight deterministic repair replicas, all starting from the same exact parent and the same bite. They differ only in stable cyclic row order derived from `(request_seed, epoch, worker_ordinal)`.

At a fixed barrier:

- finish equal work on all workers;
- consider every worker inside the publication band;
- exact-check in worker ordinal order;
- choose the deepest valid child by `(raw_depth, fingerprint, worker_ordinal)`;
- if none publishes, retain the best proxy worker for the next batch.

Completion order must never be observable.

## 3. Shrink regime

Replace the old 10% affine specification in [homotopy.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/homotopy.rs:20).

Let the constructor’s dual-valid raw depth and repaired poses be \(D\).

1. Initial bite: \(\beta=0.001\).
2. Target: \(T=D(1-\beta)\).
3. First attempt at a parent uses the centre cut.
4. Translate every piece whose transformed source centroid is above that cut by \(T-D\) along the long axis; θ and mirror are unchanged.
5. Run the joint separator through infeasible states.
6. A bite succeeds only when publication returns an Exclusive-valid and contract-valid layout with raw depth \(\le T\).
7. On success:

   - install `Publication.poses` into the continuous state;
   - rebuild geometry and all rows;
   - set \(D\) to `Publication.raw_source_depth_mm`;
   - keep the current \(\beta\);
   - take the next bite.

8. After three failed component strikes:

   - restore the exact parent poses;
   - halve \(\beta\);
   - advance to a seed-derived low-discrepancy cut;
   - retry.

9. Minimum \(\beta=10^{-5}\). If that bite fails, stop shrinking; no late jump or alternate optimizer.

Starting around 180–183 mm, reaching 168.484 takes approximately 66–83 successful 0.1% bites. Sparrow demonstrated 350 such children in eight search seconds.

“Re-legalized” means exact publication, not `Phi≈0`. Repair must remain within the existing target; [publish.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:418) already rejects giveback beyond it.

### Wall allocation

Our clock starts on entry to nesting with a decoded bare request, before source decomposition and construction.

- Existing deterministic constructor: once, approximately 1.4–1.5 s.
- Build continuous geometry and the initial exact parent.
- Spend the remainder on resumable joint-separator batches.
- Read wall time only between batches.
- Publications finishing after a nominal checkpoint do not count for that checkpoint.

There is no copied 80/20 phase split: Sparrow’s 8/2 excludes its constructor and uses a different solver.

The 3/10/30 curve is one 30-second trajectory sampled as a step function:

- 3 s: constructor floor or any child completed before 3 s;
- 10 s: last dual-valid incumbent completed before 10 s;
- 30 s: same trajectory continued, without restart or new operators.

## 4. Pre-committed gate text

This is the text I would commit before implementation:

> **ROUND VALIDITY.** One release binary, feature `overlap-ics`, eight workers, seeds 0 through 8, one run per seed. S0 remains bit-identical at raw depth 150.16451 with Φ bits zero, Exclusive `two_r=5000`, untouched contract-valid, zero repair rows and zero giveback. Both numeric-soundness populations retain zero false-feasible, zero containment false-feasible and zero incremental mismatch. The committed cold-Φ, row-rebuild and cell-gap throughput thresholds remain green. The legacy proposal microbenchmark remains recorded under its original meaning; the new member additionally sustains at least 100,000 joint-row projections projected into eight seconds. All four pinned engine gates pass on default, feature-compiled-unarmed and armed builds.
>
> **PASS.** From the bare mixed-61 request, at the 10.000-second checkpoint, at least 3 of 9 distinct seeds have published a non-constructor layout with raw-source depth ≤168.484 mm. Every emitted publication at every time passes Exclusive `r=2.500` and the untouched publication contract. The complete non-interpolated 3/10/30 curve, all nine seeds, is reported.
>
> **FAIL.** A valid round with fewer than 3 of 9 qualifying seeds fails the funded `JointSmallBiteSeparator + cut-close 0.1% homotopy` member. Proxy depth, best seed, median alone, or a publication completed after 10.000 s cannot change that verdict.
>
> **FORBIDDEN RESCUES.** No Sparrow fixture as a seed or warm start; no Sparrow or jagua code linked into the engine; no `general_relaxed`, portfolio, m34, crot, contact-block, old jump, Union kernel, allowance, 2.502 radius, enlarged repair band, alternate feature arm, seed substitution, wall interpolation, post-result bite change, or rerun selected by outcome.
>
> **FAILURE LICENSE.** A failing valid round licenses one read-only funnel autopsy: `bitesStarted → proxyBandReached → exactAttempted → dualValidPublished → ≤168.484`. It licenses a rerun only for a line-level violation of this frozen specification accompanied by a red/green minimal vector. Otherwise the member is closed; another move family is a separately funded proposal.

Use a contemporaneous interleaved wall-arm control, with AB/BA order alternated across seeds, because the old wall path moves several millimetres between sessions. But 168.484 remains the absolute judge; the paired control cannot raise or lower it. Also report that the historical 168.484 arm had wall max 10.30 s and reproduced 0/3, so it is a quality reference rather than an exact 10.000-second replay. [shipped-surface.md](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/shipped-surface.md:54)

## 5. Determinism and honesty

Fixed-work contract:

- same request, seed, binary, toolchain, feature set, worker count and work cap → bit-identical epoch targets, cut IDs, poses, row weights, publications and document;
- fixed libm path;
- stable row/component order;
- counter-derived cuts and worker permutations;
- fixed work per worker and barrier;
- serial ordinal merge;
- no clock in state transitions.

Wall mode may stop after a different number of batches. Every wall publication records its fixed-work ordinal, so it can be replayed exactly.

New mandatory counters:

- constructor wall/work/depth;
- bite start depth, target, β and cut;
- PGS sweeps and joint projections;
- component attempts/commits;
- proxy-band arrivals;
- exact attempts and refusal reason;
- target-to-published giveback;
- exact parent fingerprint before and after every success;
- successful/failed bites and work per bite;
- incumbent at actual 3/10/30 timestamps.

The shrink-specific self-deceptions to forbid are:

- counting the constructor as the qualifying child;
- plotting target or Φ instead of exact depth;
- advancing from proxy-feasible rather than published poses;
- hiding repair giveback by deriving the next target from \(T\);
- tuning β or the split sequence against mixed-61;
- counting a late batch at an earlier checkpoint;
- quoting only the worker or seed that crossed 168;
- treating Sparrow’s optimizer-only 10 seconds as bare-request wall time.

## 6. Workflow

Use three agents, staged rather than three simultaneous core editors.

1. **Specification commit first.** Freeze the gate, formulas, one feature combination, and a Sparrow source ledger at revision `14f4868f`, including the timer-definition finding. No engine code.

2. **Core agent.** Owns new `projection.rs`, `component.rs`, paired-row rebuild, weight update, and mathematical unit vectors. It must not touch `homotopy.rs` or the driver.

3. **Schedule agent, after the core API lands.** Owns `homotopy.rs`, resumable epoch state, exact-parent installation, eight-worker barrier/merge, and changes to `mod.rs`.

4. **Evidence/red-team agent.** Owns the example driver, FAST scripts, fixed-work replay, wall control and committed evidence. It reviews but does not rewrite the solver.

Commit order:

1. spec/source ledger;
2. joint row projection and two-body/boundary vectors;
3. paired-row cache audit;
4. component Y vector;
5. cut-close and failed-bite rollback vectors;
6. exact-parent promotion;
7. resumable eight-worker batches;
8. FAST tier;
9. frozen 3/10/30 gate run;
10. heavy gates/suites only after the result is frozen.

FAST additions that must run every iteration:

- default-build compile isolation and dependency-tree exclusion;
- current module tests, S0 and 1,000-state corpus;
- joint pair projection vector;
- three-piece boundary-chain vector that defeats a one-piece veto;
- cut-close vector proving only the selected side moves;
- a forced nonzero-repair publication followed by another bite;
- two-process fixed-work one-bite smoke, including epoch/parent fingerprints;
- one release throughput sample.

Also fix the FAST driver’s stale hard-coded worktree default at [fast.sh](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/drivers/fast.sh:34); otherwise an agent can test the wrong tree successfully.

The most likely implementation defect is exact-parent drift. Today `checkpoint()` copies a successful publication into `ExactIncumbent` but leaves the continuous `state` untouched. [mod.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:295) If homotopy takes its next bite from the pre-repair poses or from target \(T\), it silently ceases to be legal-to-legal and hides cumulative giveback. The forced-repair two-bite FAST vector must prove that `Publication.poses` are installed, every cache is rebuilt, and the next \(D\) is the measured published raw depth.


# ROUND 2 — quorum convergence

## Convergence verdict

I change my Round-1 position. Grok is right on the main member.

The funded round should implement the published relocate + coordinate-descent + all-row GLS + failed-separation disruption member on our signed-gap geometry and dual-valid publication pipeline. Joint two-endpoint PGS/component-Y is removed; it is a separately funded experiment.

My remaining hard disagreements are:

- use 8 competitive workers, not 1;
- use a guaranteed interior witness, not polygon centroid, for disruption followers;
- interleave the non-judging wall control rather than run one nearly meaningless sample afterward;
- install repaired publication poses before the next bite;
- do not silently rename the old 100K “piece proposal” throughput threshold as 100K sample evaluations.

## 1. Material differences marked

| Difference | Verdict | Reason |
|---|---|---|
| Relocate+CD+GLS+disrupt versus joint PGS/component-Y | **AGREE with Grok** | Sparrow’s actual member is the former. The owner’s question is now whether our engine can exploit the proven family member/regime, not whether our pre-source speculative solver works. |
| Joint projection/component-Y in this round | **AGREE: CUT** | Unsupported by the source and would confound the experiment. It remains a legitimate independent idea, but not this member. |
| 25 focused + 50 container samples, current pose included, three finalists | **AGREE** | These are the published/default member parameters, not instance constants. Freeze them before the run. |
| Two-stage axis CD, accept-equal | **AGREE** | It removes our strict local veto and is integral to the member, not a tuning option. |
| Sixteen sampled orientations plus continuous wiggle | **AGREE** | Search remains continuous after the sampled seed; no 2.5° catalogue. |
| All-row multiplicative GLS every master iteration | **AGREE** | Use the published `0.95` inactive decay and `1.2…2.0` active schedule on our \(v^2\), not my alternative formula. One GLS dialect only. |
| Signed-gap Φ, source rings, Exclusive+contract publication | **AGREE** | This is what keeps the optimizer ours and the product contract untouched. |
| Split-and-close 0.1% exploration | **AGREE** | This is the central source-backed correction to C175’s 5–7 mm shock. |
| Exploration failure: persist at W, pool+disrupt | **AGREE** | Restoring the parent and halving would substitute compression semantics for exploration and would not test the proven regime. |
| Compression failure: restore exact parent, time-decayed smaller bite | **AGREE** | This is legal-to-legal compression and should remain distinct from exploration. |
| Old strip/ball jump | **AGREE: CUT** | It is absent from Sparrow and measured catastrophic here. |
| One worker in this round | **DISAGREE** | The end-to-end gate is wall economics, and Sparrow’s recorded member includes eight competitive separator workers. One worker would knowingly test a handicapped member and create a predictable “worker follow-up.” |
| Eight workers as a second mechanism | **DISAGREE** | It is variance reduction inside one separator iteration, not another operator: same master, different deterministic sweeps, stable winner. [separator.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:145) |
| 80/20 of remaining wall | **AGREE**, with one correction | Preserve the ratio after our constructor because our 10 s includes it. Check the clock at complete worker-sweep barriers, not only whole bites, or one failed separation can overrun the gate. |
| Hard wall cap of 1.4 s inside constructor | **DISAGREE** | That makes the starting layout load-dependent. Use the constructor’s frozen deterministic work/configuration; its measured ~1.4 s is charged to wall but is not an internal clock decision. |
| Separate 3/10/30 runs, each 80/20 | **AGREE** | This matches a budget-aware anytime API and Sparrow’s reported methodology. State explicitly that these are budget-response cells, not checkpoints of one 30-second prefix. |
| Contemporaneous control once afterward | **DISAGREE** | A single draw from a 0/3-reproducible arm provides almost no box information. Use interleaved AB/BA controls, diagnostic only; 168.484 stays absolute. |
| Three agents/two waves | **AGREE** | Safe once a specification commit precedes both waves. |
| Failure licenses a later worker-count round | **DISAGREE** | Use eight now. A valid failure licenses no automatic scaling rescue. |
| Counter-hash/Halton instead of Sparrow RNG | **KNOB**, freeze counter-based | This preserves deterministic trajectories and does not change the operator class. |
| S1 and triangle become relocate regressions | **AGREE** | They detect an emptied relocate before the wall battery. |
| First 0.1% bite canary | **AGREE** | It is an implementation prerequisite, not a substitute for the end-to-end gate. |
| Centroid-in-ring disruption followers | **DISAGREE** | A concave polygon’s area centroid can be outside material. Sparrow uses its pole of inaccessibility, a guaranteed interior point. [explore.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/explore.rs:218) |
| “Different in area OR diameter” | **DISAGREE: source says AND** | The recorded binary requires both area and diameter to differ beyond 1%, then falls back to any other item. [explore.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/explore.rs:145) |
| 100K relocate-evals as inherited throughput pin | **DISAGREE** | A sample evaluation is not the previous piece proposal. Renaming the unit manufactures continuity. Version the metric and report eval/s, evals/relocate and relocates/s. |
| Sparrow timer asymmetry | **AGREE: document, do not adjust** | Import and LBF precede its phase timeout. Our owner’s gate explicitly begins from the bare request, so ours remains stricter. [optimizer/mod.rs](/var/lib/t3/tmp/sparrow-bench/src/optimizer/mod.rs:33) |

## 2. FAST tripwires

Grok’s tripwires subsume most of mine:

- container-wide proposals must actually commit;
- Φ=0 without exact publication must not advance W;
- exact split-and-close bit vector;
- first-bite canary;
- fixed-work bite sequence;
- S1/triangle;
- dependency and RNG hygiene.

They do not subsume three important checks.

### Mandatory addition 1: exact-parent drift

Current `checkpoint()` installs the publication only in `ExactIncumbent`; it leaves continuous `state` unchanged. [mod.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:295)

Add a forced-nonzero-repair, two-bite vector:

1. bite A reaches the 4 µm band;
2. publication repair moves at least one pose;
3. publication succeeds;
4. assert that engine state poses equal `Publication.poses`;
5. assert geometry and all incremental rows equal a cold rebuild;
6. assert bite B derives D from published raw depth, not target T or pre-repair proxy depth.

Without that, “legal-to-legal” is false and repair giveback accumulates invisibly.

### Mandatory addition 2: eight-worker merge

Because the final member uses eight workers, FAST must run two processes and assert identity of:

- each worker seed;
- each master snapshot;
- winning worker ordinal;
- pose and weight fingerprint after every master iteration;
- exact parent after every bite.

A test that only proves workers=1 deterministic is insufficient.

### Mandatory addition 3: driver provenance

The current FAST script defaults to an obsolete worktree. [fast.sh](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/drivers/fast.sh:34) Resolve the root from the script/repository or require it explicitly; otherwise the strongest tripwires can validate the wrong tree.

I agree that the most likely new-member defect is Grok’s “neutered relocate”: samples are evaluated, but a leftover strict filter or step-radius cap prevents a distant winner from committing. Exact-parent drift is complementary and ranks second.

One correction to the throughput tripwire: keep the existing cold-Φ, row rebuild and cell-gap thresholds literally. Introduce a new metric version for relocation:

- `sampleEvaluations`;
- `sampleEvaluationsPerRelocate`;
- `relocatesPerSecond`;
- `containerWinners`;
- `focusedWinners`;
- `stayPutWinners`.

Do not preclaim parity from `≥100K/8s`; the end-to-end gate remains the economic judge.

## 3. No-copying and the funded question

Yes: under the owner’s clarified rule, this member is within scope.

The algorithms are published and explicitly presented as a reproducible nesting heuristic in the [Sparrow paper](https://arxiv.org/abs/2509.13329). Implementing those algorithm descriptions with attribution, using:

- our state types;
- our deterministic sampler;
- our source-ring signed-gap objective;
- our incremental cache;
- our exact round kernel;
- our untouched contract validator;
- no `jagua-rs` dependency;
- no copied source text or functions;

is the same kind of literature-derived implementation already accepted for Egeblad and Imamichi.

Call it “paper-derived relocate/GLS homotopy,” not “a Sparrow port.” Commit a provenance table listing each adopted concept, paper algorithm, source-confirmed default, and our implementation difference.

The funded experiment is now:

> Can our exact-contract engine beat 168.484 using the family’s demonstrated relocate/GLS/split-and-close regime?

It is not:

> Does our novel coordinated contact solver work?

The latter was the correct question before source inspection. The owner’s new authorization supersedes it.

## 4. Converged member

I sign Grok’s member with four amendments:

1. eight competitive workers;
2. a guaranteed interior witness for disruption followers;
3. repaired publication poses atomically become the next legal parent;
4. wall checks at worker-sweep barriers.

A deterministic interior witness is cheap: store the centroid of the first positive-area ear-clipped cell in `PieceSource`. Unlike an area centroid, it is guaranteed inside material. Transform that witness and test it against the swapped source ring.

The disruption predicate follows the recorded source exactly:

- large set defined by cumulative convex-hull area cutoff;
- second piece preferred only when both area and diameter differ by more than 1%;
- otherwise any distinct piece;
- interior-witness followers receive the corresponding rigid transform.

My Round-1 joint projection/component design is withdrawn from this round. The minimum amendment that would make it acceptable later is a separately pre-committed A/B against this shipped relocate member at equal wall—not co-arming them.

## 5. Other convergence decisions

### Workers

Eight, from the start.

Each master iteration:

1. clone identical pose, row and weight state into workers 0–7;
2. give each a counter-derived colliding-piece permutation and independent sample stream;
3. run one complete sequential relocate sweep;
4. finish equal work;
5. select minimum total weighted Φ, stable worker-ordinal tie;
6. install only that state;
7. update all master weights once.

No early cancellation or completion-order adoption.

### Exploration failure

Follow the published regime:

- retain current infeasible W;
- add the minimum-raw snapshot to the pool;
- select a deterministic Normal-biased pool rank;
- reset weights for the restored pool state;
- disrupt;
- continue at exactly W.

No parent restore, no β halving, no W enlargement.

If proxy Φ reaches zero but exact publication refuses, classify it as a failed separation: otherwise every piece is skipped forever and the loop spins at a false legal state.

### Compression failure

Restore the last dual-valid parent, including repaired poses, reset weights, draw a deterministic split, compute the time-decayed bite at attempt start, and retry. Incomplete attempts at the phase deadline are discarded.

### Time

After the deterministic constructor finishes, compute remaining wall \(R\):

- exploration deadline: `now + 0.8R`;
- compression deadline: overall 10-second deadline.

No clock inside a relocation, CD walk or worker sweep. Check between synchronized sweeps so deadline overrun is bounded.

At the exploration deadline, compression starts from the best exact parent, never the current infeasible state.

### Control

Use nine diagnostic wall-control cells interleaved AB/BA with the nine new-engine 10-second cells. They are separate processes, never lanes.

The pass threshold remains absolute 168.484. The paired control:

- cannot rescue a fail;
- cannot kill a pass;
- is reported to expose session drift.

A single afterward control is too weak given the measured 2–5 mm box movement.

### Timer asymmetry

Document it, do not compensate:

- Sparrow’s `--global-time` begins after import/LBF;
- ours starts before decomposition/constructor;
- our 168.484 gate remains bare-request because that is the owner’s contract.

For future direct 150 comparisons, publish both optimizer-only and bare-request wall numbers.

### Workflow

Adopt Grok’s three-agent/two-wave structure, preceded by a spec commit:

1. Commit specification, source/paper provenance and gate.
2. Wave 1A: relocate/CD/GLS plus FAST unit vectors.
3. Wave 1B: split-and-close/disruption primitives plus vectors.
4. Wave 2C: eight-worker master loop, explore/compress schedule, exact-parent handoff, wall driver and evidence.

Only the integration agent touches `Engine::run`.

## 6. Single implementation spec I sign

### Representation and objective

- Existing continuous source-ring poses and decomposition.
- Existing signed-gap pair and boundary rows.
- `guided = Σ w_r v_r²`.
- Source rings only; no simplification, poles, miter envelopes or jagua.

### Relocate

For every currently colliding piece, in a counter-derived permutation:

- current pose;
- 25 focused samples;
- 50 strip-wide samples;
- 16 rotation seeds for continuously rotatable pieces;
- retain three unique candidates;
- coarse then fine axis CD;
- accept equal;
- commit minimum incident weighted Φ.

Mirror remains fixed. Candidate bounds use physical left/right/bottom and sag-less depth top.

### Master separator

- Eight competitive workers from one master.
- Stable winner by total weighted Φ then ordinal.
- All-row GLS after every master iteration:

  - inactive `w=max(1,0.95w)`;
  - active `w *= 1.2 + 0.8(v/vmax)`.

- Track minimum raw Φ.
- Explore limits: 200 no-improvement iterations, three strikes.
- Compression: 100, five strikes.
- A ≥2% raw improvement resets the strike.
- Rollback inside a separation retains weights.

### Exploration

- Exact constructor layout D; no affine shock.
- Centre cut; `W ← 0.999W`.
- Translate only pieces whose transformed centroid lies above the cut.
- Separate through infeasible states.
- Advance only on dual-valid publication.
- Successful repaired publication poses become the next continuous state and D becomes published raw depth.
- On failure, stay at W, pool+disrupt+retry.

### Compression

- Remaining 20% wall.
- Restore best dual-valid parent per attempt.
- Deterministic strip-wide cut.
- Time-decayed `0.0005 → 0.00001` bite.
- Success installs repaired publication state; failure discards attempt.

### Publication

Unchanged:

- band 4 µm;
- Exclusive `r=2.500`, allowance zero;
- repair ≤4n rows and ≤16 µm/piece;
- untouched contract validator;
- raw improvement ≥1 µm;
- every output publication dual-valid.

### Determinism

- Counter-derived permutations/samples/pool draws/cuts.
- Fixed worker work and synchronized barrier.
- Stable ordinal merge.
- No clock inside solver work.
- Fixed-work two-process identity.
- Wall observed only between sweeps and phase boundaries.

### FAST

All Grok tripwires plus:

- forced-repair two-bite exact-parent installation;
- eight-worker merge identity;
- correct repository-root assertion;
- versioned relocation throughput metrics.

The first-bite canary must publish at `0.999 × constructor_depth` before the nine-seed wall battery runs.

### End-to-end judge

- Mixed-61, seeds 0–8, bare request, eight workers, one frozen feature combination.
- Separate 3/10/30 budget cells, no interpolation.
- PASS: ≥3/9 non-constructor exact-valid publications ≤168.484 by 10 seconds.
- 168.484 is absolute.
- Interleaved controls diagnostic only.
- Regression floor fully green.
- No invalid publication.
- No post-result tuning or extra operator.

A valid failure licenses one funnel autopsy and only a pivot-grade implementation correction. Because workers=8 are already present, it does not license a scaling follow-up.

Remaining refusal: I will not sign the one-worker version. It would test a deliberately incomplete instance of the demonstrated member against an eight-worker wall target and pre-authorize the predictable excuse.
