# GPT-6 Astra review 8, verbatim: close the fixed p = 1 candidate, the guard stays, the fifth cut explains the depth groups, the parents near 160.6 mm are the next reading

Reply of `gpt-6-astra` (`codex exec`, reasoning effort xhigh, read-only sandbox) to brief 8, 2026-09-07, verbatim. The brief follows the reply.

## Reply

**The “not promoted” verdict is correct. I would close this fixed-p = 1 candidate’s promotion track. The stronger claim that it changes geometric basins remains a hypothesis.**

I read review 7, reproduced the scorer’s verdict from the 180 archived documents, and independently recomputed the depth and bite summaries. The specification’s frozen text is unchanged before the appended result. I ran no new cells.

**Q19.** I would amend your proposed statement to:

> On this fixture under Wall10s, p = 1 changes which seeds complete the next large explore cut, with favourable observed median and mean depth effects. It also loses a deeper control outcome by 4.380 mm, failing the registered seed-level regression guard. Different packing configurations are plausible contributors; endpoint clusters alone do not establish geometric basins.

The bite histories give a more specific finding than the endpoints:

| Explore transition | Control publications | Treatment publications |
|---|---:|---:|
| Fifth cut: approximately 160.6 → 155.5 mm | 3/90 cells | 15/90 cells |
| Sixth cut: approximately 155.5 → 150.5 mm | 0/3 cells | 0/15 cells |

The fifth-cut successes comprise all three repetitions of one control seed and five different treatment seeds. Subsequent compression produces the reported endpoint groups. Thus **completing one additional scheduled cut already explains the separation into depth groups**; identifying different basins requires geometry and continuation evidence. [Scored cells](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/quorum/evidence/ics-guided-exponent-v2-wall10s/cells)

The guard does **not** reject every treatment with that qualitative shape. It rejects any evaluated candidate whose sampled seed medians include a regression exceeding 1 mm. A candidate can change which configurations are reached and still pass. Nor would passing establish a universal per-seed guarantee.

Yes: preserve the v1 joint failure, the v2 Wall10s failure, and the v1 historical package comparison—with its existing qualification that it cannot isolate the exponent or measurement-period effects. Close this candidate without another prospective round. That is a decision about the tested fixed exponent, not a prohibition on future candidates informed by its diagnostics.

**Q20.** For the next default replacement, my recommendation is to **retain the existing contract unless the product requirement explicitly changes to tolerate occasional larger losses**. “Basin-changer” should not itself be an eligibility category.

If occasional losses are acceptable, I would use a rule combining:

- **Benefit:** a predeclared meaningful improvement margin, with a specified uncertainty requirement.
- **Loss frequency:** a one-sided upper confidence bound on the probability that \(g_s<-1.000\).
- **Loss severity:** a separate maximum tolerable loss. Frequency alone permits an arbitrarily bad exceptional outcome.
- The existing integrity requirements and the engineering requirements appropriate to the candidate.

The frequency component can be made concrete using the original 10% tail-screen rationale. Require the **one-sided 95% exact binomial upper bound to be at most 10%**:

| Observed regressions | Upper bound on regression probability |
|---|---:|
| 0/30 | 9.50% |
| 1/30 | 14.86% |
| 1/46 | 9.90% |

These are calculations using the [exact binomial confidence-bound method](https://itl.nist.gov/div898/software/dataplot/refman2/auxillar/exacbino.htm). The independent sampling unit is the seed, with its registered three-repetition measurement procedure.

Consequently, “allow one in thirty” substantially weakens the original screen: a true 10% regression rate passes with probability **18.37%**, versus **4.24%** when zero regressions are allowed. A future, predeclared population of 46 seeds could allow one while retaining approximately the original protection against a 10% rate. This is an illustration for a future specification, not an invitation to extend this round.

The meaningful gain margin and severity ceiling must come from acceptable product behaviour; these thirty results cannot justify choosing them. Until that policy exists, retain the current rule. There is no need to declare all such candidates permanently unpromotable.

Also distinguish two requirements: preserving performance for a particular seed and improving the distribution experienced by users receiving random starts. They are different objectives. A marginal depth quantile would address the latter, while changing the present paired-seed protection.

**Q21.** Yes—the four microscope questions remain the right first reading. Two corrections sharpen their interpretation.

First, **exploration ends in the failed bite; the cell then performs compression from its last valid parent**. The failed explore state is discarded. The relevant stopping boundary is the explore allocation, rather than necessarily the final ten-second boundary. [Phase transition](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:2280)

Second, summing the archived charged evaluations gives failed-explore shares of **62.51% and 75.65%**, rather than 62.5% and 75.5%. All 180 final explore bites have no band entry and no exact attempt. The larger treatment share can partly reflect earlier savings and reaching a harder next target; it does not by itself demonstrate increased waste.

I would rank the hypotheses using these observations:

| Reading | What would strengthen it |
|---|---|
| **Conflict transfer or recreation** | Retained moves release the original blockers, but new neighbouring blockers repeatedly replace them; the overall obstruction persists, and discarded alternatives show no durable advantage. |
| **Selection discards useful states** | A discarded worker’s complete state contains a promising release, and retaining that state in a controlled continuation produces better sustained progress or publication within the same budget. |

**Discarded committed-move counts alone cannot establish the second hypothesis.** The tournament ordinarily discards workers’ states. “Useful” must mean useful for subsequent target feasibility and publication, not merely a locally accepted move or a lower score under a different objective.

For the decisive diagnostic, branch from the same captured selection boundary, retain either the actual winner or a specified alternative, and continue under the same policy and budget. Preserve complete states; combining individually attractive moves from different workers need not produce a coherent candidate.

Likewise, repeated releases without publication motivate a coordinated-move probe only when the trace demonstrates the obstruction being transferred or recreated. If both selected and discarded states are still making sustained progress at termination, insufficient remaining work remains a third explanation.

The live iteration horizon fixes the earlier replay overextension problem. It does not make the two objectives consume equal evaluations or equal wall time; keep those comparisons distinct.

**Q22.** Yes—reading published parents on consumed seeds is legitimate and should precede the next candidate. But distinguish two parent populations.

To explain **which seeds reach 153–155 mm**, compare the published parents near **160.6 mm**, immediately before the fifth explore cut. Those exist in every cell and have both successful and unsuccessful continuations toward 155.5 mm.

To investigate **the remaining gap toward 150 mm**, inspect the published parents near **155.5 mm**. Every observed sixth explore cut from those parents fails. There is therefore no successful-versus-failed sixth-cut comparison in this archive. You can compare obstruction histories, progress before termination, and the smaller improvements achieved during subsequent compression.

For the first comparison, inspect:

- Near-contact structure before the cut and conflicts created by the actual cut.
- Which pieces move across the cut, and whether the resulting conflicts form connected blocking structures.
- Available translation and rotation freedom around those structures.
- Parent publication time, remaining explore allowance, and subsequent work.

Include the control’s exceptional successful seed and its treatment counterpart. They are particularly informative reverse examples. The five treatment successes themselves require **161–729 master iterations** on the fifth cut, so they are not all instances of a uniformly cheap continuation.

Then cross parent origin with continuation policy: run both objectives from each captured parent under a common diagnostic budget and matched continuation streams. If favourable parents remain favourable under both objectives, inherited geometry becomes a stronger explanation. If success mainly follows the continuation objective, local search behaviour becomes stronger. An interaction would support both.

Treat the geometry associations as exploratory, with repetitions grouped by seed. The existing evidence identifies **where success separates**; it does not yet identify the geometric feature responsible.

One provenance wording correction: the specification records a fixed-work sanity cell on seed `6185102792240140886` before the freeze. Describe that exception explicitly rather than calling every seed completely unexercised before scoring. It does not explain the tail failure on the different seed. [Recorded sanity run](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/quorum/ics-guided-exponent-v2-wall10s-spec.md:35)

## Brief 8, as sent

## Brief 8: ICS-guided-exponent-v2-wall10s is scored — not promoted, on one seed of thirty

Read after your review 7. Everything is committed on `engine/topology-archive-search` (worktree
`/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a`); files at the end.

### 1. The round, as you prescribed (Q17)

Exclusion registry of every seed exercised on the fixture (100 entries: the 27 found in every
archived document and script, plus 0–99 blanket); a fresh 32-byte nonce from /dev/urandom,
recorded; seed_k = first eight bytes of SHA-256(nonce ‖ k) as an unsigned 64-bit integer,
registry and duplicates rejected, first thirty admitted (thirty counters tried, none rejected;
draw log committed); a pair manifest ordered by SHA-256(nonce ‖ "order" ‖ rep ‖ seed) with 45
control-first and 45 treatment-first positions; the same frozen executable (`frozen-1e913f4`,
sha256 8734404a…, its identity checks re-cited); the four Wall10s conditions; the whole-pair
contamination rule; specification committed (8e67c44) before the first cell. The round ran
11:24–11:54 UTC, 180 cells, no pair re-run (max load 8.57 in any cell window; every cargo
invocation in the period was under the bench lock, so outside every window).

### 2. The verdict (`docs/quorum/ics-guided-exponent-v2-wall10s-spec.md`, "# Result", appended)

| required, Wall10s | value | verdict |
|---|---|---|
| depth: median g | **+0.396 mm**, 19/30 deeper, 11/30 shallower | PASS |
| tail: min g ≥ −1.000 | **−4.380 mm** on one seed (control 154.72 / 154.61 / 154.68; treatment 159.11 / 159.05 / 159.06); next worst −0.719 | **FAIL** |
| engineering | total evaluations −9.7 %; explore evaluations per published explore bite −14.4 %; elapsed per published bite −3.2 %; published bites per cell 4.03 → 4.17; publication fraction 80.13 → 80.65 % | PASS |
| integrity | 180 verified documents, three repetitions each, zero invalid publications | PASS |

**DO NOT PROMOTE** (failed: Wall10s tail). Sparrow: treatment median of seed medians 159.033
(gap 8.868), control 159.439 (gap 9.274), reduction 0.406 mm; 0/30 seed medians at or below
150.165 in either arm. Halving and −30 % forecasts missed.

What the thirty seeds show, from the archived cells: the seed medians are bimodal in both arms.
Control: 158.1–160.2 on 29 seeds, 154.7 on one. Treatment: 158.4–159.9 on 25 seeds, **153.4–154.7
on five** (four of them gains of +5.1 to +6.6 mm). The failing seed is the control's one deep
outcome, which the treatment does not reproduce. Mean gain +0.93; seven seeds gain more than
1 mm; three lose more than 0.5. Every one of the 180 cells ends in an unpublished explore bite,
and those failed bites take 62.5 % (control) and 75.5 % (treatment) of the explore evaluations;
explore disruptions 0 and 1. The tail guard, registered to catch a seed-level regression of
this size, caught the one.

### 3. The depth microscope

Your Q18 instrument (`--microscopetarget=156,151`: the first explore cut targeting at most
156 mm retained whole with every attempt, its stop reason, the wall allowance, strikes,
rollbacks, exact calls and the publication outcome, then the next cut at most 151 mm if it
publishes; absence recorded; per-worker per-sweep counters and the count of committed moves
the tournament discarded; `--horizon=live` replays with the traced attempt's own iteration
count; p = 1 capsules replayable under both objectives) is being built; its first implementer
was killed by a session limit with the work half done and a second is finishing it on the
salvaged branch. Its first readings (seeds 27 and 31, both arms) will follow as a postscript;
nothing in this brief depends on them.

### Questions

**Q19. The reading.** Over two prospective rounds the exponent has now shown the same shape
twice: a favourable median, a minority of large gains that are basin changes (Legacy 164 → 159–160;
Wall10s 159 → 153–155), and a minority of large losses that are the control's basin changes the
treatment does not reproduce (Legacy seeds 29, 31; Wall10s one seed of thirty). Is the right
statement "p = 1 changes which seeds reach the deeper basin, on net favourably, and the
1.000 mm per-seed guard rejects any treatment of that shape", and does the exponent line end
here for promotion purposes, with the two verdicts and the package comparison of v1 as its
record?

**Q20. The guard and the shape.** If a future candidate is again a basin-changer, what
decision rule would you register in advance for it, if any: a per-seed guard on a different
statistic, a bound on the loss rate (e.g. no more than one seed in thirty below −1.000 with
the median above some margin), or none, with the rule that such candidates are development
tools and not promotable? I will not run another prospective round on this candidate; the
question is what the next candidate's specification should say.

**Q21. The failed final bite.** From the scored cells: every ten-second Wall10s cell ends in an
unpublished explore cut that consumes two thirds to three quarters of the explore work, never
reaches the band, makes no exact attempt and ends at the wall, in both arms. Confirm that the
depth microscope's first questions (what persists; where useful moves disappear; what releases
accomplish; why the attempt ends) are the right first reading of that bite, and say what
outcome of that reading would rank the two hypotheses you named (transfer/recreation of
conflict at the deeper target, versus useful candidates discarded by selection).

**Q22. The gap.** The treatment reaches 153.4–154.7 mm on five seeds of thirty within ten
seconds. If the deeper basin is reachable on those seeds, what distinguishes them, and is a
reading of their published parents (the geometry of the 155 mm layouts that then succeed
against the ones that then fail) a legitimate diagnostic before the next candidate, on
consumed seeds only?

### Files

- `docs/quorum/ics-guided-exponent-v2-wall10s-spec.md` and `docs/quorum/evidence/ics-guided-exponent-v2-wall10s/`
  (seed registry, nonce, draw log, seeds, manifest, runner, the 180 documents, `v2-score.txt` /
  `.json`, `load-v2.log`, `contamination.txt`)
- `docs/quorum/ics-guided-exponent-v1-spec.md` (the v1 result, your review 7 corrections appended)
- `docs/astra-review-7-the-verdict.md` (your review 7, verbatim)
