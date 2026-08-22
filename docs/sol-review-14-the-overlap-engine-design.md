# Sol review 14 — the overlap engine design (2026-08-22, two rounds)

Consultation for the overlap-tolerant continuous engine (the owner's directive:
Sparrow-class at 10 s without copying). Round 1 = independent design against
the six-question brief; round 2 = convergence after reading Grok's full reply.
Codex resumed instance, model_reasoning_effort=max, sandbox read-only. Verbatim.

# ROUND 1

Verdict: fund the direction, but as a gated 3–5 day geometry/repair spike followed by one vertical prototype—not as a month-long engine build. It is genuinely outside the retired operator space only if:

- infeasible states persist across moves;
- no exact or canonical-envelope predicate truncates a move;
- arbitrary rotations do not build offsets, NFPs, or catalogue entries;
- exact geometry appears only when attempting publication.

Otherwise this becomes contact-block or m34 under a new name.

The public provenance is clean: relaxed overlap minimization with guided penalties is an established family in [Egeblad, Nielsen & Odgaard](https://www.sciencedirect.com/science/article/abs/pii/S037722170600302X); nonlinear separation and larger perturbations also predate Sparrow in the [extended local-search literature](https://www.sciencedirect.com/science/article/pii/S0305054811001596). Use those family-level ideas and derive our own state, objective, moves, and schedule. Do not inspect or port Sparrow’s optimizer.

## 1. Minimal prototype architecture

New, feature-gated module, initially reachable only from a benchmark/example:

```text
search/continuous_overlap/
    mod.rs              public optimize_continuous()
    state.rs            poses, contact matrix, exact incumbent
    decomposition.rs    deterministic material triangulation
    contact.rs          signed convex-cell distance + witnesses
    broad_phase.rs      f64 piece/cell AABBs
    energy.rs           raw and guided penalty
    descent.rs          one-piece SE(2) updates
    homotopy.rs         strip targets and affine shocks
    publish.rs          round check, micro-repair, contract validation
    diagnostics.rs      work and anytime trace
```

Do not implement it as another `ExplorationKernel`: that seam still consumes rotations baked into concrete legacy surrogates and catalogues ([kernel/mod.rs:65](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/kernel/mod.rs:65)). That is precisely the representation this experiment must escape.

### Representation

Use one complete layout with:

```rust
Pose {
    tx_mm: f64,
    ty_mm: f64,
    theta_rad: f64,
    mirrored: bool,
}
```

- `tx`, `ty`, and `theta` remain continuous `f64`.
- Start from the constructor’s angle and allow `theta` to accumulate across the full circle; no local angle window and no 2.5° catalogue.
- Freeze mirror choice in Round 1. Mirror flips, swaps, and restarts are separate variables.
- Cache transformed source triangles and AABBs for the current pose. A proposal transforms only the moving piece and recomputes only its `n−1` contact rows.

Do not snap the pose itself to 1 µm. That would be a second, unnecessary quantization. Existing placements are continuous `f64` ([general_fast.rs:125](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:125)); the round kernel canonicalizes the transformed rings through `GridSet::of` ([round_envelope.rs:195](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:195)).

For Round 1, support the simple outer-ring population used by mixed-61 and reject holes explicitly. Do not silently fill them. Proper constrained triangulation of regions with holes is a transfer-round requirement. The current relaxed surrogate also refuses holes ([general_relaxed.rs:18238](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:18238)), but its private ear clipper should not be copied into a new coupled dependency.

### Overlap measure

Use deterministic convex decomposition into triangles over the unoffset source material. For triangle cells \(A_a,B_b\) at current poses, form their configuration-space difference:

\[
C_{ab}=\operatorname{conv}\{R_i a_u+t_i-(R_j b_v+t_j)\}
\]

There are only nine point differences. Define signed distance:

\[
s_{ab} =
\begin{cases}
\operatorname{dist}(0,C_{ab}), & 0\notin C_{ab}\\
-\operatorname{dist}(0,\partial C_{ab}), & 0\in C_{ab}.
\end{cases}
\]

Thus disjoint cells have positive separation and intersecting cells have negative penetration depth. The closest Minkowski feature retains its two material witnesses and outward normal.

For a piece pair:

\[
v_{ij}=\max_{a,b}[c_{\rm pair}-s_{ab}]_+,
\qquad
E_{ij}=\tfrac12 w_{ij}v_{ij}^2
\]

where `c_pair = total_padding + 2*sag`, matching the material contract described by the round kernel ([round_envelope.rs:6](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:6)).

Use the maximum cell violation in Round 1:

- zero means every material cell pair clears;
- it avoids weighting a shape according to how finely it was triangulated;
- it supplies one deterministic active witness;
- it is nonsmooth, but this optimizer does not require a smooth global gradient.

Do not begin with overlap area, raster SDF, or generic GJK/EPA:

- Clipper overlap area has already inverted thousands of local rankings and worsened the trajectory in this lifecycle.
- Raster/SDF introduces angle and resolution artefacts exactly where legalization is most sensitive.
- Signed segment distance alone misses containment.
- GJK/EPA over a nonconvex polygon still requires a convex decomposition; on triangles the nine-point Minkowski query is smaller and auditable.

Boundary violations are analogous:

\[
v_L=[e-\min x_i]_+,\quad
v_R=[\max x_i-(W-e)]_+,
\]
\[
v_B=[e-\min y_i]_+,\quad
v_T=[\max y_i-(D-e)]_+.
\]

Keep target depth \(D\) outside the objective as a hard penalty boundary. Do not optimize `E + λD`; that invites the optimizer to trade illegality against depth.

The total objective is:

\[
E(q;D)=\sum_{i<j}E_{ij}+\tfrac12\sum_{i,k}w_{ik}v_{ik}^2.
\]

Maintain raw and guided versions separately.

### Move scheme

One deterministic Gauss–Seidel trajectory:

1. Select the piece with maximum incident guided energy; stable tie by input ID.
2. Sum its active pair and boundary forces.
3. Obtain torque from each witness:
   \[
   \tau_i=(p_i-c_i)\times (w\,v\,n).
   \]
4. Normalize in the SE(2) metric
   \[
   \|\Delta t\|^2+(R_i\Delta\theta)^2,
   \]
   where \(R_i\) is that piece’s maximum source-vertex radius.
5. Backtrack on a fixed ladder from
   `max(clearance/4, median_diameter/128, 8 µm)` down to `0.25 µm`.
   The angular step is capped by \(R_i|\Delta\theta|\le|\Delta t|_{\rm trust}\).
6. Accept only a strict decrease in current guided energy, with canonical pose fingerprint as the final tie-break.

Store the `n×n` contact matrix. A rejected or accepted move recomputes only the moving piece’s rows and applies score deltas; no full rescore and no allocation.

After one complete `n`-piece sweep without raw improvement, apply our own guided penalty:

\[
u_{ij}=\frac{v_{ij}}{1+p_{ij}},
\]

increment the integer `p` of the lexicographically first maximum-utility contact, and use `w=1+p`. This changes the local landscape while allowing raw overlap temporarily to worsen.

No swaps, random restarts, low-discrepancy teleports, archive, or mirror flips in Round 1. If fixed-target repair works but the anytime curve later stalls, the one admissible Round-2 addition is a deterministic relocation of the highest-pressure piece. If fixed-target repair itself fails, adding jumps merely hides a bad separation field.

This is materially different from contact-block: that operator asks the exact composite to approve every line-search step and is reduced to 1% of its modeled vector ([contact-block README:18](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/contact-block/README.md:18)). Here exact rejection cannot shorten an intermediate move.

It also avoids the rotation tax: the existing continuous path rotates and offsets geometry per orientation ([general_relaxed.rs:18179](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:18179)); this path rotates source triangles with one `sin_cos`, without offset or angle-key cache.

### Strip homotopy

Let \(D^\*\) be the best exact depth. Derive a safe request-level lower scale \(L\) from raw material area, usable sheet width, edge clearance, and the tallest piece. Do not reuse `portfolio::area_lower_bound_depth_mm`: it explicitly offsets with the miter/search allowance ([portfolio.rs:9181](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:9181)).

Initial target:

\[
T=D^\*-0.10(D^\*-L).
\]

Before descent, compress all piece centroids along the long axis by a common affine factor, chosen by bisection so the translated layout reaches approximately \(T\). Shapes remain rigid. This deliberately distributes overlap through the layout instead of creating only top-boundary offenders.

At each work epoch:

- descend at fixed \(T\);
- when proxy violation enters the publication band, attempt publication;
- on success, set \(D^\*\) to raw exact depth and generate the next 10%-residual target;
- on failure at the epoch limit, replace \(T\) by \((T+D^\*)/2\), retaining the infeasible state rather than restarting.

Precommit eight work epochs at 10 seconds. There is one trajectory and one protected exact incumbent.

This explicitly enters the 58.62% bulk-overlap population that current search suppresses ([skip-pile README:170](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/skip-pile-diagnostic/README.md:170)); it is not another attempt to monetize the 0.80% round/miter released region, which was worth zero depth ([skip-pile README:261](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/skip-pile-diagnostic/README.md:261)).

## 2. Ten-second architecture

Use the fast constructor, not a random overlapping throw.

A random throw would spend most of the budget rediscovering coarse structure and would leave no exact fallback. The constructor supplies:

- an exact-valid incumbent at about 1.4 seconds;
- a realistic topology to perturb;
- an anytime answer if the continuous trajectory never legalizes.

Provisional 10-second allocation:

| Component | Budget |
|---|---:|
| Constructor | 1.4 s measured target |
| Decomposition/contact preprocessing | ≤0.2 s |
| Eight continuous epochs | 8.0 s |
| checkpoint/headroom | 0.4 s |

At 3 seconds, run one or two epochs; at 30 seconds, continue the same trajectory for 24–28 epochs. Do not restart at 30 seconds until the single-trajectory curve is known.

The old engine’s 265 ns “candidate evaluation” does not transfer automatically: one signed cell-contact update is different work. Define and report:

- piece proposals;
- triangle-pair signed-distance queries;
- AABB rejects;
- accepted moves;
- weight updates;
- exact checkpoints;
- legalization corrections.

Gate the implementation on projecting at least 100,000 complete piece proposals into the approximately eight-second search slice. If it cannot reach roughly 12.5K piece proposals/s after incremental row updates, it is not a plausible 10-second engine even if its triangle primitive looks fast.

### Publication pipeline

```text
continuous state
  → near-feasible proxy
  → canonical round check
  → bounded µm repair
  → untouched material-contract validation
  → best_exact
```

Important details:

1. Do not round `tx`, `ty`, or `theta`. Transform the source rings at the continuous pose and let `GridSet::of` perform the sole 1 µm canonicalization.
2. Set search allowance to zero. It is explicitly search-only and never part of publication legality ([general_fast.rs:67](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:67)).
3. Call the pure round predicates at the requested contract radius, not `KernelMode::Union` and not a process-global arm. `pair_admissible` and the critical-radius diagnostics already exist ([round_envelope.rs:494](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:494), [round_envelope.rs:607](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:607)).
4. Always finish with untouched `validate_placements_against_contract` ([general_fast.rs:3452](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:3452)). That remains the publication authority.

For non-integer-micrometre contract distances, Round 1 should bypass the integer round preflight rather than silently round the contract outward. Mixed-61’s 5.0 mm is exactly representable in the kernel’s unit.

### Local repair

Attempt repair only when every continuous violation is within a derived canonicalization guard:

\[
\epsilon_{\rm grid}=2\lceil\sqrt2\cdot 1\mu{\rm m}\rceil=4\mu{\rm m}.
\]

For an exact failing pair:

- if `critical_two_r_micron` reports a shortfall no larger than \(\epsilon_{\rm grid}\), take the continuous active normal;
- move the two pieces apart by `shortfall + εgrid`, divided according to available sheet slack;
- if one side has no slack, put the whole correction on the other;
- freeze rotations during repair.

For a boundary failure, translate inward by the measured deficit plus the same guard.

Run ordered Gauss–Seidel corrections, at most `4n` rows, and cap cumulative displacement of any piece at `4εgrid = 16 µm`. If a material-overlap row has no critical radius, a contract deficit exceeds the band, or the repair consumes more than the cap, discard the checkpoint. Do not let “legalization” return 0.5–5 mm and claim the penalty search found it.

The infeasible search state may continue after a rejected checkpoint, but `best_exact` never changes. Deadline expiry returns `best_exact`.

## 3. Replace the proposed gate

I disagree with “Round 1 has no gate,” and “100% publication rate” needs two definitions.

- Every returned publication being exact-valid is an invariant. One invalid output halts the experiment.
- The fraction of runs in which this engine creates a new exact-valid child is the mechanism rate. Constructor fallback makes “100% valid outputs” otherwise trivial.

Also, three seeds repeated three times are not nine seeds. Either use nine distinct predeclared seeds, or make claims over three seed medians and treat repetitions only as timing noise.

### Gate 0 — contact/repair falsifier, before the engine

Effort: 3–5 days.

On at least 10,000 deterministic states produced from three constructor layouts by 1%, 3%, and 10%-residual affine compression plus predeclared SE(2) perturbations:

- zero proxy-feasible/exact-invalid states outside the 4 µm band;
- no containment false-feasible case;
- an accepted negative-force step improves the independently measured active violation in at least 95% of cases;
- it does not worsen independent total violation in at least 80%;
- projected throughput is at least 100K complete piece proposals in eight seconds.

The independent score should combine exact material intersection area with exact segment-clearance and boundary deficits. It is diagnostic only, not the optimizer’s objective.

Kill or replace the contact model immediately if any false-feasible state lies outside the band, or if force correlation/throughput misses. Do not proceed to schedule work.

### Round 1 — vertical mechanism gate

Include the constructor and first 3/10/30-second curve, but precommit this causal test:

- start from a `0.10(D₀−L)` affine shock;
- permit the target bisection described above;
- within two solver seconds, publish an exact child at least `0.05(D₀−L)` below \(D₀\) on at least six of nine distinct seeds;
- produce a strict continuous-engine publication by 10 seconds on at least six of nine;
- every reported layout passes independent contract validation.

This asks whether overlap descent can return from a meaningful distributed infeasibility without giving back most of the compression. If it fails, the paradigm as implemented is dead; do not rescue it with a scheduler.

### Round 2 — reproducible 10-second gate

Run a contemporaneous, interleaved control—not only historical `175.388`.

Require all of:

- treatment median ≤175.388 mm;
- treatment beats the same-seed reproducible plan on at least six of nine distinct seeds;
- paired median gain ≥1.0 mm;
- p95 from-request wall ≤10.0 seconds;
- at least six runs publish a strict continuous child.

One predeclared addition is allowed between Rounds 1 and 2: a single worst-pressure-piece relocation after repeated guided stalls. No parameter campaign.

### Round 3 — product gate

Against a contemporaneous wall control:

- treatment median ≤168.484 mm;
- at least six of nine paired seed wins;
- paired median gain ≥2.0 mm;
- p95 total wall ≤10.0 seconds;
- all returned layouts exact-valid;
- shapes-17 and triangle-20 produce no validity failure or paired median regression.

If Round 3 fails, stop the 150@10s program for this engine family. A 0.2–0.8 mm signal is not enough; the known gap is approximately 18 mm.

## 4. Determinism and honesty

The honest contract is:

> Same request, seed, binary, x86 target, Rust toolchain, libm implementation, feature set, worker count, and fixed work quota produce bit-identical poses, checkpoint sequence, and publications.

Do not promise cross-platform bit identity for arbitrary-angle `sin/cos`.

Round 1 should be single-trajectory and serial. If parallelism becomes necessary, parallelize only contact-row calculation:

- fixed contiguous pair partitions;
- workers write to predetermined slots;
- reduction in pair-ID order;
- no completion-order observation;
- no concurrent pose commits.

Pin:

- request SHA and normalized ring order;
- triangulation and tie rules;
- compiler, target features, and FMA policy;
- seed generator;
- initial constructor configuration;
- target fraction and epoch count;
- contact aggregation;
- trust and backtracking ladder;
- weight-update rule;
- exact-check threshold and cadence;
- repair cap;
- work-unit definition.

Use a counter-based random source if the later relocation needs randomness; key it by `(request seed, epoch, stall, piece ID, proposal ordinal)`.

There are necessarily two budget semantics:

- Replay/gates: fixed work, no clock reads in the trajectory.
- Production wall mode: check deadline only between deterministic batches and return `best_exact`; quality is not replay-identical under different load because a different batch count may finish.

Reuse the `plancal` protocol and load-testing discipline, but not its numeric calibration file or old work currency: it is keyed to the current portfolio’s counters ([robust-plan README:41](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/robust-plan/README.md:41)).

Mandatory telemetry:

- target depth and proxy energy never appear as anytime quality;
- only exact-valid raw source depth is plotted;
- log proxy-near attempts, round failures, contract failures, repair displacement, depth giveback, and time to first strict child;
- retain raw continuous poses and independent validation results;
- include constructor and preprocessing in wall;
- publish every seed, including fallback-only runs;
- freeze all constants before examining the Sparrow fixture.

The Sparrow pose fixture is a holdout for final reachability diagnosis only. It must not initialize a run or select parameters.

## 5. Reuse map

Use:

- `GeneralFastPiece`, settings, and placement boundary types ([general_fast.rs:51](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:51)).
- `construct_short_side_first` behind an `InitialLayoutProvider` adapter ([general_fast.rs:450](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:450)).
- `PolygonSet` source geometry and normalization.
- Pure `GridSet`, pair, boundary, and critical-radius functions from the round kernel.
- The untouched contract validator and raw-source depth measurement.
- Fast-contract validation only as an acceleration of that same final verdict.
- Existing whole-document fingerprints after extracting a neutral helper if necessary.
- Plan calibration methodology and the 3/10/30 trace format.
- Sparrow poses only after the gates are frozen.

Do not use:

- coordinator, archive, action pricing, or modes 20–34;
- `LaneSearch`, `general_relaxed`, its tracker, poles, pressure model, or private triangulation;
- oriented surrogate catalogues;
- miter offsets, Clipper overlap area, or NFP caches in the move loop;
- `GridSet` as the continuous broad phase—it is a canonical Boolean endpoint, not a directional field;
- Jagua or Sparrow optimizer code;
- exact validation in move acceptance;
- global kernel arming or Union policy;
- the portfolio’s miter-derived lower bound.

The default legacy path should remain byte-identical. Initial integration is a separate example/feature; no adoption into `GeneralRelaxedOutcome.result` until Round 2 passes.

## 6. Biggest risk and earliest exposure

The biggest risk is not f64 determinism. It is that the deepest-triangle witness field is not a useful navigation field for a nonconvex union: energy may fall quickly while contacts exchange, forces cancel, or the trajectory converges to a proxy-near state that cannot be legalized without returning the strip gain.

That failure has precedent. The retained-infeasible experiment lowered raw overlap from 1596 to 975 but never produced one feasible state ([next-generation-engine-plan.md:217](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:217)). The new design evades its named causes—source-faithful signed contacts, continuous SE(2), distributed compression, and persistent homotopy—but it does not make legalizability automatic.

The earliest cheap measurement is therefore not a 10-second curve. It is Gate 0 followed by the two-second affine-shock repair test. In particular:

- if energy drops by orders of magnitude but the exact child recovers less than half of the imposed `0.10(D−L)` shock on six of nine seeds, stop;
- if repair commonly exceeds 16 µm per piece, stop;
- if the contact loop cannot project 100K complete piece proposals into eight seconds, stop.

That combination exposes both ways to waste the month: a fast field that goes nowhere, and a good field that cannot run often enough.


# ROUND 2 — convergence (after reading Grok's reply)

Verdict: convergence is high. I accept Grok’s architecture except for three points I will not yield: random throw as the fatal family test, constructor-as-floor-only, and millimetre-scale publication repair. Those three would respectively confound initialization with separation, discard the most useful from-request structure, and hide legalization inflation.

## 1. Grok’s ten disagreements, marked

| # | Mark | Position |
|---:|---|---|
| 1 | **DISAGREE** | Constructor is both the anytime floor and the source of the primary ICS state. The state is immediately affinely compressed into infeasibility, so this is not mode-0/global-legalize. A uniform dense throw is a valuable stress cell, but failure from that distribution does not falsify shrink-and-separate from a structured start. |
| 2 | **AGREE** | The integer kernel is publication judge only, never \(\Phi\). My proposed nine-point Minkowski query was a continuous triangle penetration proxy, not finite-differencing the kernel. |
| 3 | **AGREE** | Round 1 must have a paradigm gate. “Infrastructure only” is too permissive. I change the single-thread timing of the absolute-depth clause below to avoid contradicting #10. |
| 4 | **AGREE** | Beating 175.388 is hygiene. The eight-worker Round-2 kill bar is the 168.484 wall reference, under a reproducible treatment work cap that fits the 10-second wall envelope. |
| 5 | **AGREE** | Every published incumbent must be exact-valid; failed attempts are expected. The meaningful mechanism rates are strict-child production, near-feasible-to-publication conversion, and repair giveback. |
| 6 | **AGREE** | Kill at Round 2 if the family cannot reproduce ≤168.484 median at 10 seconds. Do not kill solely because Round 3 misses 150.165; 160 median/one ≤155 is sufficient evidence of a generation change. |
| 7 | **AGREE** | Continuous \(\theta\) from the first solver sweep, unrestricted by a catalogue window. Starting angle depends on the start-state decision, but the search coordinate is continuous. |
| 8 | **AGREE** | ICS children must pass request-scoped Exclusive at \(r=2.500\), allowance zero, plus the untouched material contract. No Union and no 2.502. I reject an extra pre-snap of the translation; see publication below. |
| 9 | **AGREE** | PGS is a local separator, not the search. Persistent infeasible state plus guided weights and at least one real topology-changing jump belongs in Round 1. Jump type/order/stall threshold are **KNOBS**, not architectural disagreements. |
| 10 | **AGREE** | Round 1 solver is single-threaded. Round 2 uses eight deterministic independent trajectories and ordinal merge. |

### The four named convergence blocks

- **T-before-the-loop:** **DISAGREE as fatal gate; AGREE as diagnostic.** Run it before `shrink.rs`, but do not kill the family because a uniform 61-piece throw cannot unmix at 168.484 in two seconds.
- **Exclusive@2.500:** **AGREE**, request-scoped and followed by the untouched contract validator.
- **Constructor-as-anytime-floor-only:** **DISAGREE.** Use it as floor and structured primary start; the immediate affine shock supplies the infeasible transition.
- **Kill at Round 2 versus 168.484:** **AGREE.**

There is also an internal tension in Grok’s spec: #10 says not to judge a one-core prototype against the eight-worker wall result, while its Round-1 gate requires 3/9 one-core runs at ≤168.484 in ten seconds. I resolve this by making 10 seconds the strict-child gate and 30 seconds the single-thread reachability gate. Round 2 owns 168.484-at-10s.

## 2. Resolving the design deltas

### \(\Phi\): allocation-free signed convex gap, with both proposals retained as oracles

The hot primitive should be:

\[
s(A,B)=
\begin{cases}
+\operatorname{distance}(A,B), & A\cap B=\varnothing\\
-\operatorname{MTVdepth}(A,B), & A^\circ\cap B^\circ\ne\varnothing
\end{cases}
\]

and

\[
v_{ij}=\max_{a,b}[c_{\rm pair}-s(A_a,B_b)]_+,\qquad
\Phi_{\rm raw}=\sum_{i<j}v_{ij}^2+\sum_{i,e}v_{ie}^2.
\]

Resolution:

- Two convex source pieces: streamed SAT axes for overlap MTV; deterministic closest segment/vertex feature for disjoint clearance.
- Any nonconvex piece: deterministic source-material triangulation and maximum cell-pair violation.
- Active cell retains normal plus material witnesses for torque.
- Pair energy uses the maximum cell residual, avoiding triangulation-count bias.
- Boundary residuals use the request’s exact material-edge clearance.

Do not call existing `measure_convex_sat_penetration` directly in the hot loop. It is useful as a differential oracle, but it:

- returns `None` for both separation and exact contact and supplies no closest feature ([sat.rs:60](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/sat.rs:60));
- allocates an axes `Vec` per call ([sat.rs:78](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/sat.rs:78));
- already documents 20/269 ULP-scale residual differences ([sat.rs:23](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/sat.rs:23)).

Use two independent test oracles:

- existing SAT for overlapping convex cases;
- the nine-point triangle Minkowski hull for triangle signed-distance differentials.

Thus Grok’s SAT/closest-feature design becomes the hot implementation; my Minkowski construction becomes the independent small-cell oracle. That is stronger than choosing only one.

Only geometry rows involving the moved piece are recomputed. However, after updating those cached rows, fold all 1,830 cached scalar row values in fixed pair-ID order. That is only 1,830 additions—not 1,830 geometry queries—and prevents another incremental-tracker inheritance defect.

### Start state and the pre-loop probe

Both proposed tests should run; they answer different questions.

1. **Primitive Gate 0:** my deterministic contact/repair corpus catches wrong signs, containment, incremental drift, numeric false feasibility, and insufficient throughput.
2. **Inflation probe:** Grok’s cells catch whether a correct local field actually returns to exact legality.

Use these pre-`shrink.rs` cells:

| Cell | Purpose | Verdict |
|---|---|---|
| S0: untouched Sparrow fixture | \(\Phi\)/publication calibration | Fatal if \(\Phi\ne+0.0\), raw depth differs from **150.16451**, or either exact gate refuses |
| S1: frozen ±0.5 mm/±2° perturbation | local basin capture and publication repair | Fatal if it cannot republish within the locked 150.16547 strip |
| S2: frozen ±2 mm/±10° perturbation | damage tolerance representative of jumps | Diagnostic/yellow, not family kill |
| C175: constructor affinely shocked by \(0.10(D-L)\) | realistic from-request separator test | Fatal if none of three fixed seeds returns a strict exact child in two solver seconds |
| C168: constructor affinely squeezed into 168.484 | direct wall-basin stress | Diagnostic before schedule; becomes a Round-1/2 quality observation |
| T: uniform random throw into 168.484 | worst-case initialization stress | Diagnostic only |
| triangle-20 at 70.742 | exact-convex implementation canary | Fatal if it cannot legalize |

Why T is not fatal: a dense uniform throw changes both initialization and separation. Failure would not distinguish “bad \(\Phi\)” from “the distribution erased every useful coarse structure.” The evidence supplied in the brief says Sparrow itself continuously compresses one layout; random-from-dense-strip is not the family’s only admissible initialization.

### Solver: PGS locally, guided penalties and one jump globally

Converged Round-1 solver:

- Deterministic damped PGS, piece/pair order fixed.
- Translation and continuous rotation from the first sweep.
- Re-measure each affected row after each piece update.
- Backtracking accepts a decrease in incident guided \(\Phi\); no exact predicate is consulted.
- On a full-sweep stall, increment the integer guided weight of the maximum-utility persistent contact.
- After two guided stalls, relocate the highest-pressure piece.

The relocation jump:

- evaluates 16 deterministic low-discrepancy positions/orientations in the current strip;
- runs one bounded local sweep from each;
- chooses by guided \(\Phi\), stable fingerprint tie;
- commits for a full epoch even if raw \(\Phi\) temporarily worsens;
- never touches the protected exact incumbent.

No swap, mirror flip, or restart in Round 1. Their inclusion and order are knobs after the one-jump mechanism is measured. Adding all three immediately would make a failure uninterpretable.

### Round-2 bar

I withdraw my earlier 175.388 Round-2 bar.

The treatment is fixed-work for reproducibility, with its work vector calibrated so p95 total from-request wall is ≤10 seconds. It must then satisfy:

- median raw depth ≤168.484;
- at least 6/9 distinct seeds ≤168.484;
- contemporaneous paired win against the current wall arm, not historical comparison alone;
- all ICS publications dual-valid;
- transfer holds.

Failing this kills the 10-second production program.

## 3. Two-tier test discipline

### Feature boundary

One feature only for prototype testing:

```toml
overlap-ics = ["round-envelope-kernel", "fast-contract-validator"]
```

It must not imply `jagua-experimental`, `compression-schedule`, or anything in the existing relaxed/coordinator stack.

The prototype example has:

```toml
required-features = ["overlap-ics"]
```

### Fast tier: every implementation iteration, minutes

One script, direct exit statuses, no `tee`/`tail` pipelines.

#### 1. Compile-only default-build isolation

```sh
cargo check -p polygon-nesting-core --no-default-features --lib
```

This catches accidental unconditional imports, module visibility changes, and feature leakage. It does not prove semantic identity; the heavy gates do that.

Also assert dependency hygiene:

```sh
cargo tree -p polygon-nesting-core --features overlap-ics -e features
```

The script fails if the resolved tree contains `jagua-rs`.

#### 2. One release feature combo

All active tests use only:

```sh
--features overlap-ics
```

Run:

```sh
cargo test -p polygon-nesting-core --release \
  --features overlap-ics --lib search::overlap_ics::

cargo test -p polygon-nesting-core --release \
  --features overlap-ics --test validation_vectors \
  sat_penetration_matches_ts_oracle

cargo test -p polygon-nesting-core --release \
  --features overlap-ics --test canonical_grid_vectors

cargo test -p polygon-nesting-core --release \
  --features overlap-ics --test collision_builder_vectors
```

The new module tests must include:

- convex separated/touching/overlap/containment vectors;
- signed-gap symmetry and opposite normals;
- SAT-hot-path versus existing SAT oracle;
- triangle hot path versus nine-point Minkowski oracle;
- nonfinite input fails closed;
- source triangulation area and winding preservation;
- cached incremental rows exactly equal cold row reconstruction;
- cached-row fixed-order scalar fold exactly equals cold \(\Phi\);
- \(\Phi=0\) implies round+contract acceptance outside the 4 µm canonical band;
- a 4 µm synthetic exact failure is repaired within the same strip with the pinned 4 µm total pair correction;
- repair never changes target \(W\), never moves one piece by more than 16 µm, and never runs more than `4n` row corrections.

The 1,000-state deterministic contact corpus belongs here. The 10,000-state corpus is heavy.

#### 3. One two-process smoke canary

Run the release example twice as separate processes, fixed seed and fixed work vector.

The smoke document contains two cells:

**S0, pinned now:**

- request and committed Sparrow poses;
- 61 placements;
- `rawSourceDepthMm = 150.16451`;
- `phi.to_bits() == 0`;
- `kernelExclusiveValid = true` at `two_r=5000`;
- `contractValid = true`;
- no repair;
- `repairDepthGivebackMm = 0.0`.

**S1, fixed perturbation:**

- committed perturbation vector, not generated afresh;
- locked \(W=150.16547\);
- fixed solver work;
- dual-valid publication within the same \(W\);
- per-piece repair displacement ≤0.016 mm;
- repair depth giveback ≤0.050 mm.

After the first independently validated implementation establishes S1’s exact placement digest, pin that digest too. Before that, the numerical bounds above are precommitted.

Compare the two process outputs bit-for-bit after stripping an explicit closed list of wall fields. The comparison must include:

- every `x`, `y`, and `theta` bit;
- raw and guided \(\Phi\);
- step digest;
- work counters;
- exact attempts/refusals/publications;
- repair displacement and giveback;
- placement fingerprint and raw depth.

A fixed-work smoke, not a wall smoke, catches f64 trajectory nondeterminism.

### What must never be deferred to heavy

These are per-iteration requirements:

- No proxy-feasible/exact-invalid state outside the derived canonical band.
- Incremental rows versus cold rows.
- Fixed-order \(\Phi\) consistency.
- Locked-strip repair: publication repair may not enlarge \(W\).
- Repair displacement/giveback caps.
- Exact contract and Exclusive validation on every published child.
- Two-process fixed-work determinism.
- Nonfinite input rejection.
- S0’s pinned 150.16451/zero-\(\Phi\)/dual-valid canary.
- Default compilation without the feature.
- Evidence fields `W`, raw/guided \(\Phi\), `max_g`, exact attempts, exact publications, repair displacement, repair giveback, and published raw depth.

A full suite at the round boundary cannot repair a week of optimizing proxy depth or silently inflating legalizations.

### Heavy tier: round boundaries only

Run from a clean committed tree:

1. Default and `overlap-ics` release builds.
2. Four pinned gates on both builds, whole-document identity when the feature is compiled but unarmed:

   - g1: `206.869 / 8a7737381238fa4d`
   - g2: `159.09233022733062 / fa01012af1d559ae`
   - g3: `159.07876040364795 / e28fba007f8031d4`
   - g4: `164.0375677990678 / 49f094d7e59a9008`

   These values are committed in the existing gate protocol ([round-envelope-gate README:521](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-gate/README.md:521)).

3. Full four-suite protocol, plus the feature’s example tests if not already one of the four.
4. The 10,000-state contact corpus.
5. Full S0/S1/S2/C175/C168/T/triangle inflation probe.
6. Determinism:

   - two processes;
   - two independently built binaries with the same pinned toolchain;
   - same fixed-work trajectory;
   - worker 1 replay;
   - worker 8 ordinal-merge replay in Round 2;
   - placement/frontier/work/publication digests.

7. Nine distinct seeds, not three seeds repeated three times, at 3/10/30 seconds.
8. Mixed-61 plus shapes-17 and triangle-20.
9. Independent audit of every publication through Exclusive@2.500 and untouched contract validation.
10. Contemporary interleaved controls, raw staircase values only, no interpolation.

## 4. Single converged implementation spec

### State

- Independent `search/overlap_ics/`, feature `overlap-ics`.
- Continuous `f64` `(x,y,theta)` over source material; constructor mirror fixed in Round 1.
- Deterministic source-material triangulation; holes explicitly unsupported in Round 1.
- SoA transformed geometry.
- Cached piece-pair rows, fixed-order scalar fold.
- Persistent infeasible current state plus separately protected exact incumbent.
- `libm` trigonometry, stable piece/pair order, counter-based seeded jump proposals.

### \(\Phi\)

- Allocation-free signed convex gap:
  - streamed SAT MTV while overlapping;
  - closest material feature while separated;
  - triangle-cell maximum for nonconvex pairs.
- Pair clearance and edge clearance derived from the real request, including sag exactly as the contract does.
- Raw squared-hinge penalty and separately guided integer weights.
- Kernel never participates in \(\Phi\).

### Solver/search

- Damped PGS translation and continuous rotation.
- Exact row remeasurement after each changed piece.
- Guided contact-weight update after one stalled sweep.
- One topology jump after two guided stalls: relocate the highest-pressure piece through 16 deterministic continuous candidates and commit one bounded epoch.
- No exact validation in move acceptance.
- No swap/mirror/restart until this mechanism is measured.

### Start state

- Run the full fast constructor, approximately 1.4 seconds, once.
- Retain its exact result as anytime floor.
- Use its poses as the primary ICS state, immediately affinely compressed into an infeasible target.
- Random T is diagnostic only.
- Round 2’s eight workers start from the same constructor but use distinct deterministic affine perturbations/jump streams.

### Strip schedule

Let \(D^\*\) be best exact depth and \(L\) a safe raw-material request bound.

\[
T_0=D^\*-0.10(D^\*-L).
\]

Eight equal-work epochs at 10 seconds:

- exact success: publish and apply the same 10%-remaining-gap contraction again;
- failed epoch: retain the infeasible state and set \(T\leftarrow(T+D^\*)/2\);
- no restart;
- publication repair is constrained inside the same \(T\), never allowed to expand the strip.

The 10% fraction, stall count, 16 relocation samples, and epoch count are frozen Round-1 knobs.

### Publication

- Do not pre-snap pose translations.
- Transform continuous source rings, then canonicalize once through `GridSet::of`.
- Request-scoped round kernel Exclusive at \(r=2.500\), allowance zero.
- Untouched material contract validator.
- Attempt only when `max_g ≤ 0.004 mm` and raw depth can improve by at least 1 µm.
- Frozen-\(\theta\), same-strip repair:
  - at most `4n` row corrections;
  - at most 0.016 mm cumulative displacement per piece;
  - target \(T\) immutable.
- Failure returns to ICS; only dual-valid success updates the incumbent.

This is where I explicitly reject Grok’s 2 mm repair cap and 8 mm inflation allowance. A source-faithful \(\Phi\) reaching zero should disagree with canonical/exact geometry only at grid scale. Millimetre repair would conceal a broken proxy and recreate terminal legalization.

### Budgets

At 10 seconds:

- constructor: approximately 1.4 s;
- preprocessing: ≤0.2 s;
- continuous solver: approximately 8.0 s;
- safe checkpoint reserve: 0.4 s.

Use a deterministic work vector, not an invented exchange rate:

```text
pair_row_probes
convex_cell_gap_queries
pose_transforms
jump_proposals
exact_checkpoints
repair_rows
```

Stop at piece-proposal boundaries. Calibrate the vector so total p95 wall is ≤10 seconds. Report every component.

Round-0 performance kills:

- cold full mixed-61 \(\Phi\) geometry ≤200 µs;
- one moved-piece row reconstruction ≤20 µs;
- ≥1M convex-cell signed-gap evaluations/s single-thread;
- projected ≥100K complete piece proposals in the eight-second solver slice.

### Rounds and kills

**Gate 0, before schedule/parallel work**

- Fast mathematical corpus plus inflation cells.
- Fatal: S0, S1, C175, triangle canary, numeric soundness, or throughput failure.
- S2, C168, and random T are diagnostic.

**Round 1, single-thread, nine distinct seeds**

- Full 3/10/30 curve.
- By 10 seconds: strict dual-valid non-constructor child on ≥6/9.
- By 30 seconds: ≤168.484 on ≥3/9.
- Every ICS publication dual-valid.
- Median repair giveback ≤0.050 mm; target never expanded.
- Kill before parallelization if either mechanism clause fails.

**Round 2, eight deterministic workers**

- Fixed work calibrated to p95 total wall ≤10 seconds.
- Median ≤168.484.
- ≥6/9 seeds ≤168.484.
- Contemporary paired win versus the wall arm.
- shapes-17 and triangle-20 no more than 1 mm median regression.
- Kill the 10-second program if this fails.

**Round 3**

- Median ≤160.
- At least one seed ≤155.
- Transfer holds.
- Missing 150.165 alone is not a kill; it is the owner’s next go/no-go after the engine family has proved itself.

The remaining disagreement I refuse to yield is narrow and explicit: no fatal random-throw gate, no constructor-as-floor-only start, no translation pre-snap, and no millimetre-scale legalization. Everything else has converged.
