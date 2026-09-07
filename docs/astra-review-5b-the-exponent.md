# GPT-6 Astra review 5b: the escape weight and the exponent (postscript to review 5, same session)

Consulted 2026-09-07 at `model_reasoning_effort = xhigh`, resuming the review-5 session
(`codex exec resume --last`), on the owner's instruction ("ignora grok, usa codex astra xhigh").
The brief is reproduced after the reply. Verbatim; nothing edited. Written after the
continuation and revisit probes and before the exponent probe's results
(`docs/experiments/overlap-ics/bite-microscope/README.md`, "The exponent probe").

## Reply

**Keep the exponent first, with p = 1 fixed now as the sole candidate for the 108-cell development screen.** Run all four replay exponents as a mechanism experiment; do not select the live exponent by whichever wins bite 15.

The completed probes support this ordering. On bite 15, continuation saves **23.5%** of evaluations and revisit **1.6%**—useful effects, below the prescribed halving threshold. Their row-clearance results alone do not establish that the remaining delay is entirely weight growth.

**Q6. Pre-register a weight-threshold prediction separately from an efficiency claim.**

Two corrections constrain the numerical prediction.

First, a required weight of approximately 20 takes **about eight updates at 1.5×**, not three to five. Second, Sparrow’s supplied formula does not establish `loss ∝ sqrt(v)` for our source-ring clearance violation. Pole penetration, its decay threshold, other pole contributions, shape penalties and the collision predicate intervene. The square-root comparison is motivation for a surrogate experiment, not a calibrated prediction of Sparrow’s passes.

Using the *actual three old and three new incident residuals* of the recorded escape gives:

| Exponent | New/old unweighted cost ratio | Updates at a hypothetical constant 1.5× |
|---|---:|---:|
| 2 | 156,889 | 29.5 |
| 1 | 416 | 14.9 |
| 0.75 | 93 | 11.2 |
| 0.5 | 20.7 | 7.5 |

These are uniform-old-weight crossover estimates. Actual row weights, candidate availability and subsequent cleanup differ.

I would register the following **strong-support targets**, before inspecting exponent results:

| p | Column-break deadline | First band-entry deadline |
|---|---:|---:|
| 2 | Reproduce **37** | Reproduce **43**, **685,980 evaluations** |
| 1 | **20** | **27** |
| 0.75 | **16** | **23** |
| 0.5 | **12** | **19** |

The treatment deadlines are predictions allowing some startup and cleanup, not consequences mathematically guaranteed by the exponent. Earlier is acceptable.

Define “column break” from committed geometry and row identities: a core-member rearrangement releases the original bottom-to-top branches, which do not re-form before band entry. Record temporary releases separately. If the treatment prevents the column from forming, report **“column avoided”** rather than inventing an escape iteration.

Run each exponent once from the identical bite-15 entry, eight workers, margin 8, both other probes off, with **50 master sweeps maximum**. Keep failures censored; do not extend an unsuccessful exponent’s horizon after seeing it fail.

Interpret the outcomes as follows:

- Earlier release at much smaller **actual old-row weights**, followed by band entry within the table, supports the predicted mechanism.
- Missing a deadline falsifies that numerical prediction. A smaller benefit may remain, but cannot be reported as confirmation of the registered fast-escape forecast.
- Earlier escape with cleanup consuming the saved work supports the acceptance explanation and **fails the efficiency explanation**.
- If none of the lower exponents materially shortens the column’s residence or work to band, reject exponent change as the leading treatment for this exposure.
- If the column never forms, the experiment supports changed trajectory formation. It does not alone demonstrate faster escape from an existing jam.

Retain the separate halving target: **at most 342,990 evaluations to band**, with **at most 21 sweeps** for the iteration-halving claim. Thus p = 1 reaching band at 25 could support the weight-race mechanism while failing the promised halving.

There is a qualification to my previous wording: the implemented replay **stops at band entry and makes no exact calls**. Its numbers establish neither certification nor publication. Require a detached check of the unchanged publication path before promoting any band-entry result to the certification claim. [Replay stopping rule](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/replay.rs:470)

The numerical hazards deserve explicit observation:

- **Infinite derivative:** `sqrt(v)` has an infinite right derivative at zero, but CD evaluates values; it does not evaluate that derivative. `sqrt(0)` is finite. The practical hazard is sensitivity to tiny positive residuals and geometry noise, rather than an unavoidable arithmetic infinity. Preserve exact zero and introduce no unregistered epsilon or smoothing.
- **Conflict concentration:** for equal weights and fixed total violation \(V\), spreading it across \(m\) rows costs \(m^{1-p}V^p\). Squared loss favours spreading; sublinear loss favours concentration. Fewer blocking rows may therefore accompany a larger worst overlap.
- **Equal acceptance:** keep the existing comparison and step schedule. Linear loss can create neutral redistribution directions; sublinear loss changes their shape. Count actual comparison outcomes and accepted equal moves rather than assuming plateaus disappear.
- **Eight-worker selection:** use the exponent consistently in candidate/CD scores and the **pre-update** tournament score. A worker with lower guided cost may have higher raw loss and maximum violation. Record that tradeoff and charge all workers.
- **Weight saturation:** retain `2^20`. Measure capped rows and time spent capped. Earlier escape should generally require less accumulated weight; a treatment that waits at saturation is not demonstrating the proposed shortcut. Never raise the cap to save it.

**Q7. Changing p does not change the frozen GLS multipliers, but it does change the frozen objective.**

The old specification expressly chose `guided = w*v²`. The fact that this choice was ours means it can be reconsidered in a **prospective successor**; it does not make changing it an implementation repair or permit revising an earlier verdict. [Original objective choice](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/grok-review-12-reading-sparrow.md:183)

I would name the successor **ICS-guided-exponent-v1**, with:

> `--guidedexponent=2` as the default/control and `--guidedexponent=1` as this round’s sole live treatment. Replay admits the predeclared set `{2,1,0.75,0.5}`. The exponent is fixed for the entire process, across both phases, all workers, retries and publications.

Beyond review 4 Q4’s six falsifiers, freeze these details:

1. **Exact scope.** Change `guided = Σw*v^p` for every positive pair and boundary residual. Raw `Σv²`, maximum violation, band membership, strike minima, pool selection and the GLS recurrence keep their definitions. Their *trajectories* can naturally change after different moves.
2. **One objective everywhere.** Update [`energy::fold`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:386) and [`incident_totals`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:425). Audit sample-pool ranking, both CD stages, relocate comparisons and both live/replay tournament selection. [`gls_update`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:578) remains unchanged.
3. **Arithmetic identity.** Preserve the original multiplication path for p = 2; do not replace it with generic `powf(v,2)`. Pin fractional-power arithmetic before replay. Reject unsupported/nonfinite exponents and nonfinite scores; NaN must not become an accepted equality.
4. **Replay identity.** The current 43/43 gate compares raw loss, maximum and winner. Extend the p = 2 versus `none` comparison to pose/row/weight/stream fingerprints and evaluation counts. Equal aggregate scalars alone are not full state identity. [Current comparison](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/replay.rs:555)
5. **Correct diagnostics.** Keep raw quadratic Phi identifiable. For p ≠ 2, `guided/rawPhi` is no longer an effective weight. Report actual row weights and, if useful, `Σwv^p / Σv^p`. Record p in executable/request configuration, capsules and results. The existing documents’ top-level seed 0 versus capsule seed 20 also needs an explicit resolved-seed field.
6. **No bundled adjustment.** CD limits, sampling, equality acceptance, weight cap, boundary margin, publication and profile schedules remain fixed. Any later smoothing, boundary-specific exponent or adaptive p is another treatment.

The **108 cells** remain:

| Arm | Legacy | Wall10s |
|---|---|---|
| Control | p = 2, margin 8, cap 50 | p = 2, margin 8, existing unbounded cap |
| Treatment | p = 1, otherwise identical | p = 1, otherwise identical |

Nine seeds **18–26**, three repetitions, ten seconds from the bare request. Retain rotated arm order, per-seed repetition medians and one timed eight-worker process at a time.

The six existing gates remain substantive: demonstrated mechanism; seed-balanced halving with failed opportunities included; lower total work and measured wall at maintained completion; positive paired-median depth gains on both profiles; **no per-seed-median regression greater than 1.000 mm**; and integrity. Capsule measurements are diagnostic, not scored depths. Seeds 27–35 remain untouched.

**Q8. The exponent remains first; the trace must show why it helped.**

My order is:

1. **Exponent change**, with linear loss the first live candidate.
2. **Bounded joint movement that changes relative column geometry.**
3. **Additional escalation associated with unsuccessful relocates.**
4. **Forced relocation selected by raw loss.**

Sparrow’s answer supplies the motivation for rank 1: its quantifier gives different relative prices to minor persistent collisions and major new ones. Its exact quantifier is neither required nor reproduced here.

For the next microscope trace, require the following causal chain:

- **Exposure:** identify whether the same core and spanning contacts form. Report their residence time, positive residuals and above-band residuals separately.
- **Price crossover:** immediately before a proposed escape, decompose stay and candidate scores by row, under both p = 2 and the treatment exponent **at the same poses and weights**.
- **Discovery and retention:** identify when a qualifying candidate is first sampled, reaches refinement, commits within a worker, and survives the tournament. An acceptable candidate repeatedly discarded by the tournament points to a different bottleneck.
- **Consequences:** record core-member displacement/rotation, contacts released, new contacts created, actual weights at escape, cleanup work, band entry and detached publication outcome.
- **Accounting:** report unmoved-relocate counts alongside opportunities and residence time. Fewer calls simply because the run is shorter is not independent evidence.

Add one fixed diagnostic fork at the **end of control sweep 24**, when all four core members were unmoved. Re-score identical candidate poses under all four exponents there. If entry-start treatments avoid the column, this common-state comparison can still test whether changing p would make an existing escape affordable.

The strongest result is an escape candidate rejected by quadratic scoring but accepted at substantially lower accumulated weights under the treatment, followed by cheaper cleanup and certification. Faster band entry through a different contact arrangement is useful, but supports a different causal account.

**Q9. Keep p = 1 fixed before the replay outcomes. A generic knob need not imply a parameter search.**

Use `--guidedexponent` as the single interface rather than maintaining overlapping `guidedlinear` and `guidedexponent` knobs. For this live round, compare only **2 versus 1**.

The reason to select 1 now is structural: it removes the quadratic price amplification while avoiding sublinear loss’s singular slope and stronger incentive to concentrate conflict. It was also selected in review 5 before these exponent outcomes.

**Selecting the best of 1, 0.75 and 0.5 on seed 20 would still be fitting to seed 20—even with a predeclared selection rule.** Such a rule controls discretion; it does not create independent evidence.

Therefore:

- Fractional replays test the predicted response to decreasing p.
- They may expose a reason to reject p = 1.
- They do not automatically substitute their winner into the promised 108-cell screen.
- If only a fractional exponent works, document that development finding and freeze a separate selection/validation plan before further treatment results.
- Testing all four exponents in the live development screen would be **216 cells**, with an additional selection problem—not the registered 108-cell comparison.

A successful development screen can justify proposing later independent confirmation. It cannot make these already consumed seeds an unseen validation set.


## The brief, verbatim

# Brief 5b (postscript to brief 5): how the column broke, and the exponent

Read after your reply to brief 5. Same setting; everything here is committed in
`docs/experiments/overlap-ics/bite-microscope/README.md` (addenda).

## The escape, from the trace (bite 15, margin 8, winner's relocates)

- Iterations 25-36: the column's pieces 0, 6, 43, 44 are `stayPut` and unmoved or moved by
  0.05-0.09 mm at residuals 4-11 um; their effective weights (guided / raw on the relocate's
  incident rows) climb from 1e2-2e3 at iteration 25 to 1.0e5-2.0e5 at iteration 37, about
  1.5x per update.
- Iteration 37: piece 43 (top of the column) commits a **container** sample 165.9 mm away,
  rotated 120 degrees, opening 1.77-1.82 mm overlaps with pieces 40, 42, 60 (fresh rows, weight
  1): raw 6.1e-5 -> 9.58 mm^2, guided 10.7 -> 9.58. In the same sweep piece 6 (focused, 7.2 mm,
  200 degrees) and piece 0 (2.4 mm) clear their rows. Iterations 38-43: pieces 40, 42, 60, 47
  absorb the new overlaps in 1-2 mm moves; proxy-zero at 43.

So the column is broken by a weight race: our guided objective is `w v^2`, a 5 um residual is
2.5e-5 mm^2, and a member leaves only when its rows' weights make that cost more than a
millimetre-scale fresh overlap, i.e. weights of order 1e5, thirty-odd updates at 1.4-1.6x.

## Sparrow's loss, read at the pinned revision (nothing ported)

`src/quantify/overlap_proxy.rs`: per pole pair `pd = r1 + r2 - d`, contribution
`pd_decay * min(r1, r2)` with `pd_decay = pd` for `pd >= eps` and `eps^2 / (-pd + 2 eps)`
below (positive beyond contact), summed, times pi. `src/quantify/mod.rs`:
`loss = sqrt(overlap + eps^2) * penalty`, penalty the geometric mean of the two hull
sqrt-areas. For a colliding pair that is about `sqrt(pd)`. Against a 1.8 mm fresh overlap a
5 um residual costs `(0.005/1.8)^0.5 = 0.053` of it for Sparrow and `8e-6` for us: escape
weight about 20 (3-5 updates) against about 1e5 (30-36). Its 17 passes on this bite fit.

The `v^2` is our own design (Grok review 12, "promote penalty to f64 weight with guided =
w v^2, one guided path"), not the paper's, and the pole proxy stays unlicensed and unneeded:
the question is the exponent on our signed-gap violation in the guided ranking only (raw Phi,
the band, the strike minimum and `v / v_max` untouched).

## Questions

**Q6.** Pre-register the replay probe `guided = w v^p` on the bite-15 capsule, p in {2 (control),
1, 0.75, 0.5}: what iteration of column break and what band-entry iteration would confirm the
hypothesis, and what would kill it? Also the p = 0.5 hazard you can see: infinite slope at zero
violation in the CD comparisons, the coarse/fine CD accept-equal on plateaus, the tournament's
winner selection on guided Phi with 8 workers, weight saturation at 2^20.

**Q7.** Is a change of exponent a forbidden-rescue row (the GLS constants row says the
multipliers are the paper's; the exponent is ours), and what does the prospective spec for
`--guidedexponent=<p>` need beyond review 4 Q4's six falsifiers? Name the knob, the sites
(energy::fold guided term, incident_totals, relocate ordering, gls_update untouched), and the
dev screen (108 cells) arms.

**Q8.** Re-rank Q3 of brief 5 (jam detector, joint column move, faster escalation, Sparrow's
own answer) against the exponent. If the exponent is first, say what the microscope's next
trace must show on the p-run to confirm the mechanism is the one predicted (fewer pinned
relocates? shorter weight race? the same column?).

## Postscript, after your review 5 and the two probes

Your review 5 is committed verbatim (`docs/astra-review-5-the-pinned-column.md`). The two probes
ran from the margin-8 bite-15 capsule with a 43/43 identity gate on `--probe=none`
(`docs/experiments/overlap-ics/bite-microscope/README.md`, "The two detached probes"):
`cdfinish` 30 iterations / 524 855 evaluations (81 388 continuation), `revisit` 35 / 674 855;
bite 17: control 26 / 262 987, `cdfinish` 20 / 245 520, `revisit` 15 / 209 704; margin-0 bite
17: control 37 / 500 681, `cdfinish` 33 / 626 469, `revisit` 32 / 446 862. The five persistent
rows clear at 27-28 (`cdfinish`) and 32-34 (`revisit`) against 37-38. Neither reaches your
342 990 threshold on bite 15, so by your Q4 the isolated linear probe is next: it is being
built as `--probe=exponent:<p>` with p in {2 (identity gate), 1, 0.75, 0.5}, replay-only,
guided = sum w v^p in candidate ranking and winner selection only. Your point that "unmoved"
does not establish local optimality is taken and the README will say so. Q6-Q8 above stand;
add: **Q9.** with your rank 1 now the exponent, is p = 1 the only value to carry into
`--guidedlinear=1`, or should the live knob be `--guidedexponent=<p>` with p fixed
prospectively from the replay, and what stops that from being a constant fitted to seed 20?
