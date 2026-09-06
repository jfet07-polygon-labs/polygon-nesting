# The proxy margin: the churn is gone, and it was not the bottleneck

Branch `engine/topology-archive-search`, mechanism commit "The proxy margin: the search targets
strictly inside the Exclusive kernel's region, behind --proxymargin" on `c35b1f6`. Consumed dev
seeds 18-26 only (the virgin seeds 27-35 were not touched). One repetition per cell, ten seconds,
mixed-61 exact-clearance request, 8 workers, both schedule profiles, bench lock held. The
baseline `devbase` is the same engine at `c35b1f6` with the knob off.

## What was asked

The autopsy (commit "Instrument the give-up", `docs/quorum/ics-achieved-depth-v1-spec.md`,
"Result") showed two exact calls in three refusing with "a failing row is outside the 4 um band
or has no sheet slack": the proxy converges pairs to exactly 5.000 mm, the integer-micrometre
kernel reads them one cell short, the one-row repair moves a piece 5 um into a neighbour also
at 5.000 and gives up on the new 5 um shortfall. The mechanism inflates every proxy clearance
by `m` so a proxy-zero state lies strictly inside the kernel's region.

Falsifier, written before the measure: any invalid publication; or the give-up count not
falling by at least half; or a paired-median depth regression on either profile.

## What was measured, `--proxymargin=4`, nine seeds

| profile | give-ups | exact checkpoints / publication | publications | explore bites / cell | invalid |
|---|---|---|---|---|---|
| Legacy  | 1535 -> 3 | 2.84 -> 1.00 | 1042 -> 990 | 98.8 -> 100.7 | 0 |
| Wall10s |  737 -> 1 | 4.35 -> 1.01 |  266 -> 291 |   3.9 -> 3.8 | 0 |

Every other refusal class vanished with it ("piece crosses the strip", "repair would have
enlarged the locked strip", pair refusals): with the margin on, essentially every exact call
publishes. Full per-seed tables: `giveups-m4-seeds18-26.txt`.

| profile | paired median depth gain (devbase - margin) | wins | worst | per seed |
|---|---|---|---|---|
| Legacy  | -0.181 mm | 4/9 | -1.468 | 18:+2.20 19:-0.39 20:+0.95 21:+3.15 22:-0.96 23:-0.71 24:-1.47 25:-0.18 26:+2.22 |
| Wall10s | -0.076 mm | 4/9 | -4.749 | 18:-0.64 19:-2.31 20:-4.75 21:+6.23 22:-0.08 23:-0.88 24:+0.00 25:+0.99 26:+0.56 |

`analyse-m4-seeds18-26.txt` has the medians and means. Quick A/B at `--proxymargin=8` on seeds
18-20 (`analyse-m8-seeds18-20.txt`, `giveups-m8-seeds18-20.txt`): give-ups 588 -> 0 (Legacy)
and 169 -> 0 (Wall10s); depth +0.621 (3/3) on Legacy and -0.218 (1/3) on Wall10s. Three seeds
say nothing about depth; they say the same thing about the churn.

## Where the freed time went (work counters, nine Legacy cells summed)

| counter | devbase | margin 4 |
|---|---|---|
| sample evaluations | 393.0 M | 415.4 M (+6 %) |
| relocates | 1.548 M | 1.648 M (+6 %) |
| master iterations (weight updates) | 24 253 | 18 108 (-25 %) |
| relocates per master iteration | 64 | 91 |
| exact checkpoints | 2961 | 993 |
| repair rows | 4756 | 37 |
| disruptions | 304 | 72 |

The failed exact calls cost about five per cent of the wall; the margin gives that back as
evaluations, and spends it again: the rows it activates (every contact at 5.000-5.004 mm) make
each master iteration relocate more pieces, so a bite takes fewer iterations of a longer kind
and the bite rate does not move. Depth at ten seconds is set by the bite rate, and the bite
rate is set by the separator's work per bite: about 1 740 relocates and 440 000 sample
evaluations per 0.1 % bite on Legacy, twenty-eight relocates per piece. That is the number
the next mechanism has to attack, not the exact call.

## Verdict against the falsifier

- Invalid publications: 0 of 1281. Passes.
- Give-ups halved: 1535 -> 3 and 737 -> 1. Passes by three orders of magnitude.
- Paired-median depth on either profile: -0.181 and -0.076 mm, 4/9 wins each. **Trips**, by
  amounts inside the per-seed noise (+6.2 to -4.7 mm on Wall10s). The mechanism does what it
  says to the exact path and does nothing to depth.

The margin stays in the tree as an opt-in knob (default 0, bit-identical), because it turns the
exact path from a lottery into a certainty and any later mechanism that raises the bite rate
will want it. The signed round `ICS-proxy-margin-v1` that GPT-6 Astra drafted
(`docs/astra-review-2-the-proxy-margin.md`, Q4) is **not opened**: its preflight requires
`P_M >= P_R` on every seed, and the dev cells already fail it on six Legacy seeds (publications
per cell fell with the churn, because a bite that certifies at its first exact call publishes
once where the old path sometimes published twice), and its depth clauses have no dev evidence
behind them. Spending the virgin seeds on a mechanism that is neutral on the consumed ones
would burn the population for nothing.

## Verifier's six non-refuting defects, recorded

1. At `m = 4` a layout with every pair at exactly 5.000 mm has proxy max violation 0.004, equal
   to the band, so the band still admits the old edge states to the exact call; the give-up
   reduction comes from descent continuing past them. Astra's Q2 makes the same point and
   prefers `m = 8`.
2. `cluster_budget.rs` still budgets disc mass at the contract clearance while the proxy rows
   charge `pair + m`; the feasibility budget is marginally optimistic under the knob.
3. The Wall10s regression at `m = 4` on seeds 19-20 in the quick A/B (-3.8 median on three
   seeds) reproduced in the nine-seed measure on the same two seeds (-2.31, -4.75) and is offset
   by +6.23 on seed 21.
4. Under the knob `firstPairProxyViolationUm` reports the margin-inflated row; comparisons with
   the `c35b1f6` seed-20 autopsy must subtract `m`.
5. A document from a margin run replayed without `--proxymargin` reconstructs a different Phi;
   `proxyMarginUm` is emitted so a reader can restore it, none does automatically.
6. The knob and its test are process-global, like `PublishAchievedGuard`; not observed to
   interfere in the 844-test run.

## Provenance

Two workflow agents: `wf_527557c3-cc9` (implementer killed with the session at commit WIP
`5d818e3`), `wf_0cd55ba4-b99` (resumed from it: reviewed every line, wrote the test, built,
bitcheck ALL_IDENTICAL against the parent build on seeds 0/3/7, suite 844 passed, quick A/B at
4 and 8 um; adversarial verifier: no forbidden-rescue row tripped, nothing copied from Sparrow,
validator untouched; measurer: the nine-seed cells). The give-up counts were computed by
`giveups.py` here, because the workflow's measure schema did not ask for them; the schema now
does.
