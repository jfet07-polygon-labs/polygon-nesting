# The economics round — the three-signature spec (2026-08-23)

Signed by Sol (review 19 R3), Grok (review 14 R3) and ox-alpha (review 1
closing vote) after full three-way exchange — the first three-model quorum of
the campaign, mandated by the owner. **Execution is GATED on the evidence-
integrity audit** (docs/experiments/overlap-ics/evidence-audit/, in flight at
signing time): if the audit finds evidence-distorting defects in the
cutclose-rerun numbers this spec was designed against, the affected numbers
are re-measured and the three signatures re-confirmed before any
implementation.

## Funded: exactly three changes on the frozen member

1. **The two-arm strike experiment** (the item the quorum was built for —
   the consultants swapped positions across the exchange and both live in
   the design):
   - TREATMENT: work-denominated impatient strikes — after each master
     batch, observe_raw classifies (2% Substantial / Marginal / None,
     untouched); None adds the batch's all-eight-workers sample_evaluations;
     strike at the quantum, frozen as KNOB: explore **1_630_000**, compress
     **815_000** (derived from Sparrow's same-machine 3.742M evals/s ÷ 460
     iters/s × 200/100; no "more precise" second guess ever); counts 3/5
     unchanged; overshoot ≤ one batch. Honest label: *a distinct
     impatient-strike policy pre-derived from source* — NOT "what 200 always
     meant".
   - CONTROL: the frozen literals 200/3/100/5/0.98 on the identical executor
     and pacer — strike semantics are the only delta between arms.
   - PROMOTION: treatment must gain ≥2 qualifying seeds or ≥1.000 mm paired
     median over control; else the absolute 5/9 is a draw, the impatient
     policy is NOT promoted, and the control's policy remains the member.
2. **Persistent executor, behind a measured gate**: profile easy + bite-22
   hard states, workers 1/2/4/8, identical fixed work (prep, dispatch/join,
   sweeps, merge+GLS, exact/repair separately). Build iff prep+dispatch
   ≥10% of hard-state wall. Promote iff bit-identical vs ephemeral ≥1,024
   batches (incl. strike, pool restore, disruption), ≥1.15× shelf p50,
   ≥1.10× geomean over mixed-61/shapes-17/triangle-20, ≤5% any-fixture
   regression, ≤10% RSS. Implementation: local Rayon pool of 8, persistent
   slots, clone_from, ordinal merge; forbidden: global Rayon, par_iter,
   find_any/find_first, early cancel, any completion-order observation;
   fallback on a red merge-identity FAST: eight parked OS threads + barrier.
   If not built, the 5/9 clause does NOT drop (no 4/9 — Grok's refusal,
   unanimous in the end).
3. **Persisted calibrated-work pacer** (after 1 and 2 freeze): currency
   U = sample_evaluations + B·master_batches + E·actual_publication_attempt_calls
   + R·repair_rows + D·disruption_moves; B/E/R/D from timing-only
   microbenchmarks on all three fixtures, conservative rounding; REJECT the
   currency if wall-prediction error >10% on any transfer fixture. The file
   pins request hash, currency version, binary/feature key, workers=8,
   executor implementation, per-phase safe units/s; read/write separate; no
   live probe on a gated trajectory; 80/20 by calibrated units; compress
   decay by consumed compress-work; stop only between master batches.
   Wording: "10-second calibrated work plan" — quality deterministic, wall a
   distribution (no governor exists).

## Frozen verbatim

Relocate (25+50/16 angles/3 finalists/two-stage CD/accept-equal), disruption
+ pool + follower semantics, GLS multipliers/schedule, 0.1% explore bites +
centre cut, compress range + uniform-Y cut + **the 80/20 share (fully frozen;
Grok withdrew his exception — shadow counters only)**, workers=8, constructor
(charged, uncapped), publication band/Exclusive r=2.500/contract validator/
16 µm repair, observe_raw's 2% classifier, pool-restore weight policy.
Allowed instrumentation: split exactAttempted into actual-calls vs
bites-with-attempt (sums must reconcile); the profiling counters; the
icscal file format.

## §0 — the pre-committed gates

10 s calibrated-work, bare mixed-61, seeds 0..=8, workers=8, quiet box.
PASS iff ALL: (1) ≥5/9 exact-valid ≤168.484 mm; (2) median ≤168.484;
(3) every publication Exclusive r=2.500 + contract-valid; (4) per-seed
two-process bit identity; (5) quiet-box p95 ≤10.000 s over 5×9;
(6) attribution vs the control arm as above. 30 s: median ≤163.00461;
≥7/9 ≤168.484 (no-regression); paired ≥1.000 mm; shapes-17/triangle-20
within 1 mm at equal work; zero invalid publications. 60 s reported, never
gated. **150.165 is the horizon, not a clause.** No clause may require
seeds 7/8 (the different-basin pair). Failure license: one named line-level
defect with red/green vector → one identical rerun; a valid miss closes
this funding.

## Workflow (Sol's 4 roles / 3 waves) and FAST

(1) spec/profile census, no quality edit; (2) executor agent ∥ meter agent,
neither owns mod.rs; (3) one integration agent owns mod.rs/Pacer/schema;
(4) evidence agent runs drivers, does not edit engine code after the gate
text freezes. FAST union: existing stages + K=1,024 ephemeral/persistent
identity (incl. strike, pool restore, disruption), scheduling-order
perturbation with identical ordinal merge, **batch-two-delta accounting**
(batch 2's aggregate == sum of the eight batch-2 deltas, not cumulative slot
totals), strike meter with variable batch costs, calibrated-plan
hit/miss/version/clock-poison, eight-worker hard-shelf throughput,
actual-attempt reconciliation. Pre-named defects, ranked: (1) persistent-
slot leakage / double-debit ("stable but false" work accounting — the worst
class this round has); (2) completion-order leak; (3) probe-on-cheap-bites
(calibrating on bites 1-21 overstates iters/s ~1.5×; the probe is 400
iterations AT the 179 shelf); (4) compress-steal.

## The three-regime map the gates respect

Fast cascade {2,3;6 near}; strike-starved shelf {0,1,4,5} (work-strikes
target 0/1/4; seed 5 needs ~288 continued-search iterations, not a strike —
the watch-seed for treatment regressions); different basin {7,8} (five
disruptions at 30 s, still unpublished — no clause may require them).
