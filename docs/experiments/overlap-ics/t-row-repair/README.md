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
