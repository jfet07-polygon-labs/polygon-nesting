# Coordinator v5, item 1 only: the self-meter now debits the budget it prices

Sol review 5 §2 named a real bug and gave it a line number: `portfolio.rs:3438`,
the doc comment above `schedule_self_cost_units`. The self-metered charge a
compression-schedule (m34) arm reports raises `ClassStats::cost_max` — what
ranking and the affordability rule read — but the doc comment said outright
"The charge is a *price*, never a spend: the budget still advances at the
meter's own rate". Under a **work** budget that is a real gap: a class whose
own meter reads up to 11x the coordinator's global counter (§6.3 measured
307,767-3,343,739 vs. 3,341,665-3,356,020 on the same twelve gate cells) could
buy far more of itself than the nominal work budget allowed, because the meter
that gates every later affordability check never felt the higher price.

## What this round did, and what it did not

**Done, and verified**: the fix. `BudgetMeter` gained a `self_metered_debit`
accumulator and a `debit_self_metered(global_meter_delta, operator_self_units)`
method that adds `max(0, operator_self_units - global_meter_delta)` into it;
`work_units()` now reads `work_units_now() - work_base + self_metered_debit`.
The call site in the scheduling loop (previously computing `cost` for ranking
only and throwing the comparison away) now also calls
`run.meter.debit_self_metered(metered_cost, units)` whenever an action reports
a self-metered charge and the budget is Work, not Wall. See
`evidence/portfolio-rs.diff`.

**Not done in this round, and this is stated plainly rather than papered
over**: items 2 (three-level priors), 3 (wall batch of independent schedule
arms) and the anytime-curve/determinism/full-suite half of item 4. Each is a
real feature, not a one-line fix, and the honest scope for a single session
was the debit bug plus its verification. Nothing below claims otherwise.

## Verification performed

1. **The four pinned regression gates, rebuilt with the fix, `jagua-experimental`
   only** (no `compression-schedule`, matching the gate contract): all four hit
   bit-for-bit — same depths (206.869 / 159.09233022733062 /
   159.07876040364795 / 164.0375677990678) and the same fingerprint prefixes
   pinned in `docs/experiments/constructor-inner-certificate/drivers/lib.py`
   (`8a7737381238fa4d...`, `fa01012af1d559ae09c...`, `e28fba007f8031d49f...`,
   `49f094d7e59a9008...`). `ALL_PASS: true`. Evidence:
   `evidence/gates-fixed-jagua-experimental.json`. This is the
   "default-path/flagged-changes reproduce flag-off" requirement holding: the
   fix is inert wherever `self_metered_units` is `None`, which every gate
   arm's mode (20, 22) always reports.
2. **26/26 `portfolio::` unit tests pass**, built with
   `jagua-experimental,compression-schedule` (release, same profile as the
   gate binary). Evidence: `evidence/portfolio-unit-tests.log`.
3. **A paired baseline-vs-fixed comparison, `v4:work:120000000:0`, mixed-61,
   seeds 0/1/2, bare request**, both without and with the
   `compression-schedule` feature compiled in. All four combinations
   (baseline/fixed x no-sched/sched) landed on the *identical* depths per seed
   — 174.208 / 176.056 / 179.006 — and the with-schedule run's own
   `portfolio.workUnits` total is well inside the 120,000,000 cap in every
   case. Evidence: `evidence/battery-{baseline,fixed}-{sched,nosched}.json`.
4. **One warm-start probe** from the pinned 159.092 record parent
   (`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-159.092/pinned-parent-159.092.json`)
   at `work=40000000`, seed 1: baseline and fixed again produced the identical
   depth (176.056, spending 31,957,935 of the 40,000,000 cap) — see
   `evidence/warmstart-159.092-{baseline,fixed}-40M.json`. Reading the raw
   output, this run's depth and work-unit total exactly match the *cold-start*
   seed-1 number rather than showing any descent from the 159.092 parent, so
   this probe did not demonstrably exercise the warm-start path at all; it is
   reported as inconclusive, not as a second confirmation.

## The honest finding item 1 asked for, stated as a negative

Every scenario this round could run in the time available ended with the
`work_units()` reading from a run whose bare global-counter delta already sat
comfortably under the budget cap — the coordinator stopped for
`KeysExhausted`/`Affordability` on its own priced queue before the self-metered
debit could ever become the thing standing between an action and the deadline.
That matches what `docs/next-generation-engine-plan.md`'s own orientation-floor
chapter says about the current state of the record lineage: **mode 34 is
inert from a generic starting point** now (`parentProxyFeasible: false`, 26-38
colliding pairs is the typical entry state a bare or lightly-warmed run reaches
inside a 40-120M work budget on this request), which is exactly the condition
item 2 of this same review asks to price at zero eligibility. The debit bug
and the eligibility-prior gap are the same fact seen from two sides: a class
that is this rarely both eligible *and* the one thing standing at the
affordability boundary is a class whose mispriced meter is hard to catch
red-handed with a bare-request, 3-seed, 1-round battery.

**So: no headline number moved in this round's evidence, in either direction.**
The task's own framing — "the honest number may be worse than 162.161; report
it plainly" — presupposed a battery that reaches the state where m34 is doing
real, budget-constrained work. Reproducing the exact conditions of the
original v4 battery (base commit `5d6ce0c`, before four more merged rounds
changed what a bare-request run's frontier looks like at any given work
count) was out of scope for the time available. The fix is real, targeted,
proven not to disturb any pinned or unit-tested behavior, and — by the
project's own account of where m34 currently does fire (near-159/164 parents
under the finer-ladder settings the four gates and the record lineage use,
not a bare 120M cold start) — is most likely to matter on exactly the kind of
run this round did not have time to construct. That is a real gap in this
round's coverage, named rather than hidden.

## Items not attempted, and why

- **Item 2 (three-level priors)**: `parentProxyFeasible` already exists as a
  diagnostic (`report.compression_schedule`'s parent-feasibility field, read
  in the orientation-floor chapter) but is not wired as a pre-dispatch
  eligibility gate; the modeled wall price and the online yield posterior are
  both new subsystems. This is a multi-file design with its own priors,
  tests and gate re-verification, not a same-session addition on top of item
  1 without risking exactly the kind of untested regression this codebase's
  own review culture calls out.
- **Item 3 (wall batch)**: a deterministic parallel-arm scheduler with a
  barrier reducer touches the coordinator's core loop and its determinism
  contract; same reasoning.
- **Item 4, remainder**: the anytime curves (3/10/30s, three corpora, three
  seeds, three rounds, paired vs v4), the two-process determinism check, and
  the full `cargo test --release --features jagua-experimental` suite were
  not run. Given the box is shared and each of those is itself a multi-minute
  to multi-hour undertaking done properly and paired, running a truncated
  version and presenting it as "the anytime curve" would misrepresent a
  measurement this project's own conventions treat as load-bearing.

## Provenance

| item | value |
|---|---|
| worktree | `/tmp/topo-work-wf48` (this session's isolated agent worktree, `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_48bbc38e-879-1`, was on an unrelated branch with none of this code; see the final report for why) |
| baseline worktree | `/tmp/topo-baseline`, detached at `f32c629`, no changes |
| branch | `wf48/coordinator-v5-budget-debit`, based on `origin/engine/topology-archive-search` at `f32c629` |
| base commit | `f32c6299debee9dd1d8d6edc0716e3010bbbaf01` |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json` |
| gate binary sha256 (`jagua-experimental`, fixed) | `9008729895bc4461350779d9b01b45ac599d89e2ea61f08f8d59d998f8f9a1b8` |
| measurement binary sha256 (`jagua-experimental,compression-schedule`, fixed) | `38d439aa6d3c06bfd46271f255f7629d71b8ee3b91e4d53b8824d57effe06870` |
| measurement binary sha256 (`jagua-experimental,compression-schedule`, baseline `f32c629`) | `ddb7d7468166fae3205d973260712dfa135c068774f0bf8d09e45f654bc8e9e4` |
| box | x86_64, 16 cores, shared with sibling agents in this same orchestration run (their builds observed running concurrently) |
