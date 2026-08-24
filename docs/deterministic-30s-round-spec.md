# Deterministic 30-second round

## Status and authority

This is the specification of record for the round after the amended economics
wave at `fa10d2a`. It is written before the implementation, calibration, or
quality cells of this round.

Sol signed the synthesis. Grok signed it after making one binding correction:
there is **no 30-second p95 clause**. After earlier provider failures,
ox-alpha returned on the compact amended ballot, independently checked this
text and signed it. The quorum for this round is 3/3.

The economics round remains closed. Its impatient strike policy is not revived,
neither rejected currency is revived, and its frozen member remains the control.

## The licensed changes

Exactly two engine/driver changes are licensed:

1. Repair the non-profile branch of `shelf_work_plan`. Its numerator is the
   shelf probe bite's own `profile.sample_evaluations`, not the trajectory's
   cumulative work vector. The existing witness is red at approximately
   3,200,171 units/s versus the bite-local approximately 2,747,252 units/s and
   must become green before any quality battery.
2. Expose the already-existing constructor portfolio limit as the experimental
   factor it is. The gate driver already accepts `--orders`; no new constructor
   family is licensed. In particular, this round does not build an LBF arm.

No quality claim is attached to change 1. It repairs a false derivation and
removes the out-of-engine workaround used by the previous wave.

## Gate 0: constructor identity and price

Run five interleaved AB/BA pairs on mixed-61, shapes-17, and triangle-20 with
`orders=4` as A and `orders=1` as B. The exact gate binary is used throughout.

Gate 0 passes iff all of the following hold:

- mixed-61 `orders=1` and `orders=4` have identical constructor placement
  fingerprints and identical raw-source depth (the expected observation is
  182.976 mm), and both are dual-valid;
- mixed-61 `orders=1` constructor p95 is at most 0.800 s;
- the paired median constructor saving on mixed-61 is at least 1.500 s;
- on shapes-17 and triangle-20 both starts are dual-valid and `orders=1` is no
  worse than `orders=4` by more than 1.000 mm.

If mixed-61 changes fingerprint or the saving misses its bar, `orders=1` is
closed and the round stops before quality measurement. No alternative weak
constructor may be substituted.

## Safety-factor derivation

There are two derived factors, one for each constructor setting. They are
fixed before a gate depth is read:

- `f4*` for `orders=4`;
- `f1*` for `orders=1`.

Let `C4` be the mixed-61 `orders=4` constructor p95 from Gate 0, `Co` the p95
for order setting `o`, and `W0.80 = 9.5271 s` the signed wave-4 control p95 at
safety 0.80. Define the modeled request wall

```text
Wo(f) = Co + (W0.80 - C4) * f / 0.80
```

and the holdback

```text
kappa = max(0.050 s, 2 * largest within-arm Gate-0 constructor range)
```

For each order setting, `fo*` is the largest hundredth in `[0.80, 1.00]` for
which `Wo(fo*) + kappa <= 10.000 s`. The factor is capped at 1.00. Gate-0 runs
must begin in a quiet box (one-minute load below 1.00). The input readings,
formula, candidates, chosen values, and generated plans are committed before
the first end-to-end quality cell. A later p95 or depth cannot retune them.

This is a mixed-61-only denomination and carries the label **single-fixture
work plan, no transfer claim**. The plan is rebuilt and keyed to the repaired
binary. Explore and compress retain the frozen 80/20 time split; only the safe
rate factor differs.

## The four contemporaneous arms

Every end-to-end battery runs these four arms in the declared order:

| arm | constructor | safety factor | role |
|---|---:|---:|---|
| `control` | 4 | 0.80 | contemporaneous frozen-member control |
| `orders` | 1 | 0.80 | constructor-cost main effect |
| `factor` | 4 | `f4*` | safety-factor main effect |
| `composed` | 1 | `f1*` | pre-declared treatment and gate owner |

The primary contrast is `composed - control`. The other two arms make the
factorial attribution auditable. They are not alternative winners selected
after seeing the data.

Everything else is frozen: strike literals `200/3/100/5/0.98`, eight workers,
the CutCloseRelocate operator family, GLS, explore/compress bite definitions,
publication gates, bare mixed-61, seeds 0 through 8, and `--revalidate=1`.
The work-quanta impatient arm stays dead.

## Primary gate: deterministic 30 seconds

The gate owner is `composed`, paired against `control` in the same battery.
PASS requires all of:

1. the `composed` median raw-source depth is at most **163.00461 mm**;
2. at least **7/9** `composed` seeds are at most **168.484 mm**;
3. the paired median improvement `control depth - composed depth` is at least
   **1.000 mm**;
4. every publication in every arm is Exclusive `r=2.500`, contract-valid, and
   independently revalidated; there are zero invalid publications;
5. calibrated work and charge identities hold and the executable/plan keys
   match on every cell.

There is **no 30-second p95 gate**. It was not part of the standing 30-second
law, and adding it after the observed 39.83 s tail would be retroactive. The
wall distribution, p95, maximum, and overruns are reported.

The shapes-17/triangle-20 equal-work transfer clause remains **NOT-RUN**: this
is a single-fixture plan and makes no transfer claim.

## Ten seconds: one unsoftened last chance

The same four arms run at 10 seconds. For the pre-declared `composed` arm, PASS
requires all of:

1. at least **5/9** exact-valid strict non-constructor children at or below
   **168.484 mm**;
2. median at most **168.484 mm**;
3. p95 process wall at most **10.000 s** over five repetitions of all nine
   seeds;
4. two-process bit identity for every seed;
5. the validity and plan/charge floor from the primary gate.

The bar is not reduced to 3/9. Three is reported as a diagnostic, never a PASS.
If this last chance misses with a green floor, the 10-second quality gate is
permanently retired into `docs/shipped-surface.md`, with the economics AB/BA
battery and this round's four arms named as its instrument of death. It may not
be reopened by retuning this member; a genuinely new mechanism would require a
new pre-committed specification.

## Report-only cells and stop rules

- 3 seconds is reported for the curve, never gated.
- 60 seconds is reported on all four arms. Median **161.00 mm** is a named
  watch, not a clause; wall tails are reported. The 150.165 mm Sparrow result
  remains a horizon, not a clause.
- Any invalid publication, plan-key mismatch, charge-identity failure, or
  binary change during a battery stops the round for defect repair rather than
  retargeting.
- Gate 0 may kill `orders=1`. After Gate 0 passes, a valid miss of either
  quality gate licenses no constant change or fifth arm.

## Quorum record

- **Sol:** YES to the four-arm synthesis, followed by CONFIRM.
- **Grok:** YES after deleting the proposed 30-second p95 line; his correction
  is incorporated above. He explicitly withdrew his earlier 0.47 s argument
  after the measured 1.68 s constructor saving.
- **ox-alpha:** YES. Verbatim closing sentence: “the arms are pre-committed,
  the factor is derived fresh where the measured constructor cost actually is,
  and if the 10s bet still misses with a green floor it dies permanently
  instead of haunting another round.”
- **Owner:** retain the full quorum; do not set it aside.
