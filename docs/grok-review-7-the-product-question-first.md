# Grok review 7 — the product question first (2026-08-22)

Independent post-round review of the merged Gate A + m26 audition evidence
(commits 2b3f0b9/4e41996 on 1ca3315). Run on this machine:
`grok --reasoning-effort xhigh --permission-mode auto --disable-web-search`,
read access to the full tree. Prompt: the round summary with the three
questions (method defects/false positives in Gate A; method defects in the
audition; the decision surface ranked). Reply verbatim below.

---

The two experiments did what they were funded to do. Gate A’s case-3 reading is real, and the m26 audition’s CUT is real. Neither licenses a 10s curve that approaches 150. A few summary sentences overclaim; none of those overclaims move the decision.

I did not re-run the live validation battery. Numbers below are from the committed evidence and from independent arithmetic on those files.

---

## 1. Gate A — method defects and false positives

**The evidence shows** Grok’s case 3 on this pose set: contract accepts, composite miter rejects, composite round at `r = 2.5` accepts. That is in `evidence/summary.json` and `evidence/verdicts.json`, and it matches `validate_placements_against_contract` / `validate_and_measure_placements` being called unmodified (`import_gate.rs` `authority_verdict`). The import is not an artefact: worst vertex error `2.27e-13` mm, pair `[38,39]` agrees to `1.4e-13` mm with `validation-10s-x86.json` (`import.json`).

### `d − 2·r*` is a sound *price*, not the refusal-cause attribution

Refusal cause is the **set intersection** in `summarize.py:decompose`: miter overlaps and round does not, counted off the full 1830-pair scan, not by subtracting failure counts. At `r = 2.5` that is 31/31 pairs and 2/2 boundaries (`miter-failures.json`; `pairFailuresCausedByRadius: 0`). That attribution is sound given:

- miter shadow reproduces `PolygonSet::offset` on all three radii (`productionOffsetReproduced: [true, true, true]`);
- overlap uses `polygons_overlap_exact`, the composite’s predicate;
- round’s `pairFailureCount` is a full scan, so `roundAtSameRadiusOverlaps: null` on 16/31 rows still means “round did not refuse”, not “round was not asked”.

`join cost = d − 2·r*` is a different quantity. It is the material clearance the miter meter does not credit, relative to a disc of radius `r*`. For items 21·57 it is 2.3343 mm because `d = 7.0843` and `r* = 2.375`. The amount by which this pair *misses legality at r = 2.5* is the shortfall `2·(r* − 2.5) = −0.250` mm, not 2.3343. Switching to round makes the pair legal without moving anything. **Do not add join costs into strip-depth millimetres, and do not equate the 0.5057 mm median join cost with contact-block’s 0.506 mm depth median.** Those are different units on different populations; the match is coincidence.

Quantization: `r*` is the last integer micrometre with zero intersection area, so join cost can sit up to ~0.002 mm off the continuous value. On the 31 refused pairs at `r = 2.5` the minimum is +0.0157 mm — all clearly above that floor.

One documentation slip, not a false positive on cause: the README table’s “same rows under the round join [−0.0012, +0.0022] mm” is `discIdentitySignedRangeMm` over the round-bisected population (tightest 40, plus round failures, across three radii). Of the 31 miter-kill pairs, only 15 have a round `r*` at all. On those 15, round join cost is `[−0.0006, +0.0018]` mm. The other 16, including 21·57 at 7.08 mm, were not in `bisectTop=40`. Cause for those 16 still holds via the full overlap scan (0 round failures at `r = 2.5`). Magnitude of round deviation on the worst miter-kill row was not measured.

### Boundary-`k` is consistent with the miter path

Production is `JoinType::Miter`, `CLIPPER_MITER_LIMIT = 2.0` (`general_polygon.rs:28,387`). Sheet inset is **not** the expansion:

```
collision_expansion_mm = total_padding/2 + margin + allowance  → 2.502
collision_sheet_inset_mm = sheet_edge_clearance − total_padding/2 → 2.5
```

(`general_fast.rs:2176–2221`; pinned by `import_gate.rs` unit test: expansion 2.502, inset 2.5).

`k = (binding material − inset) / b*` with that fixed inset is the wall-normal component of the miter offset. For placement 5 / item 14: `k = (5.125014 − 2.5) / 2.188 = 1.19973`, implied demand `2.5 + 1.19973 × 2.502 = 5.5017` mm. Predicted excursion past the inset line is 0.3767 mm; measured `envelopeExcursionMm` is 0.377 mm. Same `k` at all three radii, and `k = 1.20 / 1.22 < 2.0`, so the miter *limit* is not binding — this is corner geometry plus pose, exactly as claimed.

Two nits, neither fatal:

- Measured `k` is the wall-normal component, hence `≤ 1/sin(half-angle)`. Calling 1.19973 “`1/sin(half-angle)`” over-identifies it with the Clipper spike factor when the bisector is not wall-normal.
- Ceiling 7.504 mm = `inset + 2 × 2.502` is the structural maximum if a limit-hitting spike points at the wall. It is not observed (max measured `k = 1.223`).

The +0.56 mm join asymmetry vs Sparrow’s flat 5.0 is real on this layout. It is an **edge-clearance** tax on the long-axis *origin* (`min y`), not on the depth-setting edge. Published depth is `max y + 5.0 = 150.16451`. The join does not shave 0.56 mm off that number; it forbids publishing the layout at all.

### Re-pin arithmetic is right; the identity check is sufficient for what it claims

Independent evaluation of `depth-lower-bound-exact-clearance-evidence.json`:

| quantity | formula | value |
|---|---|---|
| `SUM_2.5` | — | 249773.80485530035 mm² |
| identity | `sparrow_bound_mm × 2000` | **0.0 mm²** |
| plain | `SUM_2.5 / 1995` | 125.19990218310794 |
| strengthened | `+ 5.0` | **130.19990218310795** |
| composite plain | `SUM_2.502 / 1995` | 125.2160326353513 |
| composite strengthened | `+ 4.998` | **130.2140326353513** |

Usable width 1995 follows from material in `[5, 1995]` and disc inflation `r = 2.5`. Depth term `D ≥ SUM/1995 + 5.0` is `y_min ≥ 5` plus `D = y_max + 5`. Geometry helpers `shoelace_area`, `perimeter`, `is_convex` are byte-identical to the retired script; `grid_offset_area_bounds` differs only in comments. Kimi’s 124.887 is `SUM_2.5 / 2000`. The retired “~7.09 mm contract overhead” is `131.978 − 124.887 = 7.091` and dies because both contracts are now 5.0; residual envelope-vs-contract at the bound is 0.0141 mm.

The identity check does **not** prove the depth-term algebra (that is a three-line derivation, and it holds) and the composite-native figure inflates by **disc** `2.502`, not miter. That is still a valid lower bound (miter ⊃ disc ⇒ smaller area ⇒ weaker bound). It is not a price of the join.

### Square is a negative, not a footnote

At `r = 2.5`, square still refuses 19/1830 pairs and 2/61 boundaries. Grok’s “square containing the disc” alternative is falsified on this layout. Only the round/disc join accepts. Any product question is specifically miter vs round, not “any super-disc join”.

### What does *not* contradict the summary

`verdicts.json` / `miter-failures.json` agree with the README on the three-verdict table, the 31/2 and 37/4 counts, items 21·57 as worst join cost, the two radius-caused pairs at 2.502 (`[38,39]` at 5.000840 and `[50,52]` at 5.002879), pair 38·39 as the sole `roundRefusesMiterAccepts` at 2.5005, lattice 57/61 off 2.5° (worst 1.24586°; also 57/61 off 1.0°), and the five soundness checks. Feature-gating matches the tree: `PolygonSet::offset` is untouched; 153 insertions / 0 deletions in the four existing production files.

The remaining caveats in the README are correctly caveats: n=1; inscribed round (pair 38·39 has 0.42 µm radius margin; Sol’s outward-only would refuse it); legality ≠ reachability; suite-4 pipe 101 caught and rerun; `free_material_multi_eviction` flake recorded both ways.

---

## 2. m26 audition — method defects and Grok-5

**The evidence shows** CUT. `verdict.json` `killRule.verdict` is CUT at the work-matched control (`m34:15000000`, 3.911 M operator units vs the arm’s 4.094 M): median 0.2332 mm vs 7.0129 mm, **0/12** below, 8.4× on aggregate mm per M operator unit (1.2991 / 0.1547).

### The five-budget curve does neutralize the W-choice objection — for the rule Kimi wrote

Kimi’s Italian `o` is survive iff **both** clauses hold. `verdict.py` implements that as `survivesStrict` and as the published `verdict`. Clause A (beat control median by ≥1 mm) fails at all five budgets. The arm’s median is below the control’s even at one-tenth the work (0.2332 vs 0.2534).

The residual documentation defect: the README (and the brief that copied it) says CUT “under both readings of the kill-rule connective” at all five. That is false for the *weak* reading at `m34:1670689`, where `survivesWeak: true` because clause B passes 8/12 while clause A still fails. `verdict` is still CUT because the code treats strict as governing. That cell is also a loss on the median and a 3.8× loss on mm/work. It does not reopen the arm.

The uncapped 6-rung ladder would **SURVIVE** Kimi’s kill against the two smallest controls (`ladderKillRuleAtEveryControlBudget`). That is why the curve exists: those cells give the ladder 45.5 M operator units against 0.39 M and 0.87 M. Yield falls as rungs are added (0.1547 → 0.0756 mm/M). Not testing 8 rungs is not a gap on the axis the kill is written in.

Structural cap (`stepsPlanned == 1` on 12/12) is the right substitute for a work cap mode 26 does not have. Harness-floor subtraction is a real instrument correction: 6.84–11.91 M is common mode; leaving it in would have compressed a 10× operator-work ratio into ~1.4× process-work. Net-of-floor the matched control is the one being starved (4.5% less work), so the loss is not an artefact of over-feeding m34.

### Grok review 5’s arm-C archival is falsified on the reason, not on the decision

Grok-5 archived the forced ladder as an artefact of a saturated archive after ~16 s v2 (`grok-review-5-stop-and-consolidate.md:29`). From the same pinned from-request parents, `m26:drop1.0` publishes **−5.7266 / −8.2890 / 0** on seeds 0/1/2 vs arm C’s −4.957 / −4.317 / 0. Same shape, same two-of-three, larger magnitudes. The mechanism is in-band.

It still loses: the control at 33.4 M takes **−10.200 / −11.953 / −14.702** from those three parents for 2.6× less operator work. Arm C stays archived. The death certificate changes from “wrong precondition” to “dominated by its own port”, which is `compression_schedule.rs:1–16` (mode 34), the thing Kimi reserved as the follow-up if the audition passed. That is more final, not less.

The 85.4% abort does not transfer (7.1% / 20.4%). The loss is not the ULP bug. The 6.5003 mm seed-10 publication split from a `2.235e-4` mm bound difference is real (`audition-first-pass-RETRACTED.json` 169.5948 vs corrected 176.0952); the luckier retracted pass still loses the matched median by ~6.5 mm.

`shipped-surface.md` has **not** received the retirement row the audition README drafted. That is a merge-hygiene gap, not an evidence gap.

---

## 3. Decision surface

### What Gate A licenses

- The legal set of the **contract** contains this 150.16451 mm layout.
- The legal set of **`P ⊕ disc(2.5)`**, as approximated by Clipper round with 0.0001 mm arc tolerance, contains it.
- The legal set of the **miter grid at the same radius does not**.
- That is Grok’s case 3. Cases 4 and 5 are closed on this layout.
- The join, not the 0.002 mm allowance, is the multi-millimetre refusal. Allowance is a separate 0.004 mm pair-clearance tax at the shipping radius.
- Square is not a substitute.
- The 131.98 / 7.09 mm contract-overhead story is retired. Quote 130.1999 (contract) / 130.2140 (composite-native).
- The product question “is `JoinType::Miter` at limit 2.0 an immutable half of the publication AND?” is now a live question with millimetres on it. Architecture still publishes only when envelope-grid **and** source-ring contract both pass (`next-generation-engine-plan.md:36–42`).

### What Gate A does **not** license

- That a round authority would *find* 150.16451. It would stop forbidding it.
- Sol review 11’s three-population zero-false-accept gate. This layout cannot produce a false accept: the contract accepts every row. Pair 38·39 is the exhibit that inscribed round is not a promotion candidate.
- Any millimetre of 10s-curve depth. Join cost is pair clearance; the 0.56 mm edge tax sits on `y = 0`, not on published depth. The depth stake is binary: this layout becomes publishable or it does not.
- That the 5.1 mm record-vs-Sparrow gap *is* the join. n=1 falsifies “miter-legal set contains Sparrow”. It does not prove no miter-legal 150-class layout exists.
- Reachability. 57/61 poses are off the 2.5° lattice **and** off 1.0°. `continuous-rotation` already expressed 46/61 off-lattice poses and made mixed-61 10s **+3.721 mm worse**, 0/9 better.
- A search-economics change. Constructor still saturates near 180 mm in 1.4 s; 40 M → 120 M still buys +5.964 mm; the record 155.264 took hours on layouts the *current* composite already accepts.
- Skipping the product question and building a kernel.

### What the audition licenses

m26 is gated-negative on the 171–179 band, including against its own shipped port, including at one-tenth the work. Reopening it needs a new mechanism, not the ULP fix and not another sweep. Kimi’s “né un positivo gated né un negativo gated” is discharged.

---

### Ranked next steps

**1. (ii) Product: is the miter join an immutable half of the publication AND?**

This is a decision, not a round of engine work. Sol review 11 already named it as the stop condition.

- If **yes**: (iv). Representation cannot be the spend. Operator space on this box is closed (contact-block dead, m26 now gated-negative, rotation arms negative, race retired).
- If the envelope must stay but the **join may change**: (i).
- If the envelope half may be **dropped** (publish on contract only): that also legalizes this layout, and it is a larger contract change than Sol specified. Gate A licenses sufficiency on this pose set; it does not license that as the smaller/safer change.

Kill for (ii): a written product answer. Do not start (i) without it.

**2. (i) Sol’s certified round-envelope kernel — only if (ii) says the join can change.**

Keep Sol’s gates unmodified:

1. Shadow vs source-ring validator on the three populations, **zero false accepts**; every currently canonical-valid layout stays valid. Inscribed Clipper round is not this kernel. Pair 38·39 at `r = 2.5` is the unit that outward-only-with-error-in-margin must refuse.
2. On the same 12 parents: round-envelope → m34 vs miter m34 at **equal operator-wall**. Promote only on ≥8/12 paired wins, better mm/s, ≤1.25× overhead, every publication passing the intact material validator.
3. Kill immediately on any false accept, **or** if new admissions stay around contact-block’s 0.506 mm depth ceiling against m34’s 1.104 mm.

Do not substitute Grok’s “A/B 10s miter vs round” using the inscribed shadow. That can false-accept. A diagnostic A/B is not a promotion gate.

Additional kill from *this* evidence: a pass on re-importing Sparrow is not a 10s-curve win. The 12 parents are already miter-legal; round may release only the local moves the miter was pinning (contact-block’s 0.5 mm class), not a 5 mm topology jump.

**3. (iv) Honest stop — for 150@10s via operators, already licensed; for representation, contingent on (ii).**

Stop is the right answer to “is there an operator in `{m20…m34 + rotation + overlay + race + contact-block + m26}` that closes 18 mm in 10s?” The audition closed the last ungated cell in that set. Stop is also the right answer to 150@10s if miter is immutable, because the remaining 5.1 mm below the record is then either unreachable under the shipped predicate or a search problem that hours of cascade did not close.

**4. (iii) Off-lattice reachability — not licensed as a next spend of the existing arms.**

Cost it *alongside* (i) as a co-requirement of any 150@10s claim. Do not re-arm `continuous-rotation` or `sparse-rotation`; those are measured negatives. A new proposal mechanism would need its own matched gate, and even a perfect one does not buy the 13 mm that sits *inside* the current legal set (wall 168.5 → record 155.264).

---

### What a 10s-curve improvement to 150 would require end-to-end

Three independent barriers. Gate A only speaks to the first, and only as legality of one layout.

| barrier | what the evidence says | 10s implication |
|---|---|---|
| **Legality** | This 150.16451 layout is contract-legal and disc-legal, miter-illegal. Square still illegal. | Necessary to stop forbidding Sparrow-class topologies. Not sufficient. Join cost ≠ curve millimetres. |
| **Reachability** | 57/61 poses off 2.5° *and* 1.0°; 59 distinct rotations. Default lane cannot propose them. `crot` expressed 46/61 off-lattice and lost 3.721 mm at 10s. | A new way to propose those poses that does not pay the crot tax. Turning on the existing flag is not it. |
| **Search economics** | Constructor ~180 mm @ 1.4 s; 3× work buys +5.96 mm; 10s wall ~168.5; record 155.264 in hours, already composite-legal; m26 8.4× worse than m34 on the 10s band. | 10s from-request does not contain the work that produced 155, let alone a new basin at 150. |

Closing 168 → 155 is inside the current legal set and is a budget/basin problem 10s does not buy. Closing 155 → 150 *might* be the representation gap Gate A exhibited on one layout; it still requires a search that can reach a now-legal topology from a from-request start in 10s, including off-lattice poses the default lane cannot name.

**I recommend:** ask (ii) before any engine work. If miter is immutable, stop on the 150@10s goal. If the join can change, the only licensed spend is Sol’s kernel with Sol’s kills, with reachability costed in the same round as a co-requirement, not as a sequel. I would not fund an m26 follow-up, a crot revival, or a 10s A/B on the inscribed shadow.
