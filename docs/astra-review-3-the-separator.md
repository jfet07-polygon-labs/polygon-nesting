# GPT-6 Astra review 3: after the margin, the separator

Consulted 2026-09-06 at `model_reasoning_effort = xhigh` on `e4069df`, on the owner's instruction
("ignora grok, usa codex astra xhigh"). The brief is reproduced after the reply. Verbatim;
nothing edited. Written before the Sparrow warm-start experiment
(`docs/experiments/overlap-ics/sparrow-warm-start/`) was run; that evidence goes to brief 4.

## Reply

**I agree with not opening `ICS-proxy-margin-v1`.** The margin has demonstrated a mechanism, not a reliable depth improvement. My `P_M ≥ P_R` preflight was a poor proxy for productive progress when publication counts can change within a bite. Preserve that failed precondition; do not amend it retrospectively or spend seeds 27–35 on it.

I reviewed `3b3474e2` without changing files or running solver cells. The local Sparrow source directories are empty in this environment; I could retrieve its pinned upstream separator and used the committed reading record for its worker and CD behavior.

**Q1. First correct three premises, then rank causes.**

1. **The loose-region mean comparison is closer to 17.2 versus 2.2, not 1.3.** Recounting the archived log over separating widths **175–182.976 mm** gives 45 successes: median **1**, mean **2.178**, nearest-rank p90 **3**, maximum **26**. None entered separation at zero loss. Your wider bin includes **157 separates above our starting width**, averaging 1.108 passes. Also, our `widthBeforeMm` is a parent width; Sparrow logs the target. Approximating its parent as `logged_width / 0.999` gives mean **2.20** in the common parent-width interval. The loose-region mean gap remains approximately **7.8×**. [Archived common-width prefix](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparrow-mixed61/log-10s-x86.txt:931).

2. **The reported minimum does not locate the plateau.** Our raw Φ is \(\sum_r v_r^2\), in mm². Therefore
   \[
   v_{\max}\leq\sqrt{\Phi}.
   \]
   At `5.9e-6`, every row is below **2.43 µm**; at `1e-5`, below **3.17 µm**. Those states are already inside the 4 µm band. The bite minimum includes its successful terminal state and all attempts. It cannot describe ninety preceding iterations outside the band. Record the current residuals during those iterations. [Fold](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:386), [bite aggregation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1962).

3. **Legacy normally disrupts after capped separations, without reaching the strike threshold.** Its default cap is **50 iterations per separation**. `WorkCap` enters the failed-separation pool/disruption path. Unless these cells explicitly used `--itercap=0`, “200 iterations → strike → disruption” is not their tail mechanism. Also, the strike meter observes a **master tournament**, charging all workers’ evaluations; `None` means no new raw minimum, not that every worker or relocate did nothing. [Resolved profile cap](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:3955), [cap exit](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1351), [observation classifier](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:2756).

My provisional ranking follows. These are hypotheses about the remaining separator cost, not explanations established by the supplied aggregates.

| Rank | Cause and code evidence | Single discriminating measurement |
|---|---|---|
| **1** | **Single-piece refinement stops while useful local moves remain.** Fine CD terminates on piece-relative translation and angular scales, not residual resolution. On the archived fixture, translation floors are **30–104 µm, median 60 µm**. A 0.05° rotation moves a witness 50 mm from the centroid approximately **44 µm**. [CD termination](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:582). | At every selected nonzero fine-CD exit, resume the same walk on a detached copy to 1 µm translation/witness resolution. Measure how often this clears the incident rows and makes the replayed sweep enter the band. Frequent cheap conversion supports resolution; negligible conversion kills it as the leading explanation. |
| **2** | **Conflicts are transferred to pieces whose turns are unavailable.** The order is collected once; a newly colliding piece is absent until the next sweep, and an already visited piece cannot be revisited. This can turn one seam correction into several sweeps. Sparrow’s recorded worker uses the same policy, so the possible difference is the amount of conflict transfer, not recollection semantics. [Our collection and traversal](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:694), [Sparrow reading](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/grok-review-12-reading-sparrow.md:32). | Attribute every end-of-sweep blocking row to its creating relocate and classify the other endpoint as **still scheduled / already visited / absent from initial order**. If most blockers are deferred births, replay an immediate-revisit queue at equal relocate work. Few such blockers kill the scheduling explanation. |
| **3** | **Different parent contact structure makes the same nominal bite harder.** Equal width and area fraction do not imply equal clearances, orientation freedom, seam-crossing contacts, or local mobility. Sparrow has already performed roughly 157 separates before entering our width range. Our constructor enters that range directly. | Measure the pre/post-bite contact graph: newly activated rows, cross-cut endpoints, near-legal neighbour degrees, and largest component. Compare first-sweep work conditional on this entry burden. A small entry burden followed by repeated failure on the same few rows demotes injection difficulty and promotes refinement/landscape. Establishing the *cross-engine* difference requires analogous Sparrow entry records. |
| **4** | **GLS amplifies a poor local tradeoff.** Our ranking uses \(wv^2\), while weight growth uses \(v/v_{\max}\). Matching Sparrow’s multipliers does not match its landscape. The cap is also reachable: a persistently maximal row doubles to \(2^{20}\) after **20 updates**, well inside a 50-iteration attempt. [Weight update](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:581). | Replay each selected stalled sweep from identical poses/streams with current weights versus unit weights. Compare band conversion at equal evaluation work. Weight saturation alone is not confirmation; materially better conversion under neutral weights is. |
| **5** | **Cap/retry policy amplifies the tail after a local failure.** It can explain a hundred-iteration *bite* composed of short failed attempts, but not the ordinary three-iteration body. | Attribute tail evaluations to first attempt, subsequent attempts, and disruption recovery, with actual `separationStop`. This directly confirms or kills the proposed 200-iteration-strike narrative. |
| **6** | **Band/authority mismatch.** It explained the removed churn. With margin 8, it is a weak candidate for pre-band work. On the same residual field, `max ≤ 4 µm` is less demanding than exact zero. Different geometry and collision quantification remain relevant, but “every row” is not itself a stricter stopping rule than zero loss. | Measure work before first band entry and after it, separately. If first entry normally certifies, the authority is excluded from the remaining approach cost. |

Two candidates can already be substantially rejected:

- **Winner-take-all versus eight whole separations:** that distinction does not exist. Sparrow reloads workers from the master, runs one `move_items` sweep each, and retains the lowest weighted-loss state. Its persistent thread pool differs from our per-tournament thread creation, which can affect **seconds per iteration**, not explain several iterations being logically necessary. [Pinned Sparrow separator](https://github.com/JeroenGar/sparrow/blob/14f4868fcd7e97036700dbebaf193fb159180aa9/src/optimizer/separator.rs), [our tournament](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:911).
- **Wrong-axis split:** the inspected implementation changes only `ty_mm` for centroids above the cut. Verify pose bits and row deltas; no speculative replacement is justified. [Split](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/homotopy.rs:219).

The CD floor is a termination scale, **not a quantization grid**. Large steps can clear tiny violations by overshooting into free space; individual axes can also fall below their floor while other axes remain active. Its numerical size establishes a testable mismatch, not the cause.

**Q2. The bite microscope.**

Use one preselected **seed-20 Legacy ten-second cell**, eight workers, margin **8**, achieved-depth publication off, existing sampling and schedule unchanged. Emit the resolved cap and all other knobs. Do not choose another seed because this cell lacks a tail.

I would add an opt-in census, provisionally `--bitemicroscope=1`, with these records:

| Record | Required fields | Capture point |
|---|---|---|
| **Cell identity** | `schemaVersion`, source/binary/fixture identities, seed, profile, workers, margin, band, resolved iteration cap, strike configuration, CD configuration | Benchmark setup |
| **Bite/attempt entry** | `bite`, `attempt`, phase, parent fingerprint and poses, parent/target widths, cut, moved-piece mask, initial rows and weights, stream coordinates | After cut/rebuild; again after pool installation/disruption |
| **Master iteration** | `iterationInAttempt`, global stream iteration, `rawPhiMm2`, `guidedPhiMm2`, positive-row count, above-band-row count, maximum violation and row ID; separate pair/L/R/B/T summaries | `separate`, before its band test at **mod.rs:1179** |
| **Worker summary—all eight** | Initial order; relocated, skipped and moved counts; ending raw/guided Φ, maximum violation, above-band count; evaluation/relocate deltas; winner ordinal | Worker return, before merge/GLS at **mod.rs:1025** |
| **Committed relocate** | Piece, order position, entry/final pose, origin, `ran`, `moved`, incident raw/guided before/after, incident maximum before/after, work by stage | Entry at **relocate.rs:706**; final rebuild at **:836** |
| **Committed row changes** | Stable row IDs; old/new violation; cleared, created, persisted, crossed-band status; contact normals/witnesses where available; endpoint scheduling status | Compare incident rows saved at relocate entry against its final committed rebuild |
| **Candidate selection** | Best raw and guided score by origin/stage; retained finalists; ordinary candidates with lower raw but worse guided score; equal-score acceptances | `evaluate`, `BestSamples::report`, CD comparison |
| **CD exit** | Stage, finalist, start/end score, actual `dxUm`, `dyUm`, `rotationDeg`, `rotationArcUm`, configured limits, pair-evaluation count, exit reason `limits/cap`; first-clear ordinal and evaluations after first clear | **relocate.rs:593–658** |
| **GLS and control transition** | Pre/post weight min/median/max, active-row statistics, cap count, maximum-row identity; `None/Marginal/Substantial`, patience count, snapshot/rollback IDs, actual stop reason | Before/after **energy.rs:581**; meter and stop sites |
| **Publication** | First-band ordinal, prefilter/refusal reason, exact-call delta, existing checkpoint ID, published depth, completion time | Existing publication path |

Three implementation details matter:

- **Measure committed row changes, not scratch-candidate changes.** `evaluate` installs every trial pose. Logging each temporary rebuild as a created conflict would manufacture enormous churn.
- **Separate `origin == StayPut` from `moved == false`.** A winner descended from the entry pose can move substantially through CD. Record `nonzeroUnmoved` and `nonzeroStayOrigin` separately.
- **Keep guided scores on the same weights when comparing moves/workers.** GLS can increase guided Φ without moving anything. Never call that a failed relocate.

For storage, retain compact iteration/all-worker summaries throughout the cell. Buffer each bite’s winning-worker move/CD records from its beginning; preserve the first three loose-region body bites and the first bite exceeding 33 iterations, using that selection rule fixed beforehand. Retain the tail’s complete attempt history. Explicitly report truncation and absent exposure.

At selected entries, preserve sufficient state for exact replay: poses, weights, target, descent counters, and original worker streams. Retain loser endpoint poses too; a winner-only trace cannot test whether selection discarded a band-reaching worker.

Perform expensive cold reconstruction and counterfactual probes **after the timed cell**, on detached copies:

1. Check cached rows, near sets and incident folds against cold reconstruction.
2. Replay finer CD.
3. Replay deferred-piece revisits at equal work.
4. Replay neutral weights.

A trace alone cannot causally distinguish all of Q1. **One cell plus its detached replays can distinguish the observed mechanisms**, provided it contains the relevant exposure. It cannot establish their prevalence across nine seeds.

Require fixed-work solver identity with census off/on after removing diagnostic fields. Buffer output until after the timed region; report diagnostic work separately. The microscope’s ten-second depth is not a treatment score.

One small discrepancy should also be recorded: margin-aware boundary residuals use `physical + m` and `T − m`, but `strip_sample_box` still samples against nominal boundaries. Measure margin-only boundary collisions by sample origin. This cannot explain the margin-zero gap and is not grounds for bundling a sampler change into the next treatment. [Sampling box](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:480), [boundary residuals](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/broad_phase.rs:58).

**Q3. My provisional mechanism: bounded fine-CD continuation at band-derived resolution.**

I would choose this ahead of changing sweep membership or GLS, **conditional on the microscope finding missed local descent**.

Proposed knob: **`--cdfinish=1`**, default zero and bit-identical off.

- Run the existing 75 samples, three coarse refinements and ordinary fine refinement.
- If fine refinement exits because of its limits with nonzero incident Φ, continue from its actual pose, steps, axis and stream position.
- Set translation limits to `min(old_limit, band/4)`—currently **1 µm**.
- Set the angular limit so the maximum source-vertex displacement about the centroid is at most 1 µm: approximately `min(old_angle_limit, (0.001/R).to_degrees())`.
- Allow at most **64 additional ± candidate pairs**. Stop on incident zero, the new limits, or that budget.
- Preserve the existing weighted comparison and accept-equal behavior. Charge every evaluation.

This spends additional work inside an already useful relocate, with the aim of avoiding another eight-worker tournament and another 75-sample pool for the same unresolved contact. It neither guarantees a local escape nor changes which positive-loss moves are acceptable.

It changes the frozen CD stopping policy and must be registered as a **new prospective treatment**. It does not trip §5.2: source rings, bite, distant container candidates, publication trigger, repair caps and both authorities remain intact. It is single-piece search, not joint PGS.

**Measure with margin 8 in both arms.** That makes successful band conversion productive and removes the already diagnosed certification lottery. It does not presume that margin 8 is independently a quality improvement.

My falsifiers on consumed seeds 18–26:

- On identically captured loose-region bite entries, compare control and continuation under identical fixed work limits, including failures. The **halving claim fails** unless the seed-balanced mean iteration cost falls by at least 50%, without reducing completion rate.
- If iterations fall but total evaluations or measured wall per completed bite do not, reject the engineering claim. Moving work inside CD must not win by changing the denominator.
- In subsequent paired ten-second cells, require a positive paired-median published-depth gain on **both profiles** before proposing an unseen-seed quality round. Report repetitions and all seeds; do not rescue a failed profile with the other.
- Any invalid publication or default-off trajectory mismatch stops the treatment.

If the detached continuation seldom clears rows, **do not build the live knob merely because the floor looks large**. The microscope should then promote conflict propagation or a sampled coordinate trap.

**Q4. Compress counts are descriptive, not evidence of wasted wall.**

Assuming the actual allocation is 80/20, your counts imply compression attempts per second are approximately:

- Legacy: \(4\times282/898 = \mathbf{1.26}\) times exploration.
- Wall10s: \(4\times326/44 = \mathbf{29.6}\) times exploration.

Wall10s exploration takes **3.2%** bites, versus compression’s **0.05% down to 0.001%**. Compression also changes the cut and discards failed children. Its much larger attempt count is therefore unsurprising. [Profile step](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:3944), [compression entry](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:2152).

Record actual phase wall, successful/failed/zero-iteration bites, and **published millimetres gained per phase-second**, charging failed attempts. Keep 80/20 unchanged now. Counts alone neither justify reallocating it nor identify a separator defect.

**Q5. Revised priorities.**

1. **Bite microscope plus missed-improvement replay census.** This is now the highest-value work. Include cold/cache consistency as its first check.
2. **The CD continuation above, if supported.** Otherwise promote the mechanism the replay actually identifies. Do not combine scheduling, weighting and CD changes.
3. **Bounded candidate evaluation.** Still worthwhile for seconds per iteration; it does not address iterations per bite. Preserve exact candidate decisions, equality handling and cache restoration.
4. **Margin: completed diagnostic infrastructure.** Keep it opt-in; use 8 for the controlled separator experiments. No standalone unseen-seed round now.
5. **Authority-discrepancy census: retain, narrow and demote.** Existing checkpoint instrumentation plus occasional cold verification is sufficient unless margin-on first-entry certification stops being reliable.
6. **Joint repair: defer.** With approximately one exact call per publication and almost no give-ups, publication repair cannot plausibly recover the missing separator factor.

The next evidence should identify **which committed move leaves the next iteration necessary**, and whether a cheaper continuation of that move removes that necessity.


## The brief, verbatim

# Brief 3 for GPT-6 Astra: the margin worked and did nothing; the bite is 3-13x Sparrow's at equal width

You are consulted at `model_reasoning_effort = xhigh` on branch `engine/topology-archive-search`
(worktree `/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a`, read-only), on the owner's
instruction ("ignora grok, usa codex astra xhigh ... prendendo ispirazione e forse facendo pure
meglio di sparrow"). Your first two reviews are committed verbatim at
`docs/astra-review-1-the-achieved-depth.md` and `docs/astra-review-2-the-proxy-margin.md`. Your
reply will be committed verbatim as review 3, before anything is done with it. Answer in
English; be as concrete as before (file:line, numbers, falsifiers). Standing constraints are
unchanged: Sparrow may be read (`/var/lib/t3/tmp/sparrow-bench`, the reading record
`docs/grok-review-12-reading-sparrow.md`) and cited, never ported verbatim; no polygon
simplification or pole proxy; the 5.0/5.0 contract and `validate_placements_against_contract`
untouched; quality is the published raw-source depth of a dual-valid layout; the forbidden-rescue
table (`docs/grok-review-12-reading-sparrow.md` §5.2) binds; seeds 27-35 are the unseen population
and no treatment has touched them.

## 1. What happened to your review 2

The proxy margin was implemented exactly as specified (commit "The proxy margin: the search
targets strictly inside the Exclusive kernel's region, behind --proxymargin"; one shared
`energy::proxy_pair_clearance_mm` feeding `rebuild_all`, `rebuild_piece_rows`, `binary_close`
and the corpus oracle; four-sided `boundary_residuals`; default 0 bit-identical; adversarially
verified: no rescue row, nothing copied). Measured on dev seeds 18-26, one repetition, both
profiles, `--proxymargin=4` (`docs/experiments/overlap-ics/proxy-margin/README.md`):

| profile | give-ups | exact checkpoints / publication | publications | explore bites / cell | invalid |
|---|---|---|---|---|---|
| Legacy  | 1535 -> 3 | 2.84 -> 1.00 | 1042 -> 990 | 98.8 -> 100.7 | 0 |
| Wall10s |  737 -> 1 | 4.35 -> 1.01 |  266 -> 291 |   3.9 -> 3.8 | 0 |

Depth: paired median -0.181 mm (Legacy) and -0.076 mm (Wall10s), 4/9 wins each, per-seed spread
-4.7..+6.2 mm. Quick A/B at m = 8 on seeds 18-20: give-ups 588 -> 0 and 169 -> 0; depth +0.62
(3/3) Legacy, -0.22 (1/3) Wall10s. Work counters (nine Legacy cells summed): sample evaluations
393.0 M -> 415.4 M, relocates 1.548 M -> 1.648 M, master iterations 24 253 -> 18 108, relocates
per master iteration 64 -> 91, repair rows 4756 -> 37, disruptions 304 -> 72.

So: your Q1 reading of the autopsy stands, the churn was real, the margin removes it to the last
give-up, and it was about five per cent of the wall. The freed time is spent again on the rows
the margin activates and the bite rate does not move. I did **not** open `ICS-proxy-margin-v1`:
its preflight `P_M >= P_R` on every seed already fails on six Legacy dev seeds (a bite that
certifies at its first exact call publishes once where the old path sometimes published twice),
and its depth clauses have no dev evidence behind them. The virgin seeds stay virgin. Please say
if you disagree with not opening it.

## 2. The number that matters: iterations per bite at equal width, ours vs Sparrow

Both engines bite the same way (split-and-close at mid-depth, 0.1 % of the width,
`homotopy.rs::split_and_close`, cited from `separator.rs::change_strip_width`). Sparrow's full
ten-second log on this fixture is archived at `docs/experiments/sparrow-mixed61/log-10s-x86.txt`
(seed 0, 8 workers, `--min-item-separation 5`, x86): 351 separator returns, every one at
`min_l: 0`, no failure before 8 s. I paired each `(S)` return with the preceding "separating at
width" line and binned by width. Ours are the nine Legacy dev cells (bite records in the
benchmark JSON, `outcome.bites[].masterIterations`, `widthBeforeMm`). "Passes" for Sparrow is
`i + 1`; a master iteration of ours is about 1.2 worker sweeps.

| width band | Sparrow separates | passes median / mean / p90 / max | our bites | master iterations median / mean / p90 / max |
|---|---|---|---|---|
| [175, 215) | 202 | 1.0 / 1.3 / 2 / 26   | 405 | 3 / 17.2 / 18 / 1145 |
| [170, 175) |  29 | 2.0 / 3.1 / 6 / 12   | 261 | 4 / 13.3 / 23 / 269  |
| [165, 170) |  30 | 6.5 / 11.5 / 26 / 49 | 225 | 14 / 32.6 / 95 / 437 |
| [160, 165) |  30 | 9.5 / 19.4 / 41 / 145 | 7  | 11 / 44.3 / 137 / 137 |
| [155, 160) |  32 | 8.5 / 37.0 / 76 / 406 | -  | - |
| [150, 155) |  28 | 10.5 / 40.7 / 189 / 241 | - | - |

Our constructor starts at 182.976, Sparrow's LBF at 214.027; Sparrow crosses our whole
ten-second range (183 -> 165) in about 1.5 s with ~100 bites, then spends 5 s from 158 to
150.6. At equal width our median bite costs 2-3x Sparrow's passes and our mean 3-13x; our
iteration rate is 270/s against its 460/s (ours 16 K sample evaluations per master iteration =
64 relocates x 254; Sparrow 8 K = ~31 moves x 263). Product: about 6x wall time per unit of
shrink at equal width. Sparrow has the same heavy tail shape as we do (2.3 % of its separates
above 100 passes take 54 % of its passes; ours 4.1 % take 52.5 %), but its tail starts at 158 mm
and ours at 183.

Tail anatomy on our side (Legacy, bites above 33 master iterations):

- devbase: 86 bites, mean 144 iterations; band reached in 84; **24.2 band entries and 7.2 exact
  calls per bite**; attempts > 0 in 69, disruptions in 69. This was the churn.
- margin m = 4: 65 bites, mean 90 iterations; band reached in 62; **1.0 band entry and 1.0 exact
  call per bite**, published at that call in 62. All ninety iterations are spent *before* the
  band: `minRawPhi` median 5.9e-6 (zero in 14 of 65), the strike meter counts 3400 "none"
  batches against 2336 "substantial" (a batch is a worker's charged work unit; "none" = no
  improvement of the best Phi), attempts > 0 in 26 (200 iterations without improvement ->
  strike -> disruption -> separate again).
- The loose-region body (width >= 175, 2-10 iterations, 229-242 bites): 1.0-1.2 exact calls,
  no attempts. These bites publish at their first exact call after 2-10 iterations where Sparrow
  needs one pass.

So after the margin the remaining cost is the separator itself: (a) the typical bite needs
three of our iterations where Sparrow needs one pass, at a width where the layout is loose
(density ~55 %), and (b) one bite in fifteen stalls with Phi ~ 1e-5 for a hundred iterations,
then a strike swaps two large pieces and the bite starts over.

## 3. What one master iteration of ours does (my reading; verify against the code)

`overlap_ics/mod.rs` (the explore loop and the barrier), `relocate.rs` (the sampler and the
coordinate descent), `energy.rs` (rows, GLS weights), `descent.rs`, `broad_phase.rs`. The
relocate is our port of Sparrow's `move_items`, cited: 25 focused samples uniform in the piece's
own AABB (16 orientations if rotatable), 50 container samples, best by incident weighted Phi,
then two-stage axis coordinate descent with Sparrow's ratios (pre-refine 0.25 -> 0.02 x min-dim
and 5 -> 1 degrees; final 0.01 -> 0.001 x min-dim, 0.5 -> 0.05 degrees), wiggle about the
transformed centroid, accept-equal. 8 workers, barrier, winner-take-all. GLS weights updated
per master iteration (`weightUpdates` ~ master iterations), cleared on a successful bite. Band
4 um on max proxy violation triggers `publish::attempt`; explore strikes 200 iterations without
improvement x 3, then disruption (swap two large pieces, Sparrow's `disrupt_solution`).

Note the final coordinate-descent floor: 0.001 x min-dim is 50-100 um for these pieces, an
order of magnitude above the 4 um band. Sparrow has the same floor and the same exact-zero
requirement on its (simplified, slightly inflated) polygons, and still closes the loose-region
bite in one pass. I do not know why ours does not, and I would rather measure than guess.

## 4. Questions

**Q1. Diagnosis.** From the code and the numbers above, what is your ranked list of causes for
"3 iterations median, 17 mean, where Sparrow needs 1.3 passes at the same width and density"?
Candidates I can see, not exhaustive: (i) which pieces a worker sweep visits and in what order
(a snapshot of violating pieces at sweep start versus re-collecting after each move); (ii) the
winner-take-all merge discarding seven workers' progress each iteration, versus Sparrow's
workers each running a full separate and only the best snapshot surviving; (iii) the
coordinate-descent floor and accept rule leaving 5-50 um residuals that need further sweeps;
(iv) GLS weights ramping too fast or too slow so the guided landscape misleads on a loose
layout; (v) the band entry condition (max violation <= 4 um on **every** row) versus Sparrow's
loss == 0 which its quantifier computes on inflated simplified polygons; (vi) something in the
bite itself (the split at mid-depth on our long axis; pieces straddling the split). For each,
say what single measurement would confirm or kill it.

**Q2. The instrument.** Specify a *bite microscope*: an always-on or census-gated per-iteration
trace inside one bite (violating rows count and max, pieces relocated by the winning worker,
rows cleared and rows created by each relocate, stay-put winners with nonzero rows, Phi raw and
guided, weight statistics, CD step at exit) that would decide Q1 in one ten-second cell on a
consumed seed. Name the fields and where they are captured. This is the next workflow run.

**Q3. The mechanism, provisionally.** If you had to name the single mechanism most likely to
halve iterations per bite in the loose region without tripping a rescue row, what is it, how is
it switched by an opt-in knob, and what is its falsifier on seeds 18-26? Also say whether it
should be measured with the margin on (m = 8) or off.

**Q4. Compress.** We spend 20 % of the wall in the compress phase (Sparrow's 80/20). At Legacy
our compress bites are 282 against 898 explore bites, at Wall10s 326 against 44. Is that
ratio evidence of anything, or noise to ignore for now?

**Q5. Priorities.** Re-rank your Q5 list from review 2 in the light of §2: margin (done, neutral
on depth), bounded evaluation, the authority-discrepancy census, the missed-improvement census,
joint repair, and whatever Q3 names.
