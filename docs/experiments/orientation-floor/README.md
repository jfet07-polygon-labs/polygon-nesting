# The orientation floor was still a floor, and the repair tier was the wall

The standing record on the true 5.0/5.0 exact-clearance contract was
**155.42229074464285 mm**, and it was not merely an incumbent: it had survived
a **finite negative on a declared battery** of 30 search arms plus 6 replays
(record-line-cascade's `probeArms: 36` folds the replays into the search
count; see that round's §7, corrected post hoc — this is not a certified
fixpoint) — mode 31 at four tiny steps, the whole frontier-flatten delta grid
handed to mode 33 under both slack values, mode 26 ladders at three drops and
two seeds, and mode 34 at three distinct step sizes (0.25, 1, 0.1) and two
seeds, with `step=0.25` also probed at a second work budget. Every mode-34 arm
in that battery entered with `parentProxyFeasible: false`, so it measured the
schedule's regrid recovery rather than a local schedule search. The
record-line round closed by saying what the last 0.422 mm would
take: "an instrument this round did not fire rather than more of the same".

Four were fired, and they compose — each one opening the fixpoint the
previous one left:

1. **One more ladder rung.** The orientation ladder's floor was 0.0032 degrees,
   and the round that put it there had already noticed its own symptom — 27 of
   40 accepted rotations sat *on* the floor. One rung further down, 0.00128
   degrees, is the last rung the pose grid can still express. The certified
   fixpoint fell to the first sweep armed with it.
2. **An entry grid deep enough to reach the frontier band.** The cascade's
   frontier-flatten grid stopped at 0.01 mm. This state's frontier has **seven**
   pieces inside 0.040 mm, so 0.01 mm moves three of them and the other four
   are never perturbed at all.
3. **The repair tier was the wall, not the entry.** Mode 33 rejects an arm in
   which any single violation component refuses to re-place — it reports
   `componentsRepaired: 1, componentsRefused: 1` and throws both away. Handing
   the same deep entry to the *global legalization* tiers (modes 30 and 31),
   which push the whole layout under a displacement cap instead of enumerating
   insertion orders, turned a 164-arm fixpoint into a 76-of-198 round.
4. **Every entry family on this line was a translation.** Separating the entry
   from the repair asks what else an entry could be. Rotating the frontier
   pieces *in place* by the ladder's own rungs — the orientation freedom, moved
   out of modes 32/33's internal candidate stream and into the entry, where the
   legalization tier can be handed it — broke three consecutive fixpoints that
   nothing else touched, and won **13 of the last cascade's 18 rounds**.

The line reached **155.26442950832842 mm**, 0.15786123631 mm below the previous record — and it did so by finding a *fourth* instrument once the first three had run out, which is §5a below.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1` |
| base commit | `8cebcaa` (record-line cascade merged, record 155.42229074464285) |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | true 5.0/5.0 exact clearance, search-offset allowance **`0.0005`**, empty warm-start slot — the record lineage's `''` `0.0005` CLI tail |
| measure | `rawSourceDepthMm` of a publication that is `exactValid` **and** `contractValid`, adopted on strict `<` with no decimal epsilon |
| base binary, nine-rung ladder (`jagua-experimental`) | sha256 `80a35c2cc30f5e6f69bd9f994431653056db08d55b1c6498f154f2797da8bfc4` |
| this round's binary, ten-rung ladder (`jagua-experimental`) | sha256 `2ca8b2b43d538f2d8471ec543ebf45e197222437d29ecdb3c4d1627db896ca57` |
| schedule binary (`jagua-experimental,compression-schedule`) | sha256 `ddb7d7468166fae3205d973260712dfa135c068774f0bf8d09e45f654bc8e9e4` |
| box | x86_64, 16 cores, engine pinned at 8 threads, deliberately oversubscribed by concurrent sweeps |

Nothing here is comparable to any number measured at allowance `0.002`.

## 1. The result

| pin | declared raw (mm) | via | delta |
|---|---:|---|---:|
| `record-line-cascade/pinned-fs-155.4223.json` | 155.42229074464285 | the prior record, a finite negative on a declared battery of 30 search arms + 6 replays (not a certified fixpoint — see that round's §7) | — |
| `pinned-fs-155.42197.json` | 155.42196626072334 | flatten 0.001 -> mode 33 p0.05, accepted rung **0.00128** | -0.000324 |
| `pinned-fs-155.41964177680.json` | 155.4196417768017 | flatten 0.005 -> mode 33 p0.05, accepted rung 0.00128 | -0.002324 |
| `pinned-fs-155.41373.json` | 155.4137281129324 | flatten **0.03** -> mode 33 p2.0 (deep entry grid) | -0.005914 |
| `pinned-fs-155.40872811293.json` | 155.4087281129324 | flatten 0.008 -> mode 33 p0.05 | -0.005000 |
| `pinned-fs-155.39673.json` | 155.3967281129324 | flatten **0.25 -> mode 30** (tier H, entry -> legalization) | -0.012000 |
| `pinned-fs-155.38372811293.json` | 155.3837281129324 | flatten 0.1 -> mode 30 | -0.013000 |
| `pinned-fs-155.37372811293.json` | 155.37372811293238 | flatten 0.08 -> mode 30 | -0.010000 |
| `pinned-fs-155.36572811293.json` | 155.36572811293237 | flatten 0.05 -> mode 30 | -0.008000 |
| `pinned-fs-155.36072811293.json` | 155.36072811293235 | flatten 0.05 -> mode 30 | -0.005000 |
| `pinned-fs-155.35272811293.json` | 155.35272811293234 | mode 22, slack 0.8, seed 0 | -0.008000 |
| `pinned-fs-155.35181307831.json` | 155.35181307831448 | flatten 0.0005 -> mode 33 p0.05 | -0.000915 |
| `pinned-fs-155.34181307831.json` | 155.3418130783145 | flatten 0.2 -> mode 30 | -0.010000 |
| `pinned-fs-155.33681307831.json` | 155.33681307831446 | flatten 0.05 -> mode 30 | -0.005000 |
| `pinned-fs-155.33281307831.json` | 155.33281307831447 | mode 22, slack 0.8, seed 0 | -0.004000 |
| `pinned-fs-155.33181307831.json` | 155.33181307831444 | flatten 0.0005 -> mode 33 p0.05 | -0.001000 |
| `pinned-fs-155.33141597700.json` | 155.33141597699955 | flatten 0.001 -> mode 33 p0.05 | -0.000397 |
| **`pinned-fs-155.33041597700.json`** | 155.33041597699957 | flatten 0.002 -> mode 33 p0.05 | -0.001000 |

That line ends at 155.33041597699957, a certified fixpoint of 138 arms. The
fourth instrument (§5a) then took it a further 0.0660 mm over eighteen rounds:

| c2f round | tier | via | declared raw (mm) | delta | others below |
|---:|---|---|---:|---:|---:|
| 0 | `I-rot-m33` | `rot-k1-d-0.008-m33` | 155.32981307831446 | -0.00060290 | 3 |
| 1 | `I-rot-m33` | `rot-k1-d-0.02-m33` | 155.3289952387182 | -0.00081784 | 3 |
| 2 | `I-rot-m30` | `rot-k1-d0.02-m30` | 155.32763453787385 | -0.00136070 | 1 |
| 3 | `B-flat-m33` | `flat0.003-m33-p0.05` | 155.32463453787386 | -0.00300000 | 44 |
| 4 | `H-legal-m30` | `legalflat0.01-m30` | 155.32363453787386 | -0.00100000 | 37 |
| 5 | `I-rot-m30` | `rot-k1-d0.02-m30` | 155.32063453787387 | -0.00300000 | 41 |
| 6 | `H-legal-m30` | `legalflat0.4-m30` | 155.3176345378739 | -0.00300000 | 59 |
| 7 | `I-rot-m30` | `rot-k1-d0.02-m30` | 155.31126417741294 | -0.00637036 | 63 |
| 8 | `I-rot-m30` | `rot-k1-d0.02-m30` | 155.30288952528645 | -0.00837465 | 64 |
| 9 | `I-rot-m30` | `rot-k2-d0.02-m30` | 155.29650956355795 | -0.00637996 | 58 |
| 10 | `I-rot-m30` | `rot-k1-d0.02-m30` | 155.2881252959753 | -0.00838427 | 69 |
| 11 | `I-rot-m33` | `rot-k1-d0.008-m33` | 155.28221538414277 | -0.00590991 | 66 |
| 12 | `I-rot-m30` | `rot-k2-d0.02-m30` | 155.27882509466292 | -0.00339029 | 6 |
| 13 | `I-rot-m30` | `rot-k1-d0.02-m30` | 155.27142950832842 | -0.00739559 | 59 |
| 14 | `B-flat-m33` | `flat0.005-m33-p0.05` | 155.26671615201747 | -0.00471336 | 48 |
| 15 | `I-rot-m30` | `rot-k1-d0.0032-m30` | 155.26642950832843 | -0.00028664 | 6 |
| 16 | `B-flat-m33` | `flat0.002-m33-p0.05` | 155.26444327521912 | -0.00198623 | 15 |
| 17 | `I-rot-m30` | `rot-k1-d-0.00128-m30` | 155.26442950832842 | -0.00001377 | 5 |

**155.26442950832842 mm is 0.15786123631 mm below the previous record** and
3.814 mm below the 159.07876040364792 that stood before the record-line round.
Its fixture sha256 is
`111082132fa7610a1000dd96b36f13e8f9282c28a4006e2333c485b7066ad7b7` and its
placement fingerprint is
`82eaa9762e0df399537226f34490e59d658867a5dec0f4b4ea67537d927a0c72`.

That second table is the round's clearest single statement. **Thirteen of the
eighteen adoptions are rotation entries**, and the other five are the flatten
and legalization grids re-opened *by* them — the `others below` column shows the
mechanism directly. Rounds 0, 1 and 2 each had only 1-3 arms below and all of
them were rotation entries; the state round 2 handed on had **44** arms below.
The instruments are not alternatives, they are a cycle, and the cycle is what
carried the last 0.066 mm.

Every pin in the table is in
`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/orientation-floor/`,
with `index.json` carrying each one's sha256 and fingerprint. Six cascades
contributed **6,299 arms** (`evidence/cascade-c2*-state.json` and the matching
logs), the standalone sweeps a further 629, and the two certification batteries
276 — **7,204 arms** in total, every one of them on the same request, contract
and CLI tail (`drivers/armcount.py` totals them from the drivers' own
documents).

The deltas separate the instruments cleanly: the ladder rung buys
0.0003-0.002 mm a step, the deep entry grid 0.005-0.006, tier H 0.005-0.013 and
tier I 0.0006-0.008. The line is not one lever applied repeatedly — it is four,
interleaved, each opening the fixpoint the others leave.

## 2. The floor's own arithmetic said where it had to stop

The previous round moved the ladder floor from 0.02 to 0.0032 degrees on an
arithmetic argument — a rung `d` moves a vertex at radius `r` by
`r · d · π/180`, so on a hand-sized `r` of 100 mm the 0.02 floor travelled
0.035 mm against a 0.001 mm pose grid, thirty-five quanta rather than the one
its justification claimed. It then reported its own residual symptom without
acting on it: of 40 accepted rotations, **27 were at the new floor and 13 at
the rung above it, and none at 0.02 or coarser**. A distribution that piles on
the floor is the signature of a floor placed above the useful band.

So the ladder gains one more rung of the same 5/2 ratio, **0.00128 degrees**,
and stops there — and it stops there for the same arithmetic, not for taste:

| rung | travel at r = 100 mm | travel on this request's depth-setting family (r = 53.852 mm) | expressible on the 0.001 mm pose grid |
|---:|---:|---:|---|
| 0.02 (the original floor) | 0.0349 mm | 0.0188 mm | yes, 19–35 quanta |
| 0.0032 (the previous floor) | 0.00559 mm | 0.00301 mm | yes, 3–6 quanta |
| **0.00128 (this floor)** | **0.00223 mm** | **0.00120 mm** | **yes, 1–2 quanta** |
| 0.000512 (the next rung) | 0.000894 mm | 0.000481 mm | **no** — below one quantum on every piece in the request |

The knock-on is derived rather than tuned, by the rule the constant already
followed: `ORIENTATION_PERTURBATION_VARIANTS` goes 37 → 41 and the charged-row
budget follows at one anchor-local budget per variant. Nothing else moved —
budgets, caps, ejection limits, insertion-order enumeration, the pose-swap
round, the finalist beam, the bound contract and the exact validator are
untouched, and the ordering rule is unchanged, so the new finest rung leads.

## 3. The A/B, and why the old ladder could not have found this

The same 22-arm sweep — the frontier-flatten delta grid
{0.0005 … 0.01} × slacks {0.05, 2.0} → mode 33 — on the certified fixpoint,
under both binaries (`evidence/ladder-ab-base-155.4223.json`,
`evidence/ladder-ab-new-155.4223.json`):

| ladder | arms | publications | strictly below the incumbent | best |
|---|---:|---:|---:|---|
| nine rungs (base commit) | 22 | 18 | **0** | 155.42229074464285 (the incumbent itself) |
| ten rungs | 22 | 18 | **2** | **155.42196626072334** |

The attribution on the winning arm is as clean as the mechanism ever gets:
`acceptedOrientation = 1`, `acceptedAnchorLocal = 0`, `acceptedStation = 0`, and
the single accepted pose is piece
`68c5a4cc-cf24-447c-ad54-974a021198a8-copy-4` rotated by exactly **+0.00128
degrees**. The geometric diff against the old record
(`evidence/geodiff-155.4223-to-first.json`) is one piece rotated, one piece
translated, zero mirror flips.

And the old ladder could not have reached that pose by *any* sequence of its own
rungs, which is a lattice fact rather than a search observation. The nine old
rungs in micro-degrees are 3200, 8000, 20000, 50000, 125000, 312500, 781250,
1953125 and 4882812.5; the lattice they generate has spacing 12.5 µdeg, and
1280 µdeg is 102.4 of those — not an integer. There is no reading of this
layout on which the previous ladder produces it.

## 4. The entry grid was three pieces deep and the frontier is seven

The record-line cascade's frontier-flatten grid ran {0.0005 … 0.01}. Measured
on the incumbent (`drivers/frontier.py`), the frontier stack is:

| rank | gap below the frontier |
|---:|---:|
| 1 | 0.000000 |
| 2 | 0.003966 |
| 3 | 0.008238 |
| 4 | 0.018966 |
| 5 | 0.021158 |
| 6 | 0.029056 |
| 7 | 0.040153 |
| 8 | **0.171320** |

Seven pieces inside 0.040 mm, then a gap four times as large as the whole band.
A flatten of 0.01 mm perturbs ranks 1–3 and never touches ranks 4–7, so the
grid could not express the move the state needed. Extending it to 0.2 mm
(`evidence/deepflat-155.41964.json`) found `flat0.03 → mode 33` publishing
155.4137281129324, a 0.0059 mm step — more than double what a whole cascade
round had been buying — and the next cascade round from that state went from
3 arms below out of 109 to **25 out of 120**.

## 5. Mode 33 throws away the repairs it has already made

That still left a fixpoint, at 155.4087281129324, of 164 arms — the full deep
grid under modes 32 and 33 at both slacks, the nudge tier, mode 22 over eight
seeds and three slacks, mode 31, mode 26, mode 34 and the mode-23 crossover.

The diagnostics say why, and they say it in one field pair. On the flatten that
should have worked (`evidence/seedtest-m33-155.40873.json`), mode 33 reports:

```
componentCount: 2   componentsRepaired: 1   componentsRefused: 1
rejectionReason: "no insertion order re-placed the 2 violation components
                  inside the 155.458728 mm bound"
```

and on the deep one, `componentsRepaired: 4, componentsRefused: 2`. The repair
is **all-or-nothing**: one refusing component discards every component the pass
already placed. The entry was never the wall — the entry reached
155.3787281129324 — the re-insertion repair was.

So the same entries were handed to the tiers that do not enumerate insertion
orders at all. Modes 30 and 31 legalize the whole layout under a displacement
cap, so a component that will not re-place is not a veto, it is a push. On the
164-arm fixpoint (`evidence/legalentry-m30-155.40873.json`,
`evidence/legalentry-m31-155.40873.json`, `evidence/legalentry-m27-155.40873.json`):

| entry → repair | arms | strictly below the 164-arm fixpoint | best |
|---|---:|---:|---|
| deep flatten → **mode 30** | 28 | **14** | **155.3967281129324** |
| deep flatten → **mode 31** | 28 | **13** | 155.3967281129324 |
| deep flatten → mode 27 | 28 | 0 | — (mode 27 is the probe authority; it never repairs) |

The productive deltas are an order of magnitude deeper than the re-insertion
tier's — 0.1 to 0.3 mm rather than 0.001 to 0.03 — which is the same statement
from the other side: the legalization tier wants an entry big enough to be worth
a global push, and the re-insertion tier wants one small enough to be worth an
insertion order. They are different instruments and the round had been running
only one of them.

## 5a. Tier I: every entry family on this line was a translation

Separating the entry from the repair immediately asks what *else* an entry could
be. Every entry family the line has ever used moves pieces along the depth axis:
the frontier flatten, the rank nudges, the k-deepest nudge. The orientation
degree of freedom has only ever been reachable from *inside* modes 32 and 33, as
a candidate stream — and that stream can only perturb the pieces those modes
themselves ejected.

`drivers/rotentrylib.py` puts it in the entry: rotate the k deepest pieces **in
place**, about each one's own transformed bounding-box centre, by rungs drawn
from the ladder itself. The re-centring is the same construction the engine's
own stream uses and it is what makes the perturbation a rotation rather than a
translation — for a placement `R(r)·s + T` with footprint centre
`C = R(r)·c + T`, the rotated placement is `R(r+d)·s + T'` with
`T' = C − R(r+d)·c`.

On the certified fixpoint at 155.33041597699957 — a state that had just survived
138 certification arms plus 110 further compositions — 80 rotation-entry arms
(k ∈ {1,2,3,5,7} × 8 signed rungs × modes {30, 33}) published **3 below**
(`evidence/rotentry-155.33042.json`).

Its value is not the 0.0006 mm. It is that tier I broke **three consecutive
fixpoints** in the following cascade that no other tier could touch — rounds 0,
1 and 2 each had 4, 4 and 2 arms below and every one of them was a rotation
entry — and that the state it handed to round 3 gave the flatten tier **45**
arms below. The entry families are not substitutes; each one opens the fixpoint
the others leave.

## 6. Mode 32 is not the unproductive tier here

The record-line round measured "mode 33 is the productive tier and mode 32 is
not" — on the 159 basin's 64-arm grid, mode 33 took 4 of 4 sub-record
publications and mode 32 took none — and reasoned about it from the vertex
cover: mode 32 leaves the conflict's partner in place. That reasoning is intact
and the measurement does not carry. Across this round's cascades mode 32 took
**97 of 352** arms strictly below the incumbent against mode 33's 66 of 352.

The honest reading is that "mode 32 is unproductive" was a fact about a basin
whose conflicts were partner-blocked, and that this basin's are not.

## 7. Deferred credit, and tier frequency as a separate knob

The record-line round's process finding was that an adopt-and-restart cascade
ordered by arm cost starves whichever tier is expensive — the cheap tiers
published 0.001 mm and restarted the round, so mode 26 was never reached in 555
arms — and that hoisting mode 26 to the front starved the cheap tiers
symmetrically. Neither pure order works.

`drivers/cascade2.py` removes the ordering entirely. A round runs *every* tier
to completion, concurrently, and then adopts the single strictly-best
publication of the whole round. No tier can starve another, and the per-tier
arm counts become a map rather than an artefact of the order.

That fixes the ordering bias and exposes a second one the old design had hidden
inside it: **cost**. A tier that publishes nothing still charges its wall clock
every round, and mode 26 at ~50 s an arm plus mode 34 at ~30 s an arm were 70%
of a round's seconds for 0 of its adoptions. The answer is frequency, not
deletion — deleting is how the previous round lost mode 26 for 555 arms, and
mode 26's yield is basin-shaped, so it has to keep being asked. With the barren
tiers moved to every-Nth-round the cascade went from 349 s a round to 111 s.

The knob cuts both ways, and mode 22 is the case that proves it: 0 of 24 below
on the 155.4137 state, then **48 of 48** below once the legalization tier
started moving the state, winning two rounds outright. Its yield is conditional
on what the previous round did, which is exactly what a fixed schedule misses,
so it was moved back to every round.

## 8. The negatives, with arm counts

Every one of these is a certified negative on *this* lineage at *this* depth,
not a statement about the operator.

| instrument | arms | below | evidence |
|---|---:|---:|---|
| mode 34 at eight step/budget specs × 2 seeds, on the incumbent | 16 | **0** | `evidence/m34fine-155.42197.json` |
| the same on ancestor 155.4563 | 16 | **0** | `evidence/m34fine-155.4563.json` |
| the same on ancestor 155.4633 | 16 | **0** | `evidence/m34fine-155.4633.json` |
| the ten-rung flatten grid on the three runner-up basins | 66 | **0** | `evidence/basin-155.4563.json`, `-155.4633.json`, `-156.0914.json` |
| k-deepest nudge, k ∈ {2,3,5,7} × d ∈ {0.02 … 0.3} × modes {33,31} | 32 | **0** | `evidence/knudge-155.41964.json` |
| mode 33 seed salt, 3 deltas × 6 seeds | 18 | **0** | `evidence/seedtest-m33-155.40873.json` |
| mode 30 seed salt, 3 deltas × 6 seeds | 18 | **0** | `evidence/m30seed-155.33042.json` |
| 2.5° regrid → legalize → mode 34 at four step sizes | 13 | **0** | `evidence/regrid-155.40873.json` |
| mode 23 crossover, record-line near-ties (in-cascade) | 90 | **0** | the cascade states, tier `G-cross` |
| mode 23 crossover, **seven same-lineage siblings inside 0.09 mm**, 5 cuts, both directions | 70 | **0** | `evidence/crossover-final.json` |
| k-deepest nudge → **tier H**, k ∈ {1,2,3,5,7,10} × d ∈ {0.05 … 0.5} × modes {30,31} | 60 | **0** | `evidence/knudge-legal-155.33042.json` |
| tier H pushed an order of magnitude deeper, deltas 1.5–10 mm, modes {30,31} | 32 | **0** | `evidence/legal-deeper-155.33042.json`, `legal-deeper31-155.33042.json` |

Four of them are worth a sentence each.

**Mode 34 is inert here, and the diagnostics say exactly why.** Every one of the
48 arms returned its parent's depth to the digit. The schedule's own
`compressionSchedule` block reports `parentProxyFeasible: false` and
`parentCollisionPairs: 35` with a `startDepthMm` of 156.247 against an incumbent
of 155.422 — an entry loss of **0.825 mm** from the 2.5-degree `canonical_angle`
snap, which is the barrier the record-line round named. It is not a budget
problem: `step=0.1875` accepted 41 confirmations and still only pulled the floor
to 155.956, half a millimetre above the incumbent it started from.

**Walking around that barrier still costs more than it pays.** The regrid probe
moves 49 of 61 poses onto the 2.5° grid for an entry loss of 0.779 mm, legalizes
to 156.395, and then mode 34 *does* ratchet — 155, 294 and 467 accepted
confirmations at the three fine steps, against 0 on the unregridded state — and
reaches 155.604. That is 0.195 mm **worse** than the incumbent it left. The
mechanism is confirmed twice over and the trade is still negative, now measured
at 155.4 as well as at 156.9.

**Modes 33 and 30 are both seed-invariant.** Eighteen arms each, over three
deltas and six seeds, returned bit-identical results — the entry-tier arms all
publish the same raw whatever the salt. The seed is a real knob for mode 22
(which won two rounds on seed 0 alone) and it is not one for the two tiers that
produced most of this line, so their negatives do not need re-running over
seeds. That is worth stating because it removes a factor of six from every
future entry grid, and because it means the 549 standalone arms cover more of
the space than their count suggests.

**The sibling crossover is a much stronger negative than the last one.** The
record-line round ran mode 23 against the old record co-states, a basin 4 mm
behind, and correctly flagged its 0-of-24 as a fact about that pool. This round
ran it against **seven states of its own lineage inside 0.09 mm of each other**,
produced by four different instruments, over five cut fractions in both
directions: 70 arms, 0 below, best 155.33581 — 0.005 mm above. If crossover were
going to pay anywhere it would pay on a pool of genuine near-tie siblings, and
it does not pay there either.

## 9. Regression

The four pinned gates, on the base nine-rung binary and this round's ten-rung
binary (`evidence/gates-base.json`, `evidence/gates-l10.json`):

| gate | pinned | base | ten-rung |
|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | hit | hit |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | hit | hit |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | hit | hit |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | hit | hit |

All four are `exactValid` and `contractValid` in every run. The stronger check —
every field of the benchmark document compared, with only the wall-clock and
build-identity fields removed (`evidence/docdiff-base-l10.json`):

| comparison | fields compared (g1/g2/g3/g4) | differences |
|---|---|---|
| ten-rung vs base | 3,262 / 3,243 / 3,243 / 3,243 | **0** |

Zero, which is the expected result and worth stating plainly: the gates are
modes 20 and 22, and neither enters the orientation-perturbation stream, so a
rung added to that stream must not be able to reach them. It cannot.

Release suite with `jagua-experimental`: **1,250 passed, 0 failed, 2 ignored**
(`evidence/suite-jagua-experimental.log`), and with
`jagua-experimental,compression-schedule`: **1,264 passed, 0 failed, 2 ignored**
(`evidence/suite-compression-schedule.log`). The three ladder-derived assertions
(the variant count, the row budget and the quota arithmetic) were updated from 37 to 41 by
the rule they already stated, and the ladder test's expressibility assertion is
unchanged and still passes — 0.00128 degrees clears one 0.001 mm quantum on a
100 mm radius with a factor of 2.2 to spare.

## 10. Certification

Two states are certified, because the round produced two fixpoints and the first
one is what motivated §5a.

**The intermediate, `pinned-fs-155.33041597700.json`** (declared raw
`155.33041597699957`, 1,247 s, `evidence/cert-final.json`): `replayPass: true`,
**138 probe arms**, `belowIncumbent: 0`, `fixpoint: true`. A further 110 arms of
untried compositions (§8's last three rows) were also barren on it. That is the
state tier I was invented against, and it is the reason the round can say what a
certified fixpoint does and does not mean.

**The final, `pinned-fs-155.26442950833.json`** (declared raw
`155.26442950832842`, 1,130 s, `evidence/cert-final2.json`):

| half | what | result |
|---|---|---|
| replay | modes 27, 30 and 22 seeds 0-3 | 6 of 6 `exactValid` **and** `contractValid`, all six reproducing fingerprint `82eaa976…` at **0 ULPs** from the declared raw |
| fixpoint | **132 probe arms**: mode 31 x 4 tiny steps, the eleven-delta flatten grid x 2 slacks -> mode 33 and x 1 slack -> mode 32 under **both ladder generations**, the nudge tier x 16, tier H's twelve-delta grid -> modes 30 and 31, mode 26 x 3 drops x 2 seeds, and mode 34 x 8 step/budget specs x 2 seeds | **0 below the incumbent** |

`replayPass: true`, `belowIncumbent: 0`, `fixpoint: true`. The battery is 138
arms against the record-line round's 36, and the additions are the instruments
that moved this round: the wide entry grid, mode 32, and tier H. A fixpoint claim
that does not probe the tier that produced the descent is a claim about the tiers
that were already exhausted — which is exactly what the 155.33042 certificate
turned out to be, since tier I then took 0.066 mm out of it. The final
certificate has the same shape and therefore the same caveat, stated plainly:
**it certifies the instruments in the battery, and tier I is not yet in it.**

Two independent replays are kept rather than one. The record was produced by the
ten-rung binary, so `evidence/replay-final.json` is the in-family check — and
`evidence/replay-final-basebinary.json` is the stronger one: the **pristine
base-commit binary, which has no knowledge of the 0.00128 rung**, replays the
final layout `exactValid` and `contractValid` on modes 27, 30 and 22 seeds 0-1,
reproducing raw `155.26442950832842` at **0 ULPs** and fingerprint
`82eaa9762e0df399…` exactly. The ladder change is what *found* the state; it is
not needed to *verify* it.

The battery's accepted-rung histogram is the diagnostic that justified this
round's change, and it now reads differently in exactly the way it should. The
previous round's was 27 of 40 at the floor and **zero at 0.02 or coarser**; this
one is 24 at 0.00128, 16 at 0.0032, 11 at 0.008, 10 at 0.02, 8 at 0.05 and 30 at
0.3125, plus 32 mirror-family acceptances. The new floor is heavily used — it is
the single most-used rotation rung — but the distribution now spans the whole
ladder instead of stopping dead above the floor, which is what a floor placed at
the edge of the useful band looks like rather than above it. That, and not
taste, is why the ladder stops here even before §2's arithmetic forbids the next
rung.

## 11. Honest limits

* **The 155 mm goal is not reached.** The gap to 155.000 is **0.236 mm**, down
  from 0.422 mm. The round does not claim a path to it, and it stopped for
  wall-clock reasons rather than at a fixpoint — the last cascade was still
  adopting when it was stopped, at a decaying but non-zero rate. What it does
  claim is that the levers are not exhausted in *kind*: all four were found by
  asking what the state's own diagnostics were saying rather than by widening a
  grid, and three of the four were found *after* a certified fixpoint said the
  previous set was done. A certified fixpoint is a statement about the
  instruments that were fired, and this round fired four sets of them and broke
  four fixpoints.
* **The descent decays inside a basin and the instruments have to be
  re-chosen.** Every cascade in this round started fast and slowed: 25 arms
  below out of 120, then 76 of 198, then 3 of 138. Each stall was broken by a
  different instrument, and each instrument's productive parameter *moved* as
  the state descended — the legalization tier's best flatten delta walked
  0.25 → 0.1 → 0.08 → 0.05. Nothing here supports a fixed recipe; what it
  supports is measuring the frontier stack and sizing the entry grid to it.
* **The new rung is one rung, and the floor is now genuinely at the grid.**
  0.000512 degrees moves the *largest* piece in this request by 0.00083 mm,
  below one pose quantum, so this lever is spent. A further one would need a
  finer pose grid, which is a different change with a different blast radius.
* **The ladder result is one request.** The rung is scale-free and the argument
  is arithmetic, but the *measurement* is mixed-61 at one depth on one lineage.
  Nothing here says anything about shapes-17 or triangle-20, and coordinator
  v2's generality finding applies in full.
* **The basins negative is a negative about these basins.** The ten-rung grid
  found nothing below on 155.4563, 155.4633 or 156.0914 — 66 arms — so the new
  rung did not simply make every state better; it opened one.
* **No wall-clock claim.** The box was deliberately oversubscribed throughout
  (five concurrent cascade arms plus separate sweeps, on 16 cores with the
  engine pinned at 8 threads). Every quality number is a work-budgeted or seeded
  arm and the seconds in the logs are reported only so a reader can see the arms
  were concurrent.
* **Mode 26's determinism under load is still assumed rather than proved.** It
  returned its parent to the digit on every arm this round, under every load,
  but no paired same-arm-under-two-loads check was run on it specifically.
* **Mode 32's productivity is a basin fact, not a correction.** §6 does not
  overturn the record-line round's vertex-cover reasoning; it measures a basin
  where the reasoning's premise does not hold.

## Files

* `drivers/lib.py`, `drivers/drv.py`, `drivers/gatelib.py`, `drivers/gates.py` —
  the record-line drivers with `ROOT` repointed at this worktree.
* `drivers/cascade2.py` — the deferred-credit interleave, with tier-frequency
  knobs and tier H.
* `drivers/flatsweep.py` — the frontier-flatten entry grid × slacks × modes ×
  seeds, concurrently, under a selectable binary.
* `drivers/knudge.py` — the k-deepest-pieces perturbation swept over k, d and
  the repair mode.
* `drivers/frontier.py` — the frontier stack of a pinned layout.
* `drivers/geodiff.py` — the geometric diff between two pins.
* `drivers/lineage.py` — the descent table, generated from the cascade states.
* `drivers/pinit.py` — pin one run document and report its identity.
* `drivers/certify_full.py` — the certification battery, extended with the wide
  flatten grid, mode-32 arms, the nudge tier and mode 34's fine steps.
* `drivers/schedsweep.py`, `drivers/regrid.py`, `drivers/crosssweep.py`,
  `drivers/replay.py`, `drivers/docdiff.py` — unchanged instruments.
* `drivers/run-*.sh` — the exact invocations, with the reason for each
  parameter choice in the header.
* `evidence/` — every driver's own emitted document, unedited.

Reproduce:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental --target-dir target-l10
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,compression-schedule --target-dir target-sched

python3 drivers/gates.py l10 target-l10/release/examples/general_request_benchmark
python3 drivers/flatsweep.py ab <pin-155.4223> 155.42229074464285 \
    0.0005,0.001,0.0015,0.002,0.0025,0.003,0.004,0.005,0.006,0.008,0.01 \
    0.05,2.0 33 5
bash drivers/run-c2e.sh <deadline> <pin> <raw> c2e
python3 drivers/certify_full.py <pin> <raw> cert-final
```
