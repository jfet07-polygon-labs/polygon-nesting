# Gate 0 — the verdict

**`GATE0_PASS: false`, on a valid instrument.** Two of the nine clauses are red
and both are pre-declared closures. Under
[`t-row-repair-spec.md`](../../../../t-row-repair-spec.md) §3 the T-row repair is
**closed** and no quality battery runs.

Aggregate: [`evidence/gate0.json`](evidence/gate0.json). Driver:
[`gate0.py`](gate0.py). Every arm-and-seed cell is beside it.

## The instrument, and how its residual was chosen

The specification asks for the residual explore allocation of the Round-4
composed deterministic **ten-second** arm at bite 22. That arm's own cells give
it: the five frozen seeds each spend **one retry attempt and 821-1,155 master
iterations** there before the budget ends, and the four that close spend 0-1
attempts and 131-1,424 iterations. (The 7,262-iteration figure quoted in the
autopsy is the *thirty*-second cell, not the ten-second one.)

Fixed-work mode reproduces that residual without a clock. The setting was
calibrated **on the control alone, before any treatment cell was run**, against
the specification's pre-declared partition. The whole sweep, so nobody has to
take it on trust:

| attempts | iters | control closes bite 22 | control leaves open |
| ---: | ---: | --- | --- |
| 12 | 600 | 0,1,2,3,4,5,6,8 | 7 |
| 2 | 700 | 2,3,5,6 | 0,1,4,7,8 |
| 2 | 600 | 2,3,4,6 | 0,1,5,7,8 |
| 1 | 900 | 3,6 | 0,1,2,4,5,7,8 |
| 1 | 1200 | 3,6 | 0,1,2,4,5,7,8 |
| **2** | **500** | **0,2,3,6** | **1,4,5,7,8** |

`attempts = 2, iters = 500` reproduces `{0,2,3,6}` closed and `{1,4,5,7,8}` open
exactly, and Gate 0 confirms it contemporaneously. **`instrumentPartition:
true`** — this is a mechanism result, not an invalid instrument.

The 12x600 row is worth keeping for its own sake: given twelve retry attempts
the **closed member** closes bite 22 on eight of nine seeds. Bite 22 is not
impossible for the frozen engine; at the ten-second budget it simply does not
get the attempts.

## The clauses

| clause | result |
| --- | :---: |
| instrument partition reproduces | **PASS** |
| per-bite census integrity | PASS |
| unique install (`eligible == eligibleWithTRow`) | PASS |
| causal witness | PASS |
| authority and caps (`<=16 um`, zero invalid, `raw_depth <= T`) | PASS |
| two-process bit identity, all three arms, all nine seeds | PASS |
| `ComputeIgnore` isolation from `Control` | PASS |
| **tail-relevant conversion** | **FAIL** |
| **no reverse** | **FAIL** |

**Conversion: zero.** Not merely seeds 7 and 8 - **none** of `{1,4,5,7,8}`
closes bite 22 under the T-row at the round's own residual.

**Reverse: seeds 2 and 3.** Both *lose* the bite-22 publication the control
makes:

| seed | control bite 22 | control depth | repair bite 22 | repair depth | delta |
| ---: | :---: | ---: | :---: | ---: | ---: |
| 0 | published | 178.9851 | published | 178.9851 | 0.0000 |
| 2 | published | 178.9831 | **not published** | 179.1692 | **-0.1861** |
| 3 | published | 178.9666 | **not published** | 179.1691 | **-0.2025** |
| 6 | published | 178.9554 | published | 178.9554 | 0.0000 |

## Why the reverse happens, and it is the mechanism rather than a defect

**The T-row is not confined to bite 22.** Bites 1-21 publish with
`proxy_depth <= T`, but *inside* each of those bites the descent passes through
many states that are momentarily proud of that bite's own target, and those are
eligible. The census shows it: every seed records eligible states, including the
four that were never frozen.

So the repair arm publishes the *first* layout it can scrape under `T` -
including ones at `proxy in (T, T+4 um]` that the closed member refuses and
searches past - and the closed member goes on to find a better one at the same
width. A worse parent then compounds into the next bite, because the next width
is the achieved depth. On seeds 2 and 3 that costs 0.19 and 0.20 mm and the
bite-22 publication itself.

That is the mechanism's own behaviour under the frozen rules, measured. It is
not an instrument failure, and the specification does not permit confining the
relaxation to a single ordinal after the fact.

## What is closed, and what is not

Closed, by §3's pre-declared reading: **the depth lip is not legalizable with
the existing frozen-θ repair.** Both reviewers had already ruled the 4 um
per-row guard a validity domain rather than a step size, so the 8-16 um pair
cascade the T-row creates is outside the repair's competence by construction,
and the gate now shows it costs publications even where the lip was not the
binding problem.

Not closed, and not licensed either: Sol names an exact contact-chain repair,
joint T-and-pair, pre-committed and still bounded by 16 um per piece and `4n`
rows, as the only direction this evidence could support; Grok holds that nothing
is licensed and that a bounded-step or 16 um-per-row repair would be a packing
legalizer this repair exists to refuse. That divergence is a new round's
business. The 37 wall-run conversions and seed 4's escape do not re-aim this
gate afterwards, and the ten-second gate stays retired.

## Regression floor at the verdict

Fresh build, after the T-row is compiled in and switched off by default:

- four pinned regression gates: **4/4 reproduce**, `ALL_PASS: true`;
- default workspace suite: **1,104 passed, 0 failed**;
- `overlap-ics` suite with and without `t-row-repair`: **839 passed, 0 failed**
  each.

One honest note about the first run of the workspace suite: it stopped at
`search::layout_scorer::tests::free_material_multi_eviction_shrinks_retained_container_capacity`,
whose assertion is `cache.entries.capacity() < entries_capacity_before`. That
test passes three times out of three in isolation and the suite is green on a
re-run; it is an allocator-capacity assertion that is sensitive to parallel test
execution, it is in a module this work does not touch, and it is recorded here
rather than quietly re-run.
