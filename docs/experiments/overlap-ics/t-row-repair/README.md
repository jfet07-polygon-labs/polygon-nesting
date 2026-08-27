# The T-row repair, built — and the row it cannot afford

The specification is [`../../../t-row-repair-spec.md`](../../../t-row-repair-spec.md)
at sha256 `bce5c8bd...`, signed by Sol (`CONFIRM`) and Grok (`AMEND` on §4,
folded in). This directory holds the implementation's first evidence.

**Nothing here is a Gate 0 verdict.** Gate 0 requires the residual fork of the
Round-4 composed deterministic ten-second trajectory, complete with pacer
charge and stream ordinals, and that driver is not built yet. These are
full from-request wall runs, which the round's own errata already record as
non-deterministic run to run. They are reported because the *structural* result
is far outside that variance and because the no-failure-without-autopsy rule
requires the mechanism to be understood before any verdict is proposed.

## What was built

`t-row-repair` (off by default) turns the strip top into a **tightened far-`y`
boundary of the kernel box**: `T - depth_top_inset_mm() + expansion_mm()`, in
the same convention `inset_box` uses, and `inset[3]` becomes the tighter of
that and the physical sheet. Nothing downstream changed - `scan`,
`boundary_admissible`, `critical_boundary_radius_micron` and `repair_one_row`'s
binding-side rule then treat a piece proud of the strip as an ordinary failing
boundary row and push it inward under the same 4 um guard, the same 16 um
per-piece cap and the same `4n` row budget. `attempt`'s entry gate is relaxed
only for `0 < proxy_depth - T <= band`, and the final `published_depth > T`
refusal is untouched, so nothing above the target can publish.

Three arms, selected at runtime (`--trow=off|repair|computeignore`), because
Gate 0 runs one arm per process.

## It works, and it is legal

Nine seeds, 10 s wall, control against repair:

| seed | off: depth / bites | repair: depth / bites | eligible | **conversions** | refused | delta |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 169.3656 / 74 | 167.4114 / 74 | 173 | **12** | 159 | **+1.9543** |
| 1 | 179.0810 / 21 | 179.0810 / 21 | 67 | 0 | 67 | -0.0000 |
| 2 | 165.2128 / 93 | 167.3896 / 87 | 425 | **6** | 419 | -2.1768 |
| 3 | 165.4239 / 99 | 170.0675 / 71 | 124 | **4** | 120 | -4.6435 |
| 4 | 179.0812 / 21 | 170.8000 / 63 | 580 | **6** | 573 | **+8.2812** |
| 5 | 175.0116 / 44 | 175.0234 / 44 | 159 | **1** | 158 | -0.0118 |
| 6 | 169.1906 / 74 | 173.5709 / 41 | 226 | **8** | 170 | -4.3804 |
| 7 | 179.0821 / 21 | 179.0821 / 21 | 93 | **0** | 93 | -0.0000 |
| 8 | 179.0821 / 21 | 179.0821 / 21 | 192 | **0** | 192 | +0.0000 |

**37 conversions, zero invalid publications.** The mechanism does what §1 says:
states that today die at `publish.rs:364` are pulled under `T` by the existing
repair, certified by the Exclusive kernel and accepted by the untouched
contract validator. Seed 4 - a frozen seed - goes from 21 bites at 179.0812 to
**63 bites at 170.8000**, which is the escape the whole diagnosis predicted.

Seeds 2, 3 and 6 finish worse. A conversion costs time and moves the
trajectory, and on a wall clock that is a different run, not a regression
measurement; the paired claim belongs to Gate 0's fixed residual, not here.

## The clause it fails, and exactly why

§3 clause 3 requires conversions on **both** seeds 7 and 8. They do not
convert - not at 10 s, and not at the 30 s clock the specification aims at:

| seed | eligible | converted | blocked on **pair** | on boundary | on displacement cap | on row budget |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 7 (30 s) | 4284 | **0** | **4284** | 0 | 0 | **0** |
| 8 (30 s) | 1527 | **1** | 1518 | 0 | 0 | **0** |

Read the columns. **No refusal is a boundary row**: the T-row itself always
clears. One refused checkpoint reads `repairRows: 11`,
`repairMaxDisplacementMm: 0.008`, `repairDepthGivebackMm: -0.004551` — eleven
corrections, eight micrometres spent, and the depth pulled 4.55 um *below* a
target it was 1.6 um above. The T-row does its job.

What refuses is a **pair** row that the T-row's own push created, and its
shortfall lands in the 8-16 um band:

| seed | shortfall of the blocking row | <=4 um | <=8 | **<=16** | <=32 | <=64 | >64 |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 7 (30 s) | | 812 | 0 | **2590** | 882 | 0 | 0 |
| 8 (30 s) | | 12 | 41 | **1459** | 6 | 0 | 0 |

`repair_one_row` refuses any single row whose shortfall exceeds
`guard_micron = epsilon_grid_mm = 4 um`, and returns `None` for the whole
repair rather than taking a partial step. **The row budget is never
exhausted** - `blockedRowBudget = 0` on every seed - so the repair gives up
with all `4n = 244` of its rows unspent, because one row wants twelve
micrometres and it will only ever grant four.

**Honesty about the diagnosis.** The `<=4 um` bucket - 812 rows on seed 7 - is
*not* explained by the guard, since those shortfalls are inside it. The autopsy
re-derives `repair_one_row`'s first-failing-row choice and its closed-form
criticals but does not reproduce its later `sheet_slack` test, so those rows are
unattributed. The dominant mass, 2,590 of 4,284 on seed 7 and 1,459 of 1,527 on
seed 8, is squarely the guard.

## The question this hands to the quorum

The T-row asks the repair to absorb a pair violation of 8-16 um. The repair's
per-row guard is 4 um and its per-piece cap is 16 um, so the design already
contemplates a piece accumulating sixteen micrometres **across rows** while
refusing twelve **in one row**. Whether that asymmetry is the repair's declared
competence - a micro-corrector must not become a solver - or an inconsistency
in it, is not a call this directory makes. It is the "bug or paradigm?" charge,
and it goes to both reviewers before any verdict is proposed. The specification
freezes the 4 um guard, so under it as written this is a pre-declared miss.

## Reproducing

```
cargo build -p polygon-nesting-core --release \
    --features overlap-ics,t-row-repair,ics-publish-census \
    --example overlap_ics_benchmark
target/release/examples/overlap_ics_benchmark --cell=cutclose \
    --request=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json \
    --edge=5 --pair=5 --mode=wall --wall=30.0 --orders=1 --workers=8 \
    --arm=control --trow=repair --seed=7
```

## How deep is the cascade — a pure observation

The autopsy above says the repair quits on a pair row it cannot afford. It does
not say whether that row is one bolt or a wall. This adds the count, observed at
the give-up point and acted on by nothing: how many pair rows are still failing,
and what their outstanding shortfall totals (`evidence/casc-*.json`, 30 s wall).

| seed | conversions | failing pairs = 1 | = 2 | 3-4 | 5-8 | 9-16 | >16 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 7 | 0 | **1171** | **1864** | 600 | 6 | 8 | **0** |
| 8 | 0 | 110 | 150 | 521 | 435 | 16 | **0** |
| 4 | 7 | 500 | 127 | 143 | 114 | 31 | **0** |

Outstanding total pair shortfall at the same moment, in micrometres:

| seed | <=16 | <=32 | <=64 | <=128 | <=256 | >256 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 7 | **1968** | 814 | 853 | 14 | 0 | 0 |
| 8 | 113 | 584 | 91 | 444 | 0 | 0 |
| 4 | **620** | 259 | 36 | 0 | 0 | 0 |

Failing *boundary* rows at give-up are in the lowest bucket - at most one - on
every seed, which is the same fact section above records from the other side:
the T-row clears.

**The obstacle is bounded and small.** On seed 7, 3,035 of 3,649 give-ups have
**one or two** failing pair rows, and 1,968 of them have a total outstanding
shortfall of sixteen micrometres or less. None of the three seeds ever gives up
with more than sixteen failing pairs. The repair stops one or two rows short,
with all `4n = 244` of its rows unspent.

This directory records that and stops. Whether "one or two rows short with the
budget untouched" makes the 4 um per-row guard a declared competence or an
inconsistency is the charge put to both reviewers, and it is theirs to answer.

## Both reviewers: paradigm, not a bug — and two instrument defects in the build

The no-failure-without-autopsy charge went to both. They agree on the verdict
and both refute the "unused budget" framing.

**The 4 um guard is a validity domain, not a step size.** `EPSILON_GRID_MM` is
`2 * ceil(sqrt(2) * 1 um)`: the most `GridSet::of` can move two rings toward
each other. A row that is 12 um infeasible is not a proxy-versus-exact
disagreement at all, and `repair_one_row` classifies it as outside competence.
The per-piece 16 um cap does a different job - it lets one piece absorb about
two independent in-band corrections as rows interact - and does not license
sixteen micrometres of packing in a single row. Applying `min(shortfall, 4 um)`
and re-scanning would turn the guard into a step size and the micro-corrector
into a small PGS, which the module's own tests forbid: a 3 um deficit publishes,
a half-millimetre deficit is discarded *even after the attempt band is widened*,
so that the repair is what refuses.

**Three corrections to this directory's earlier prose, all fixed above:**

1. `blockedRowBudget = 0` means "never exhausted", **not** "all 244 rows
   unused". The eleven-row witness had already spent in-band rows and then met
   an out-of-band pair. Leftover budget is the classifier firing, not a stuck
   loop.
2. `blockedDisplacementCap = 0` is **not** unused headroom - it is the guard
   firing first. A 12 um pair shortfall would demand `correction = 16 um`, and
   the T-row has already spent 6-8 um on the pieces that created it; the entire
   16-32 um bucket (882 of seed 7's 4,284) is already over the cap on its own.
3. Seed 8's `published: 1` in the first table was **not** a bite-22 conversion.
   `exploreBites` stayed at 21 and the depth at 179.007: it was a compress
   publication. Clause 3 is "publishes bite 22".

**And two instrument defects in the implementation**, both Sol's, both real,
both repaired here before any Gate 0 is run:

- **The T-row was quantization-lossy.** Tightening the kernel box's far-`y` was
  algebraically right and instrumentally wrong: `raw_source_depth_mm` reads
  unquantized `f64` rings while both the point and the box round to the nearest
  micrometre, so a positive continuous overhang can vanish on the grid. Seed 8
  showed it - 1,527 eligible states, only 1,510 with a first-scan row - which is
  the condition the specification's clause 2 calls a wiring `AUTOFAIL`. The
  strip top is now **its own continuous row**, measured where the publication
  gate is measured (`piece_bounds.max_y + offset_y` against
  `T - depth_top_inset_mm()`), with an explicit `[0, -1]` direction, the same
  `shortfall + guard` formula every boundary row uses, the same frozen guard,
  cap and `4n` budget, re-measured after every correction, and the physical
  sheet box left untouched.
- **Neither arm memoized the eligible digest**, so one proud layout paid the
  whole repair as many times as the descent revisited it, which makes clause 2's
  "invokes T-repair exactly once" unmeasurable and inflates `ComputeIgnore`'s
  cost clause. An eligible digest is now offered once and repeats are logged and
  skipped.

**After both fixes, 30 s wall, the result is unchanged and the instrument is
clean** (`evidence/fix-*.json`):

| seed | depth / bites | eligible | with T-row | wiring | conversions | refused | repeats skipped | blocked on pair |
| ---: | ---: | ---: | ---: | :---: | ---: | ---: | ---: | ---: |
| 7 | 179.0821 / 21 | 4109 | 4109 | **OK** | **0** | 4109 | 25 | 4109 |
| 8 | 179.0821 / 21 | 1849 | 1849 | **OK** | **0** | 1849 | 47 | 1849 |
| 4 | **165.1705 / 100** | 844 | 844 | **OK** | **17** | 823 | 110 | 813 |

`eligible == eligibleWithTRow` exactly on every seed: the wiring clause is now
clean, and the continuous row sees every overhang the gate sees. Seeds 7 and 8
still convert zero. Seed 4 - frozen at 179.0812 with 21 bites under the closed
member - reaches **165.1705 with 100 bites**, under the 168.484 bar.

What remains is Gate 0 itself, on the frozen residual rather than a wall clock.
Both reviewers hold the specification as written: if seed 7 or seed 8 does not
close bite 22 there, the mechanism is closed and the pair cascade is the causal
result rather than an out-of-competence input. The 37 conversions and seed 4 do
not re-aim the gate afterwards.
