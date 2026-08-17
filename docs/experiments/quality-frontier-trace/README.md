# Quality frontier trace

The measurement the third adversarial review named as missing: *when* does this
engine reach each depth, how much work has it done by then, and which operator
produced the layout that got it there — measured in **one process, from the
request only**, with no pinned parent anywhere.

Review, verbatim: "Until that curve exists, a ten-second portfolio allocation is
informed engineering — but still storytelling."

## The instrument

`quality-trace`, a cargo feature on `polygon-nesting-core`, off by default and
empty when off. Build:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,quality-trace
```

Run with `POLYGON_NESTING_QUALITY_TRACE=<path>.jsonl`. One JSON object per
line; every line carries `seq`, `t` (seconds since the measured stream
started), `thread`, the enclosing operator scope, that scope's seed and parent
fingerprint, and a snapshot of the work counters. The `event` field
discriminates `run` / `scopeEnter` / `scopeExit` / `exactCandidate` /
`incumbent` / `publication` / `modeResult` / `end`.

The unit is one **exact-valid candidate**, not one public incumbent. There is
exactly one instrumentation choke point for it —
`search::general_fast::validate_and_measure_placements`, the composite exact
validator every mode's acceptance goes through — so no operator can be
forgotten and none needs its own copy of the rule.

Three knobs, all read from the environment so the pinned positional CLI
contract every replay driver depends on never changes meaning:

| variable | effect |
|---|---|
| `POLYGON_NESTING_QUALITY_TRACE` | the JSONL sink path; unset means no trace |
| `POLYGON_NESTING_QUALITY_TRACE_COUNTERS=0` | leaves `profiling` recording alone, so the clock is undistorted and the work ordinals read zero |
| `POLYGON_NESTING_UNPINNED_VACANCY_PARENT=1` | lets the pinned-parent mode band descend from the coupled arm this same process produced |

## What it costs

Paired interleaved A/Bs on the mode-22 gate stream, arms alternating order every
round, statistic the per-round paired ratio (`evidence/ab-*.json`):

| comparison | paired median | spread | rounds below 1.0 | outcomes identical |
|---|---:|---|---:|:--:|
| base -> feature in, sink closed | **1.014** | 0.980-1.044 | 4/10 | yes |
| sink closed -> sink open | 1.165 | 1.157-1.184 | 0/10 | yes |
| sink closed -> counters only, no sink | 1.168 | 1.156-1.195 | 0/10 | yes |

Read the three rows together: the feature is free when off, and the cost when on
is the **profiling counters**, not the trace. Formatting and writing the events
is not separable from noise once the counters are already armed. That is why
`POLYGON_NESTING_QUALITY_TRACE_COUNTERS=0` exists and why the plotted curve uses
it — a time-to-quality curve should be drawn on the clock the production build
runs on.

## What it measured

Four processes, Mixed-61 exact-clearance request, from request only, production
default search-offset allowance, `threads=8`, `relaxed-epochs=24`,
`coupled-dynamic-separator=1`. Mode 20's construction clamp is derived from the
request rather than pinned: 2× the request's own area lower-bound depth
(130.399 mm → 260.797 mm).

### Time to quality

Undistorted-clock runs. Depths are raw source depth, joined to the incumbent by
placement fingerprint.

| | m0+coupled seed 0 | m0+coupled seed 1 | mode 20 seed 0 | mode 20 seed 1 |
|---|---:|---:|---:|---:|
| first complete exact-valid layout | 0.535 s @ 231.570 | 0.546 s @ 231.570 | 0.541 s @ 231.570 | 0.546 s @ 231.570 |
| ≤ 200 / 190 / 185 mm | 0.666 s @ 182.976 | 0.672 s @ 182.976 | 0.669 s @ 182.976 | 0.655 s @ 182.976 |
| ≤ 182 mm | 0.980 s | 0.917 s | 0.977 s | 0.883 s |
| ≤ 181.6 mm | 1.249 s | 1.148 s | 1.254 s | 1.121 s |
| ≤ 180 mm | never | 1.350 s | never | 1.326 s |
| final engine depth | 181.589 | 179.690 | 181.589 | 179.690 |
| run end | 1.956 s | 1.981 s | 26.617 s | 26.960 s |
| tail with zero incumbent gain | 0.707 s | 0.630 s | **25.363 s** | **25.635 s** |

The whole quality curve is over inside **1.4 seconds**, on both seeds, in all
four runs. Everything after that is flat. Neither seed reaches 175 mm; seed 0
never reaches 180 mm at all.

A second, independent ten-round set of the same three A/Bs, run earlier on
the same box, reported 0.995 / 1.155 / 1.175 - both sets are in
`evidence/` and `evidence/round1/`.

### Marginal Δmm per second

| run | gain | window | inside window | over whole run |
|---|---:|---:|---:|---:|
| m0+coupled seed 0 | 1.387 mm | 0.583 s | 2.378 mm/s | 0.709 mm/s |
| m0+coupled seed 1 | 3.286 mm | 0.678 s | 4.845 mm/s | 1.659 mm/s |
| mode 20 seed 0 | 1.387 mm | 0.585 s | 2.372 mm/s | **0.052 mm/s** |
| mode 20 seed 1 | 3.286 mm | 0.671 s | 4.897 mm/s | **0.122 mm/s** |

The "window" is first public incumbent to last; "over whole run" divides the
same gain by the run's whole elapsed time.

The mode-20 rows are the same search as the m0 rows — the work-ordinal runs
agree with them counter for counter through the m0 phase — plus a 25-second
tail. The tail's marginal contribution to the published result is exactly
0.000 mm.

### Where the work went

Per-scope ledger, mode-20 seed 0, work-ordinal run (`summary.json` →
`curves.<tag>.topScopesBySeconds`):

| scope | wall | candidate queries | effective moves | exact pair tests | collision builds | exact-valid candidates |
|---|---:|---:|---:|---:|---:|---:|
| `constructor` | 0.648 s | 0 | 0 | **584,671** | 2,913 | 1 |
| 16x `m0.epoch*` | 1.193 s | 5,332,423 | 48,153 | 207 | 0 | 3 |
| 3x `coupled.*` | 0.334 s | 519,110 | 4,160 | 63 | 0 | 0 |
| 8x `mode20.restart*` | 24.676 s | 0 | 0 | 458\* | 0 | 8 |

\* The deep operators' own Clipper counters are behind `search-profiling`, which
costs about 4.5% of a mode-20 stream and was therefore left off; 458 is the
non-deep total only. Every event says so, in the run header's
`deepCountersCompiledIn`.

Three things this table says that no previous artifact did:

1. **The constructor owns 99.87% of the run's exact pair tests** and spends them
   in its first 0.648 s. Optimizer-internal exact geometry is not the m0
   stream's problem; constructor exact geometry is the whole of it.
2. **The relaxed loop's 5.3M candidate queries and 48.2K effective moves buy
   1.4–3.3 mm**, and they buy it in under one second of wall time.
3. **Mode 20's eight restarts cost 3.0–3.2 s each and produce exactly one
   exact-valid complete layout each** — eight basins at 204.070–217.202 mm on
   seed 0 and 204.272–228.112 mm on seed 1, every one of them deeper than the
   181.589/179.690 mm incumbent they were built alongside, and the adoption
   rule refuses all eight (`publication` event,
   `reason: notStrictlyBetterThanLegacy`).

The constructor's share is 584,671 of the run's 585,460 exact pair tests —
99.87% — and 2,913 of its 2,913 collision polygon builds. Mode 20's eight
restarts occupy 24.676 s of the 26.851 s the leaf scopes account for, out
of the run's 26.882 s.

## Two findings the trace produced by existing

**Mode 20 has no from-request path.** `run_population` refuses an unpinned
parent for the whole 9–21/25 band before it does any work, so no single process
could measure a from-request mode-20 basin at all. The review's ten-second
portfolio allocates 1.9–4.0 s to exactly that. It is now reachable behind
`GeneralRelaxedSettings::persistent_vacancy_allow_unpinned_parent`, off by
default, reported in the result document when armed, and explicitly not
quotable against any pinned number.

**Adoption refusals are now named.** The review's second finding was that "every
adoption rejection silently returns legacy; production telemetry cannot
distinguish incomplete, invalid, envelope-only rejection, or non-improvement".
Under the trace the four refusals are four distinct `publication` events
(`incompleteCardinality`, `publishedDepthUnmeasurable`,
`notStrictlyBetterThanLegacy`, `compositeValidatorRejected`).

## What this does *not* measure

The review asked for two plots. This is the first one. The second — "structurally
diverse archived basins versus time, with their eventual descendant depth under
a fixed downstream work budget" — needs a downstream descent from each archived
basin, and this trace supplies its input (eight fingerprinted, depth-measured,
exact-valid mode-20 basins per seed with their creation timestamps) without
supplying its answer. Quoting a descendant depth for them would be exactly the
storytelling the review objected to.

## Reproducing

```
python3 drivers/gates.py base   <base-binary>
python3 drivers/gates.py traced <trace-binary> --trace <sink-dir>
python3 drivers/ab.py 10 base <base-binary> traceoff <trace-binary>
python3 drivers/frontier.py both
python3 drivers/curves.py
python3 drivers/summarize.py
```

`drivers/lib.py` carries the pinned CLI tail; point `ROOT` and `BIN_DIR` at your
worktree.

## Artifacts

| path | what |
|---|---|
| `frontier.png` | the curve |
| `summary.json` | milestones, marginal Δmm/s, work attribution, overhead A/Bs, gate evidence |
| `summary-measured.json` | the full per-run derivation |
| `curves/curve-<tag>.json` | one run's incumbent series, candidate series and scope ledger |
| `traces/<tag>.jsonl` | the raw event streams |
| `evidence/` | the paired A/B summaries and both gate runs |
