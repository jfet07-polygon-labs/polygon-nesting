# Grok review 8 — the promoter, not the bouncer (2026-08-22)

Independent post-round review of the round-envelope kernel + matched gate
(commits e523c91..2e63d4a on b67f350). Run on this machine:
`grok --reasoning-effort xhigh --permission-mode auto --disable-web-search`,
full-tree read access. Reply verbatim below.

---

**DO-NOT-PROMOTE stands, and is stronger than the gate README claims.** The kernel is a real exact predicate with a real 1 µm miter leak at contact. The matched gate did not fail because the kernel is expensive or unsound; it failed because, on these 12 parents, union confirmation is a no-op. I did not re-run `collect.sh`; every number below is from the committed evidence and the source it points at.

---

## 1. Method

### Kernel battery — no false-accept false positive

**√2 µm band.** The derivation is right: nearest-micrometre snap moves a vertex by at most \(√2/2\) µm; an edge interior is a convex combination, so the same; two rings ⇒ Hausdorff \(≤ √2\) µm on boundaries ⇒ \(|d_\text{canonical} - d_\text{source}| ≤ √2\) µm. It is derived, not measured (the README already says so). The largest sweep deviation, 1.970 µm on a *floored* comparison against a 2.4143 µm budget, is consistent and does not independently test the constant.

**The 3 in-band rows are not false accepts.** Shortfalls are 0.056 / 0.296 / 0.015 µm against demanded \(2r\). Folding them into `falseAcceptCount` would demand the kernel refuse whenever source rings sit below \(2r\) by less than a grid diagonal — i.e. force the outward-only-with-margin policy that Grok 7 named as the thing that must refuse Sparrow pair 38·39. Counting them separately is the treatment that keeps pair 38·39 at \(r=2.500\) as a unit test rather than a regression. The boundary in-band row is scored against the two-ring budget; that is conservative (the sheet is already on-grid), not a hide.

**Population 2 being constructed is a disclosed caveat, not a hole that reopens soundness.** Contact-block really did not commit placements. The construction (2-piece sub-layouts, walk in 1 µm steps, keep contract-accept ∧ miter-refuse) is the right grain: a false accept of a disc kernel is pairwise or boundary. The window is centred on the *kernel* flip, so it sits in the disc-threshold neighbourhood, which is where a false accept would live. What it does not cover is rotation-as-the-walk and 61-body coincidence; those are not how this predicate false-accepts.

The inherited material-valid / canonical-invalid population **does** exist: Gate A’s 31 join-fail pairs at \(r=2.5\). Population 3 already accepts all of them. That is a stronger “would the kernel bless the actual miter-illegal layout” test than the 194 constructed walks.

**Other kernel notes, none fatal.**
- Containment, hole rings, and crossing segments each have a unit test aimed at the exact false-accept they would produce. The `i128` bound is evaluated at domain extreme with four bits of headroom.
- Fail-closed at \(2r=0\) and outside `DOMAIN_MAX_MICRON` is real; the wire point falls through to miter.
- `pair_admissible` uses `two_r.max(1)` after a `debug_assert!(certifies(two_r))`. In release a direct caller at 0 would silently certify at 1 µm. Production is gated by `certifies()` at `general_fast.rs:3854`. Nit, not a battery defect.
- Cross-platform bit-identity is a derivation (integer, no division). Measured: 2-process and 2-binary (`±fcv`) only, x86_64 only. The README already corrected this.

### P0 is sound. The function name is not.

Thirteen rows, every shortfall exactly −1.0 µm, every measured miter intersection area exactly 0.0 mm², 11/13 with source-ring clearance below demanded \(2r\), all 13 still above the 5.000 mm material contract. Independent arithmetic on `summary.json` matches the table.

The proof structure holds:

- continuous miter (and square, when the miter limit fires) \(⊇ P ⊕ \mathrm{disc}\)
- kernel \(d < 2r\) ⇒ disc interiors overlap ⇒ continuous miter interiors overlap
- Clipper reports 0 area ⇒ the leak is in the discrete offset

**Naming defect, same class Sol 12 already flagged on Gate A.** Production miter never calls `do_round`. That function is `JoinType::Round` arc emission (`offset.rs:765`, used only at `offset.rs:893` on the Round join). The production path is `do_miter` / `do_square`, and *those* snap with `math_round` onto the 1 µm `Path64` grid (`offset.rs:754`). Finishing union can also drop a sliver. “Quantized Clipper offset, not continuous miter” is the right attribution; pinning it on `do_round()` is the wrong identifier.

That does not touch the finding. Exclusive aborting a bare-request run at the constructor’s self-check, and 12/12 exclusive pinned-parent cells exiting 1 before search, are the same leak measured end-to-end. `rek=1` (union) is a real design deviation from “the kernel is the envelope half,” and it is the only arm that can load this constructor.

### Shipped authority integrity — contract-valid is the product end of it; not the engine-spec end of it

Every P0 row is above 5.000 mm (min source clearance 5.00084 mm). 192/192 gate publications pass the untouched contract validator. The publication AND is still “quantized-miter envelopes disjoint **and** source-ring contract.” Both halves pass on everything this campaign has published.

What is not intact is the *sentence* “`collision_expansion_mm = 2.502` means 5.004 mm of envelope clearance.” At contact the implemented envelope is “integer miter polygons have zero intersection area,” which is one grid step permissive of that sentence. Manufacturing kerf is orders of magnitude coarser than 1 µm, so this is not a shipped-parts defect. It *is* why exclusive cannot coexist with a constructor that places on the leak, and it is why union is a compatibility shim that **inherits** the leak: union = miter ∨ disc, so the 1 µm permissive set stays.

The millimetre-scale product issue was never this 1 µm. It was Gate A’s join tax (median ~0.5 mm, worst 2.33 mm on a pair). That is a different hole, and this round showed it does not bind at confirmation of *this* search.

### Gate — two real defects, neither moves DO-NOT-PROMOTE

**Interpolation inflates the win-count clause.** `gateverdict.py` sorts each seed’s work-capped runs by *wall* and linearly interpolates depth. Wall and work are not monotone on 2/12 seeds (1 and 2): e.g. seed 1 union does 8 M work in 4.67 s at 172.24 mm, then 3.3 M work in 7.64 s at 174.58 mm. At the 3.3 M control wall (7.29 s) the interpolant reads 168.71 mm and books a **5.86 mm win** for an arm whose same-work cell is bit-identical to the control.

A work-capped ladder is not an anytime wall curve. A 16 M run finishing faster than an 8 M run is load, not “the cheaper arm had reached 16 M work at that wall.” The no-interpolation column the agent published (1/12, 3/12, 0/12, 0/12) is the honest step-function and fails the 8/12 clause at every budget. The quieter-box pre-commit ladder still has median 0.0009–0.1996 mm.

The 1 mm clause fails even on the inflated reading (best median 0.0632 mm). Cashing the measured slice-wall ratio (~7 % more work) along the *work* axis, which is what 48/48 identity actually licenses, yields ~0.11–0.30 mm at the three budgets that have a next rung, and 0 mm at 32 M. **DO-NOT-PROMOTE is correct, and both quality clauses fail on the reading that does not scramble walls.** The “8/12 at two of four budgets” tick is a false pass of the interpolant.

**The per-row Sparrow claim is false.** `sparrow-republish.json` at allowance 0.002 reports a miter *message* of one sheet-boundary failure and two kernel pair failures. That message is the first short-circuit of `rebuild_one`, not a census. Gate A’s census at the same radius is **37 pair failures and 4 boundary failures**, of which the two kernel pairs are the radius-caused subset; `roundRefusesMiterAcceptsPairs` is `[]`. Per-row OR still refuses those two pairs. A per-row hybrid would **not** have admitted Sparrow at shipping allowance. Per-layout union is the right design (a per-row disjunction is a third authority); the “nuance” that it is what blocks Sparrow at 0.002 is the wrong reason. The right reason is the 2.502 expansion (allowance tax), which both halves refuse.

**Env door is not a measurement contaminant.** `rek` cannot reach a pinned-parent slice (`run_portfolio` is from-request only). `POLYGON_NESTING_ROUND_ENVELOPE_KERNEL` on the example, modes not booleans, refused without the feature, refused alongside a portfolio spec, asserted on 96/96 cells, is the door the specified gate needed. Arming is process-global rather than RAII; the file documents that this process serves one request and exits. 164 insertions, one example file, library unchanged. Fine.

**Union-per-layout** is a design choice, correctly implemented (kernel first; on exclusive refuse, fall through to miter; report round metrics even on miter-admitted rows). The 48/48 identity is on `rawSourceDepthMm`, fingerprints, and `stepDigest`, so the metric-basis move did not leak into the scored millimetres.

Determinism first-pass failure committed as failed, then stripped by named clock fields plus 14 verdict paths: the right correction. The git-metadata / compiled-binary correction in §5.4/§5.6 is also right.

---

## 2. Central interpretation

**Accept the reading, with one sharpening the refusal counts force.**

On 108/108 matched-gate runs, `schedule_confirmationsRefused = 0`. The control arm attempted **44,710** confirmations and accepted 44,710. Attempted equals accepted on every paired cell. The union released zero moves at confirmation because **the miter confirmation path never refused anything either.**

That is not “these 12 parents have no near-join structure.” Six of twelve parents carry P0 contact rows; Gate A’s Sparrow layout carries 31 join-fail pairs at \(r=2.5\). The structure is in the *layouts*. It is not in the *confirmations*.

What the search actually filtered is one level up. `due_for_confirmation` skips when `score.feasible()` is false (`compression_schedule.rs:739`). That score is the relaxed surrogate: `OrientedSurrogate.collision` is `polygon.offset(collision_expansion_mm(...))` — production **miter** offset, then convex cells / hazard. Across the four-rung miter ladder that skip counter is **149,762**, identical on the union arm cell for cell (242, 461, 1770, 5944, … — every paired skip count matches). Micro-legalization attempted 0/108. So:

| stage | geometry | what happened |
|---|---|---|
| proposal / proxy / lanes | miter offset + convex surrogate | 149k “infeasible” skips, identical both arms |
| confirmation | union kernel, then miter | 44,710/44,710 accept, 0 refuse, identical |
| publication | both authorities | 96/96 miter-legal, 0 new admissions |

The residual barrier is reachability **including the proxy that is allowed to hide the released region from confirmation.** 48/48 bit-identical searches at equal work is then almost tautological: the slice is `work=W` capped, cheaper confirmation cannot buy more work under a work cap, and the two authorities agreed on every confirmation they were shown. The cost advantage (0.5216× per confirmation, 0.931× per slice) is real and is the only thing equal-wall interpolation had to cash. It buys ~0.06 mm of model, not 1 mm of search.

A remaining unmeasured quantity: what fraction of those 149k skipped frontiers are disc-legal / miter-illegal. The agent did not re-score the skip pile. That is a gap in the *size* of the released region inside the search, not a gap in the 48/48 identity.

The from-request coordinator is a different machine. There the authority *can* change the basin (constructor self-check, early accepts). Anytime `plan=10s` on 9 seeds is −2.135 mm median with range \([−8.5, +9.9]\) mm; the one distinct new admission (wall=10s seed 1, 171.95 mm, 58/61 off-lattice) is 6.30 mm *worse* than the same-seed miter publication. That is Gate A case 3 happening in a live run, and it is Grok 7’s warning measured: legalising a topology does not make a good one appear. `crot` under round flips at a work budget and not at a wall budget, with per-seed range \([−14.9, +13.3]\) mm — lottery, not an operator.

Sparrow is publishable under union only at allowance 0.0. At 0.002 the kernel itself refuses the two radius-caused pairs. That is legality of one fixture, not a 10s curve.

---

## 3. Ranked next steps

### The evidence shows

Legality of \(P ⊕ \mathrm{disc}(2.5)\) is now an exact, cheap, feature-gated predicate. On the 12 pinned parents, putting that predicate on the *confirmation* wire does not change the search. Economics of confirmation are favourable and mostly irrelevant: mode-34 does not spend its wall there. The 150@10s gap is still ~18 mm on the wall line, ~23 mm on the reproducible plan line. Operator space on this box was already closed (contact-block, m26, crot, sparse rotation, race). Representation-at-confirmation is now closed too.

### I recommend

**1. (a) — keep the kernel as an available correctness surface (union, off by default) and stop the 150@10s goal.**

This is what Grok 7 already licensed if the join may change and the kernel then fails the 1 mm clause. It has now failed that clause by more than an order of magnitude, on the population the clause was written on, with the honest equal-work column at 0.0000 mm.

Kill for reopening 150@10s via this kernel: a later round that changes *proposal* geometry (not confirmation) and then beats ≥8/12 and ≥1 mm at equal operator wall, with zero false accepts, and with new admissions not sitting in contact-block’s 0.5 mm class. The present evidence does not start that round.

The kernel is still worth keeping in-tree: it is the only exact disc authority, it is the instrument that named the 1 µm leak, it is 8× cheaper on the envelope half, and it is the import path for Sparrow-class fixtures at \(r=2.5\) / allowance 0. Do not default it on. Do not promote. `rek=2` stays a measurement mode.

**2. (e), only if someone still wants a closed question rather than a spend — re-score a sample of proxy-skipped frontiers under exclusive.**

The 149,762 identical skips are the only place a released region could be hiding on this population. A few hundred skipped layouts, run through `wired_verdicts`, answers “is the skip pile join-tax or bulk overlap?”

- Kill (b) a priori if ~0 of the sample is kernel-accept ∧ miter-refuse.
- Kill (c) even more firmly if the skip pile’s released rows are sub-micron (P0 class) rather than 0.5 mm class.
- If the sample is mostly join-tax, that sizes (b) before anyone touches 27 k lines of `general_relaxed.rs`.

This is a diagnostic, not a promotion gate. I would not fund it as a campaign round unless (b) is already on the table for product reasons other than 150@10s.

**3. (d) — do not build a per-row union. Allowance revision is a product call, not a 10s lever.**

Per-row union is a third authority contained in neither half. The claim that it would publish Sparrow at 0.002 is false (both halves refuse the two radius pairs). Publishing Sparrow at allowance 0.0 is already what union does; Grok 7 already said a re-import pass is not a 10s win. Changing shipping allowance from 0.002 to 0.0 to make the disc match the contract 5.000 mm is a spec decision about the 0.004 mm radius tax. It does not generate candidates.

**4. (c) — do not revive contact-block under union.**

Not killed *a priori* by 48/48: m34 never proposes those SE(2) blocks, so zero released confirmation moves is the wrong corpse. The 6.3 mm-worse coordinator admission is also the wrong corpse.

Killed by its own economics and by Grok 7’s third kill. Contact-block’s retracted contract-only median was 0.506 mm; composite was 0.044 mm; m34 at the design slice is 1.104 mm. EnvelopePair rows still carry `contract_mm = 0.0` on **miter** collision geometry (`contact_block.rs:511–516`). Switching confirmation to union without changing that linearization leaves the program with the same first-order zero-slack model. Switching the linearization to disc distance could recover some of the 0.5 mm class. That still loses to m34 on both original clauses, and it is exactly the “new admissions around 0.506 mm against m34’s 1.104 mm” kill. Seed 3’s 0.0005 mm “win” was m34 finding nothing. Plausibly flipping 1/12 to something nicer does not flip the operator.

**5. (b) — move round distance into proposal/collision geometry: large, not licensed as the next spend for 150@10s.**

What it is: replace miter `PolygonSet::offset(collision_expansion_mm)` in the surrogate catalog, pair rows, NFP, hazard, lanes, and the constructor that places at quantized miter contact. Surface, line counts as they sit:

| file | lines |
|---|---:|
| `search/general_relaxed.rs` | 27,642 |
| `search/general_fast.rs` | 5,628 |
| `search/general_micro_legalization.rs` | 3,476 |
| `search/general_micro_legalization/se2_certificate.rs` | 2,828 |
| `search/general_micro_legalization/contact_block.rs` | 1,722 |
| `search/general_hazard.rs` | 1,557 |
| `geometry/collision_builder.rs` | 1,103 |
| **sum** | **~44 k** |

Plus the short-side-first constructor, which is why exclusive cannot load a run. This is not a wire-point swap. It is “the search reasons in discs.” Expected yield on the present evidence is the join-tax class (0.5–2 mm on pairs, Gate A’s 0.56 mm edge tax on one layout), not the 18 mm to Sparrow. Off-lattice proposal is a separate barrier (57/61 of that layout; `crot` still a measured negative). Constructor ~180 mm @ 1.4 s and 3× work → +6 mm do not change.

Kills for (b), if it is ever funded:

- skip-pile sample (item 2) is empty of released rows
- equal-work 12-parent gate still 0/12 after the proxy uses round distance
- new admissions cluster at ~0.5 mm against m34’s 1.1 mm
- any false accept vs the untouched material validator
- exclusive still aborts at constructor (the constructor was not actually moved)
- wall-line gap to Sparrow still ~18 mm (a local 0.5–1 mm promotes the tool, not the goal)

I would not fund (b) for 150@10s. I would fund it only if the product decision is “the envelope half should be \(P ⊕ \mathrm{disc}\) everywhere, including how we search,” independent of ten-second millimetres.

---

**Bottom line.** The kernel did the job Sol 11/12 and Grok 7 funded: exact, sound, cheap, and not inscribed Clipper round. The matched gate did the job they funded next: it asked whether that legality moves the 12-parent millimetre, and the answer is no. Union-at-confirmation is a correctness surface, not a 10s curve. Stop.
