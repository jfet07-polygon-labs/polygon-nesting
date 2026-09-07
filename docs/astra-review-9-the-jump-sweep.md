# GPT-6 Astra review 9, verbatim: the ordering accepted, the jump-sweep branch as the next diagnostic, the rollback-weight probe admitted, common parents for development

Reply of `gpt-6-astra` (`codex exec`, reasoning effort xhigh, read-only sandbox) to brief 9, 2026-09-07, verbatim. The brief follows the reply.

## Reply

**I accept the descriptive ordering. I would make the jump-sweep branch the next diagnostic, and I would also admit a separate rollback-weight probe.** Two qualifications matter: selection can *cause* conflict recreation, so those explanations are not mutually exclusive; and late progress supports insufficient work without establishing that more work would finish the cut.

I read review 8 and the new section, checked the selection and rollback implementation, verified all 142 archive hashes, and independently recomputed the central fifth-cut summaries. I ran no new search cells.

**Q23. The ranking.**

Transfer/recreation is the strongest directly observed finding: original conflicts disappear, conflicts re-form under changing row identities, and spanning structures recur in the retained minima. That establishes the search’s observed behaviour. A spanning conflict graph alone does not establish a mechanically locked configuration or an infeasible target.

I would retain the deadline classification with more restricted wording:

- **Two:** late progress consistent with insufficient remaining work.
- **Three:** unresolved.
- **A31:** contradicts the specific account “it was still converging and just needed its final few iterations.”
- **Twenty-eight:** no observed evidence of approaching completion before termination.

Thus, **“supported in two of thirty-four” is justified; “explains two of thirty-four” is premature.** Neither a late raw minimum nor comparison with successful attempts’ completion times establishes the missing continuation.

The loser endpoints do count against a **narrow selection hypothesis**: the tournament repeatedly throws away already feasible or nearly feasible candidates and thereby misses publication. No such candidate appears in the failed attempts.

They provide much weaker evidence against the hypothesis we actually left open: **a discarded configuration would develop better under continuation.** The approximately 5% comparison concerns attempt-best maximum residuals, potentially reached at different sweeps, summarized across attempts. It measures neither geometric similarity nor continuation potential. The fourth criterion therefore remains untested for durability; the endpoint finding does not satisfy it.

One numerical wording correction: **43/50 and 29.5/47 refer to entry pieces remaining anywhere in the blocking graph**, not retention by the largest component. The largest entry and handed-back components share medians of 20 and 12 pieces. [Persistence reading](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/bite-microscope/README.md:498)

**Q24. The jump sweep is the right first selection boundary.**

Yes. Define it mechanically as the first sweep whose actual winner has raw above twice the previous running minimum. Capture **after every worker finishes and before selection and the master’s weight update**. Do not require a metre-scale move: three of the 42 jumps have maximum relocation below 25 mm.

Two corrections constrain its interpretation:

1. **The jump occurs in all eight published fifth cuts too.** Moreover, seven of twenty failed control attempts subsequently improve their pre-jump minimum; the control publication does so as well. “The control never beats its early minimum” applies to the thirteen early-plateau failures, not the entire control.
2. **The raw incumbent survives in the snapshot.** Selection abandons an active continuation, but does not erase the saved best geometry. That distinction connects this question directly to rollback.

The experiment should preserve:

- Every worker’s coherent geometry, rows, weights, neighbourhood structures and descent state.
- The shared master context: saved minimum, strike/patience history, iteration and stream coordinates, publication context, and work already spent.
- The exact point at which installation and the single weight update occur.

Branch to the actual winner and to alternatives chosen by rules fixed before continuation—for example, minimum raw and lexicographic `(maximum residual, raw, worker ordinal)`. If selectors choose the same worker, record that duplication. Resume the existing policy after this **single changed selection**, with matched future stream keys and identical remaining all-worker evaluation caps. Preserve ordinary early stopping and certification. First require the actual-winner branch to reproduce the archived continuation.

Do not choose retrospectively whichever loser’s continuation succeeds. That would test an oracle or a separately budgeted portfolio.

**How many? There is no defensible universal “x of 42” threshold for ranking these two explanations.** The useful distinctions are:

| Claim | Required evidence |
|---|---|
| Selection prevented completion on a particular recorded continuation | One correctly reproduced paired rescue: alternative reaches the band while the actual winner does not, within equal work. Certification establishes a publication rescue. |
| Selection deserves priority as a development target | Repeated rescues across distinct parent seeds under the same specified selector, with a favourable balance against lost publications and consistent results under preassigned continuation streams. |
| This intervention rescues most recorded fifth-cut failures | At least **18 of the 34 failures**—a literal cohort majority, not a statistical or promotion threshold. |

I would advance a candidate before demanding a majority, if rescues repeat across parents and the successful controls show no comparable damage. Report **rescues among 34 failures and losses among eight publications**, separately by origin/objective. The 42 arm-parent cases are not 42 independently sampled seeds, and this cohort was partly selected by outcome.

A rescue would establish selection as an actionable upstream contributor to the obstruction on that continuation. It would not refute the observation that conflicts were being recreated.

**Q25. What the pre-update ranking must show.**

First, **pre-update does not mean stale or incorrectly timed**. The implementation compares workers under their inherited weights, installs the selected state, then updates its weights once. That is a coherent common scoring landscape. [Tournament implementation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1080)

At the jump, the microscope should explain the actual preference:

- Which previously weighted rows the winner relieves.
- Which rows it creates or enlarges, including boundary rows.
- How those contributions sum to its lower guided total despite higher raw or maximum residual.
- Which worker each proposed replacement score would select.

This requires row-level residuals and weights, not another disagreement percentage. It should reproduce the logged totals under the same exponent and tie rule.

For **lexicographic `(max residual, raw)`**, repeated successful branches from the state that this exact rule selects are sufficient to justify a development candidate. A min-raw rescue alone does not validate a different selector.

For **post-update guided total**, define the proposed score precisely. Updating each worker’s weights on its own residuals produces eight candidate-dependent weight landscapes. Scoring every worker under one common updated vector is a different policy. Neither is simply a fresher reading of the existing objective. For the first interpretation, compute each hypothetical update without mutation, select, and install exactly one updated state. [Weight-update rule](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:659)

The jump branch is sufficient to motivate a **boundary intervention**. It does not establish that applying the replacement ranking at every sweep is beneficial. That requires a separate continuation comparison using the actual proposed policy throughout.

Its operational falsifiers are:

- The proposed score selects the actual winner at the implicated boundary.
- Its alternative’s initial advantage disappears, with no sustained feasibility or publication gain within equal work.
- It suppresses successful continuations sufficiently to defeat the claimed benefit.

A null result closes the tested selector at this boundary and horizon. It does not close every selection hypothesis; other workers or later decisions remain different interventions.

**Q26. The rollback probe is justified now.**

I would not require the selection question to close first. The immediate rebound after restoration supplies a concrete reason to inspect the interaction between restored geometry and accumulated weights.

The implementation explicitly **restores geometry while keeping current weights**. It also restores the corresponding rows and neighbourhood structures. Thus the retry is not a return to the complete state that originally produced the minimum. [Rollback implementation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:2695)

Start with a clean comparison at an existing rollback:

- Restore the same argmin geometry with current weights.
- Restore it at the same time with weights reset to the documented floor.

Keep the same remaining budget, stream coordinates, selection policy and meter transition. That isolates weight reset. Earlier rollback is a separate timing intervention; choose its trigger from information available then, rather than retrospectively identifying the attempt’s final minimum. Neither intervention receives a fresh outer budget.

The falsifier is straightforward: reset or earlier restoration repeatedly recreates the same obstruction without improving completion, or removes pressure needed by successful continuations.

I would also replace **“wasted 60%”** with **“60% spent after the final raw minimum.”** The record establishes that expenditure, but not that it could all have been removed or profitably reassigned. Weight learning and geometric exploration can matter without producing an immediate raw record.

**Q27. Use common parents for development; retain the prospective round.**

Yes. This makes the common-parent development instrument more important.

I would soften two causal claims, however. The findings establish that **same-seed arms do not provide the same geometric start**; unequal fingerprints and low row overlap do not establish independence or the absence of inherited structure. Likewise, the crossed results suggest upstream contributions and an interaction, but the unequal work horizons prevent assigning how much of the exponent’s effect belongs to parent creation versus continuation.

For development:

- Freeze a parent population containing both origins, including the exceptional control parent and the failed continuations.
- Give both continuations each identical parent/cut state, target, paired continuation streams and equal evaluation budgets.
- Use common, predeclared budget checkpoints rather than each origin’s live iteration horizon.
- Report origin-specific rescues, losses, work to band and certified publication. Group repeated observations by original seed.

This estimates continuation effects conditional on the captured parents. It does not estimate their prevalence in ordinary runs, because the archive includes outcome-selected seeds. Comparing origins additionally needs common budgets and continuation-stream assignments so inherited RNG context does not silently become “geometry.”

The prospective test should remain the native **whole-run Wall10s comparison under the existing guard**. It must include the candidate’s upstream effects, parent timing, search costs and downstream compression. Common-parent diagnostics explain and develop a new candidate; they neither replace that test nor reopen the closed fixed-\(p=1\) promotion track.

## Brief 9, as sent

## Brief 9: the failed fifth cut under the depth microscope

Read after your review 8. The instrument you specified in Q18 / Q21 is built (commit "The depth
microscope", `--microscopetarget=<mm>[,<mm>]`, the whole attempt to its live stop, per-worker
economics, `--horizon=live`, `deep-cut.py`; bit-identical, 867 tests) and was run on consumed
seeds only: 27–35 and twelve v2 seeds (the five treatment fifth-cut successes, the control's one,
six failures in both arms), both arms, Wall10s, margin 8, `--microscopetarget=156,151`: 42
documents, 50 retained attempts, and 100 replays of every retained entry capsule under both
objectives at the live horizon with `--certify=1`. The microscope reproduces the scored v2
outcomes on the twelve shared seeds in 24 of 24 arm-seed pairs. The reading is
`docs/experiments/overlap-ics/bite-microscope/README.md`, section "The failed fifth cut under the
depth microscope (review 8, Q21-Q22)", organised by your four questions, with every number
recomputed by an independent verification from the documents (`deep/analysis/verify-*.py`) and
fifteen text corrections applied before you see it. The documents (2.4 GB) stay on the host under
`/var/lib/t3/tmp/astra/deep/`, identified by sha256 in `deep/archive-manifest.txt`; seeds 27 and 31
are committed gzipped.

### What the reading says, in your three readings' order

1. **Conflict transfer or recreation: shown**, on your first three criteria. Every entry row is
   released within a few sweeps in both arms (none continuously present at sweep 100 in the
   control, at sweep 20 in the treatment); 39 of 46 and 40 of 41 entry rows re-form at least once;
   what persists is structure under new rows: a bottom-to-top spanning blocking component is
   present at the handed-back state of every failed attempt (20/20, 14/14, 7/7) and absent at
   every published stop, the largest component keeps 43 of 50 (A) and 29.5 of 47 (B) of the entry's
   pieces, and the most persistent row is a created boundary row against the bottom or top edge in
   18 of 20 (A) and 12 of 14 (B). The fourth criterion (discarded alternatives show no durable
   advantage) holds at the endpoint only: no discarded worker was ever within 50 µm of the band in
   a failed attempt, the best loser state is within 5 % of the best retained one.
2. **Insufficient remaining work: the margin, not the class.** Of the 34 deadline stops, 2 are
   still improving at the deadline, 3 are undecidable, 1 is contradicted (raw 0.195 mm² at
   iteration 386, then 196 iterations without a new minimum), 28 never got below 8 mm². Every
   failed attempt ends at the phase deadline holding its argmin-raw state (42 of 42); the control
   reaches it at iteration 5–10 in 13 of 20 and spends 60 % of its iterations and 64 % of its
   evaluations after it.
3. **Selection discards useful states: open, by construction.** The tournament discards seven
   eighths of committed work in every attempt, "useful" is operationally "moved", and the
   raw-versus-guided disagreement (the winner is the min-raw worker on about a third of sweeps,
   the min-max worker on a quarter) is identical in published and failed attempts, both arms,
   every quartile. The losers' poses are not in the documents.

Two findings you did not ask for and which the record carries with the verifier's numbers:

- **The chain of minima breaks the same way in all 42 attempts.** The winner's raw falls for the
  first 4–10 iterations, then at iteration 5–11 jumps above twice the running minimum on a sweep
  whose winner made a container relocation of 1.2 m at the median (1.23 m A, 1.20 m B; the sweeps
  before have a 7–9 mm median), while another worker still held a state below twice the running
  minimum in 39 of 42 attempts. The documents do not say why the pre-update guided objective
  preferred that state. After it the control never beats its early minimum; the treatment's chain
  continues slowly through its rollbacks (restore to argmin raw, 200 iterations after the last new
  minimum, 78 of 83 rollbacks exactly) to iteration 609 at the median.
- **Nothing measured at entry separates the continuations that publish from those that fail**
  (parent time, allowance, rows, residuals, components, spanning structure), and the same seed's
  two parents are unrelated layouts (placement fingerprints equal on 0/21 seeds at bite 4; entry-row
  Jaccard 0.05 within seed equals 0.05–0.06 across seeds): parent origin is arm-made, the seed
  carries no geometry into the fifth cut. Under both objectives the treatment-made parents enter
  the band more often (7/21 live and 5/21 under p = 2 with 0.74–3.19× the live evaluations, against
  1/21 and 1/21 from control-made parents), an interaction on a sample too small to rank and
  confounded by the horizon asymmetry (a p = 2 iteration costs 2.20 p = 1 iterations, not the
  three to four I wrote; per second the arms burn evaluations at nearly the same rate).

The eight sixth cuts (155.5 → 150.5 mm) all fail far from the band (best max violation 0.9–4.8 mm);
there is no successful sixth-cut comparison, as you anticipated.

### Questions

**Q23. The ranking.** Do you accept the ranking above (transfer/recreation shown on three of four
criteria; insufficient work explains two of thirty-four; selection open), and is the fourth
criterion, "discarded alternatives show no durable advantage", now decidable only by the branching
diagnostic, or does the endpoint reading (no loser within 50 µm of the band, best loser within 5 %
of the winner) already count against the selection hypothesis?

**Q24. The jump sweep as the selection boundary.** The record points at a specific boundary: the
sweep at iteration 5–11 where the tournament retains a worker whose winning sweep contains a
1.2 m container relocation and whose raw is above twice the running minimum, while a loser holds
a state below it in 39 of 42 attempts. Is that the boundary for your decisive diagnostic (capture
every worker's complete state at that sweep; branch retaining the actual winner or the min-raw /
min-max loser; continue complete states under the same policy and budget), and what outcome would
rank selection above transfer: the loser's branch reaching the band where the winner's does not,
within the same evaluations, on how many of the 42?

**Q25. The pre-update ranking.** The tournament ranks on the pre-update guided total, which
disagrees with raw and with the maximum residual on two thirds of sweeps in both arms and both
outcomes. I am not proposing a change; I am asking what the microscope must show before "rank the
tournament on (max residual, raw) lexicographically" or "rank on the post-update guided total"
becomes a candidate: is the branching diagnostic at the jump sweep sufficient, and what is its
falsifier?

**Q26. The rollback and the wasted 60 %.** The control hands back a state it reached at iteration
5–10 and spends the remaining 60 % of its iterations without improving it; the rollback restores
that state only 200 iterations later, and the next sweep stands at 13.9 mm against 1.3 mm restored
on 14 of 18 rollbacks. Is "the retry from the argmin state with fresh weights (or an earlier
rollback)" a candidate worth a microscope, or is it another schedule change you would rule out
until the selection question is closed?

**Q27. The parents.** Since parent origin is arm-made and nothing at entry separates success
from failure, the exponent's effect on the fifth cut is partly upstream (the parents it makes)
and partly the continuation, confounded. Does this change how the next candidate should be
evaluated on the deep cut: a common parent population (the same captured parents for both
continuations, as the replays do) as the development instrument, with the prospective round
kept as it is?

### Files

- `docs/experiments/overlap-ics/bite-microscope/README.md` (the section), `deep/` (manifest,
  `deep-cut-output.txt`, runners, `analysis/` scripts including the `verify-*.py` recomputations,
  seeds 27/31 documents gzipped)
- `crates/polygon-nesting-core/src/search/overlap_ics/microscope.rs` (`--microscopetarget`),
  `replay.rs` (`--horizon=live`), `docs/experiments/overlap-ics/bite-microscope/deep-cut.py`
- `/var/lib/t3/tmp/astra/deep/` (the 42 documents and 100 replays, on the host)
- `docs/astra-review-8-closing-the-exponent.md` (your review 8, verbatim)
