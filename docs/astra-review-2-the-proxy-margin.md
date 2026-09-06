# GPT-6 Astra review 2: the proxy margin

Consulted 2026-09-06 at `model_reasoning_effort = xhigh` on `c35b1f6`, on the owner's instruction
("ignora grok, usa codex astra xhigh"). The first invocation died with the session before it
produced a line; this is the second, on the same brief. The brief is reproduced after the reply.
Verbatim; nothing edited.

## Reply

**Q1. The mismatch is real; the “one-cell stricter” explanation is incomplete.** The [kernel accepts equality](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:540): canonical distance exactly 5000 µm passes. `critical_two_r_micron` reports the largest accepted **integer** threshold. A distance of 4999.99 therefore reports 4999, producing a diagnostic shortfall of 1 µm despite only 0.01 µm geometric deficiency.

Nearest vertex rounding can change pair distance by up to √2 µm; integer reporting adds less than another micrometre. Thus your maximum **discrepancy** of 1.97 µm fits rounding plus reporting. The 2–4 µm **shortfall** tail also contains actual proxy violation; it does not establish another defect.

Distinguish these by recording, on identical pre-repair poses: cached violation, independently recomputed continuous source distance, canonical distance before integer truncation, and critical threshold. Cold/cache disagreement identifies implementation error; continuous/canonical disagreement beyond √2 µm identifies something beyond ordinary rounding. First/last scans alone do not prove the intervening cascade; trace each correction and newly failing neighbour.

**Q2. Promote margin to the leading experiment, but use 8 µm.** With entry band \(b=4\), margin guarantees only continuous clearance \(5000+m-b\) µm at checkpoint entry. Consequently:

- **m=4:** band entry can still land exactly at 5000.
- **m=8:** band entry retains 4 µm continuous slack, exceeding the √2 µm rounding bound.
- **m=6:** sufficient for initial pair legality under verified geometry correspondence, but leaves less reserve.

Eight is therefore a conservative \(b+\text{guard}\) choice, not a claim that every repair cascade is bounded. Also, 5004 continuous clearance guarantees a critical threshold of at least **5002**, not 5003.

No §5.2 row is violated if source rings remain unchanged and margin modifies residual thresholds only. Explicitly distinguish this from the table’s forbidden ring inflation. Publication and measured quality remain unchanged.

Depth cost depends on geometry: a vertical chain of 61 pieces has 60 gaps, costing **0.240/0.480 mm** at 4/8 µm; bottom/top reserves add 0.008/0.016 mm of required strip capacity. This is an illustration, not a general bound. “Under 0.1 mm” needs measured contact structure. Pair-only margin saves the boundary reserves but preserves top-gate churn and exposed boundary contacts; defer that ablation.

**Q3. Pair repair is already two-sided.** [It splits the correction](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:1235) between both endpoints subject to sheet slack and displacement caps. A 5 µm correction is ordinarily 2.5 µm each.

The missing protection is against other neighbours. Moreover, repair uses pre-repair cached normals, falling back to centroid separation when no normal exists; that direction need not clear a concave contact. A bounded joint correction with refreshed witnesses and constraints protecting nearby legal pairs deserves a separate experiment if cascades survive margin. Margin has not yet made repair irrelevant.

**Q4. Final amendment text:**

> **ICS-proxy-margin-v1**, a prospective successor to `ICS-achieved-depth-v1`. Preserve the failed H1 specification and result unchanged. Incorporate its normative text by reference, subject only to the following replacements.
>
> Replace H-L/H-W with M-L/M-W throughout, including population tables, comparisons and tie-breaking. Retain O-L/O-W at `d0c459b`. R represents corrected `6aaa4e1` behavior with the common autopsy instrumentation; require fixed-work solver identity against that corrected base. R and M share one frozen instrumented binary and differ only in margin. Set `--publishachieved=0` throughout.
>
> M uses **`--proxymargin=8`**, in micrometres; R uses zero. Add margin to existing proxy pair and physical-edge requirements, and use proxy ceiling `T−m`. Preserve sag conventions. Nominal shrink target and strict-target publication remain unchanged. Apply identical thresholds to full and incremental reconstruction, worker copies, near sets and rollback. Changing only `rebuild_piece_rows` is insufficient. Shared authority-contract values must remain unchanged.
>
> Publication, kernel, validator, raw-source depth, 4 µm band, repair algorithm, guard, caps and row budget remain frozen. Bounded evaluation, alternative margins, pair-only treatment and H1 combinations are excluded.
>
> Retain preflights 1–4, including H1 regression coverage for the dormant knob; S0’s original zero-Φ pin applies at margin zero. Add margin-on cold/incremental consistency and pair/boundary threshold vectors.
>
> Replace preflight 5 with paired fresh ten-second R-L/M-L processes on consumed seeds **0–8**, alternating order by seed parity. Define G as give-ups whose blocking shortfall exceeds 4 µm; distinguish other refusal causes. Define A as all search exact checkpoints, including unsuccessful and late attempts; P counts improving non-constructor publications certified by the deadline. For **every seed**, require `G_M ≤ floor(G_R/2)`, `P_M ≥ P_R`, `P_M > 0`, and `A_M/P_M < 1.5`. At least six controls must have `G_R > 0`; otherwise exposure is insufficient. Failure stops scoring. Record original-clearance measurements separately from margin residuals, first-scan failures, repair displacement, and iterations per publication.
>
> Retain seeds **27–35**, all **954 processes**, fixtures, walls, repetitions, ordering, clock, revalidation, control envelope, statistics, and clauses **1–14**. Clause 12’s parent-installation requirement now applies to M. Retain both PASS labels and their thresholds; ties select M-L. Preserve the documented prior O-only seed exposure; these seeds are treatment-unseen, not literally virgin.
>
> Freeze executable/compiler/fixture identities and commands before scoring. All original refusal and immutability provisions remain binding.

**Q5.** Margin moves above bounded evaluation; H1’s standalone priority collapses. Bounded evaluation remains worthwhile after productive publication throughput is established. Promote a frozen-state **authority-discrepancy and repair-cascade census** now; retain the missed-improvement census for residual stalls outside the band. The evidence demotes speculative GLS and landscape replacements, without ruling them out.


## The brief, verbatim

# Round 2: H1 is falsified by your own precondition, and the give-up autopsy found the mechanism

Same repository, head now `c35b1f6` on `engine/topology-archive-search`. Your review 1 is committed verbatim as `docs/astra-review-1-the-achieved-depth.md`; your specification as `docs/quorum/ics-achieved-depth-v1-spec.md`, signed before any scored cell. The rollback near-set defect you found is fixed with your regression vector (`6aaa4e1`; it fails without the one restored line and passes with it).

## What happened to H1

Implemented behind `--publishachieved=1` by an isolated agent (bit-identical off; adversarial verifier: no rescue row, no Sparrow copy, validator untouched). Then your preflight 5, read off six consumed-seed cells before any virgin seed:

| cell | above-target exact attempts | certified | conversion |
| --- | ---: | ---: | ---: |
| legacy s18 / s19 / s20 | 178 / 282 / 572 | 22 / 26 / 21 | 12.4 / 9.2 / 3.7 % |
| wall10s s18 / s19 / s20 | 199 / 189 / 214 | 16 / 3 / 6 | 8.0 / 1.6 / 2.8 % |

Required median conversion 50 %; measured under 10 %. **The battery did not start.** You were right in §Q1: those were discarded improving *proxy* states, not dual-valid layouts. Nine-seed dev measure for the record: Legacy paired +0.51 mm but 5/9 and worst −5.53 (seed 20: 1,141 iterations in bite 22 with 247 exact calls where the frozen engine made 2 - same 22 attempts, then 5.5 mm worse at the end); Wall10s −0.06, 2/9. Iterations per published bite did not fall. H1 stays in the tree, off, as the control it now is.

## The autopsy, always on now

Every exact checkpoint records the kernel's first-scan failing rows, the give-up row kind and shortfall, and **for the first failing pair both authorities' reading of the same pair before any repair**.

One control cell, seed 20, Legacy, 341 checkpoints, 219 with a failing pair on the first scan:

| | kernel shortfall on the first failing pair | proxy violation on the *same* pair |
| --- | ---: | ---: |
| median | **1 um** | 0.041 um |
| max | 4 um | 3.62 um |
| exactly zero | - | **100 of 219** |

Kernel minus proxy on the same pair: median **+1.00 um**, p90 1.00, max 1.97, min −0.33. Histogram of the kernel shortfall: ≤1 um: 183, 2-4 um: 36. The most frequent first-failing pairs recur: (1,17) 42 times, (7,21) 25, (1,37) 15.

When the repair then gives up, it is on a **pair**, every time, at a blocking shortfall of **median 5 um, max 11** (control), 5-13 um under H1.

Reading: the proxy (signed-gap SAT on f64 rings, pair clearance exactly 5.000) converges layouts to the *edge* of the kernel's acceptance region. `GridRing::of` canonicalises to integer micrometres and the kernel is fail-closed by one grid cell: a pair the proxy holds at 5.0000 reads as 4.999. The repair then moves one piece by `shortfall + guard` = 1 + 4 = 5 um to clear that row and pushes it into another neighbour that was also at exactly 5.000, creating a 5 um shortfall that exceeds the 4 um guard. Give up. The separator keeps iterating at Phi≈0 - GLS weights shuffling pieces at the micrometre - until a state happens to repair. That is the churn. Sparrow's zero is its legal; ours, by contract, is not.

## The mechanism now being implemented (isolated agent, knob default off)

`--proxymargin=<um>`: inflate the proxy's clearances only where the proxy rows are measured - `energy::rebuild_piece_rows` pair clearance `5.000 + m`, `broad_phase::boundary_residuals` edges `+ m` and top `T − m`. Nothing in `publish.rs`, the kernel, the repair, `raw_source_depth_mm` or the validator changes; the band stays 4 um. At `m = 4 um` a proxy-zero state has every true pair distance ≥ 5.004 (kernel reads ≥ 5003) and a 5 um repair move into a neighbour at ≥ 5.004 leaves a 1 um shortfall the guard can repair. Also measured at `m = 8`. Cost ≈ `m` per stacked row, under 0.1 mm of depth.

This is your §Q1 alternative - "a separate proxy ceiling used consistently in boundary scoring" - generalised to the pair rows, which is where the autopsy says the failure is. You ranked it below H1 and said "it asks the separator to solve a harder problem and can prevent entry to the band altogether." The census says H1's premise was false; I want your reading of the margin *now that the mechanism is measured*.

## Questions

**Q1.** Is the reading of the autopsy right? In particular: is a fail-closed integer kernel one grid cell stricter than the f64 proxy the whole story of the 1 um, or is there a second source in the 2-4 um tail (36 of 219)? Say what would distinguish them.

**Q2.** The margin: attack it. Does it trip any row of §5.2? Does it change what quality means (it does not touch publication or the validator, but say so or say why not)? What is the right value of `m` and why - 4 um (the band), 8, or derived from the rounding bound plus the repair guard - and what does the depth cost look like on a 61-piece strip? Is there a better formulation - e.g. margin on pair rows only, leaving the top row at `T` so the depth is not given away?

**Q3.** The repair itself: it is one-row Gauss-Seidel with a `+guard` overshoot and a 16 um per-piece cap, and the autopsy says its failure mode is pushing a piece into a second neighbour. Is a two-sided or joint correction worth building, or does the margin make the repair a non-issue?

**Q4.** Amend `ICS-achieved-depth-v1` into the specification for the margin round. Keep everything you can (six-arm structure with O/R/M, virgin seeds 27-35, envelope, clauses 1-14, PASS-ADVANCE / PASS-SPARROW-REFERENCE) and change only what the mechanism requires: the treatment knob, the preflight (what census replaces the conversion census - I propose: on corrected Legacy controls on seeds 0-8, the count of "a failing row is outside the 4 um band" give-ups per cell must fall by at least half with the margin on, and exact checkpoints per publication must fall below 1.5), and whether `m` is fixed by the specification or one arm per value. Give the final text; I will commit it before the first scored cell.

**Q5.** Beyond the margin, with the mechanism now known: is there anything on your original list that the finding promotes or demotes? Bounded candidate evaluation, the frozen-state census, anything new.

Under 1,000 words; no preamble.
