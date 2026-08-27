# Deterministic 30-second round

## Verdict

The round is valid and the pre-committed primary gate **passes**. The composed
arm (`orders=1`, safety factor `1.00`) records a 30-second median of
**162.94241 mm**, **7/9** seeds at or below 168.484 mm, and a paired median
gain of **1.23247 mm** over the contemporaneous control. Every publication is
valid and independently revalidated; every binary, plan, ledger, and charge
identity holds.

The unsoftened 10-second last chance **fails** its two quality clauses with a
green validity, time, and determinism floor: **2/9**, median **179.07170 mm**,
best **165.42489 mm**, and composed p95 **9.68703 s**. All 180 cells are valid,
all five repetitions are bit-identical, and the two-process identity check
passes on every seed. Under the signed rule, this permanently retires the
10-second quality gate for this member. Retuning cannot reopen it; a new
mechanism requires a new pre-committed specification.

There is deliberately no 30-second p95 clause. The composed p95
**34.69271 s** and maximum **34.95107 s** are reported wall tails, not a
retroactive gate.

`ROUND_VALID_SO_FAR: true`, `PRIMARY_GATE_PASS: true`,
`TEN_SECOND_GATE_PASS: false` in [`evidence/verdict.json`](evidence/verdict.json).

## Curve of record

All values below are for the pre-declared composed arm. Qualification means a
raw-source depth at or below 168.484 mm. The 3- and 60-second rows are
report-only.

| requested budget | best (mm) | median (mm) | qualifying seeds | wall p95 (s) | wall max (s) | status |
|---:|---:|---:|---:|---:|---:|---|
| 3 s | 179.03143 | 179.07957 | 0/9 | 1.23256 | 1.23260 | report only |
| 10 s | **165.42489** | 179.07170 | 2/9 | 9.68703 | 9.73743 | **FAIL; retired** |
| 30 s | **160.89229** | 162.94241 | 7/9 | 34.69271 | 34.95107 | **PASS** |
| 60 s | **159.24631** | 159.88477 | 9/9 | 74.04362 | 74.62990 | report only; 161 mm watch reached |

The 60-second result crosses the named 161.00 mm median watch, which was never
a gate clause. The Sparrow result at 150.165 mm remains a horizon, not a
claim made by this round.

## Attribution

The four arms were run in the signed order; none was selected after seeing its
depth. At 30 seconds:

| arm | orders | factor | best (mm) | median (mm) | qualifying | wall p95 (s) |
|---|---:|---:|---:|---:|---:|---:|
| control | 4 | 0.80 | 160.97750 | 164.03222 | 6/9 | 32.42032 |
| orders | 1 | 0.80 | 160.97750 | 164.03222 | 6/9 | 27.30028 |
| factor | 4 | 0.84 | 160.92954 | 164.00056 | 7/9 | 30.45415 |
| composed | 1 | 1.00 | **160.89229** | **162.94241** | **7/9** | 34.69271 |

`orders=1` preserves the control's quality exactly while removing constructor
work. The factor main effect moves the quorum from 6/9 to 7/9. Their composed
arm owns the gate and supplies the required paired gain.

At 60 seconds the composed arm records **159.24631 mm** best and
**159.88477 mm** median versus control's 159.75099/161.13353 mm; all four arms
are 9/9. Its paired median gain is **0.62069 mm**. The longer wall tail is
reported honestly and carries no verdict at this report-only duration.

## Calibration and integrity

Gate 0 passes all signed clauses. On mixed-61, `orders=1` and `orders=4` have
the same 182.976 mm constructor depth and placement fingerprint, both are
dual-valid, and `orders=1` measures p95 **0.62136 s** with a paired median
saving of **1.69966 s**. The shapes-17 and triangle-20 guard fixtures remain
inside their licensed tolerance.

The repaired shelf-work witness is green: **6,605,800** bite-local units replace
the incorrect **7,694,847** cumulative numerator. The frozen derivation gives
`kappa=0.072465696 s`, `f4*=0.84`, and `f1*=1.00`; all three generated plan
keys hit. The gate binary remains
`fede5ca35a4a0be40f5913289d55c848243597c7335475d42cb78d710ca9e39e`.

The implementation test suite passes **839 tests, 0 failures** under release
with `overlap-ics`. The evidence reducer is [`verdict.py`](verdict.py), and the
raw committed batteries are [`curve30.json`](evidence/curve30.json),
[`gate10.json`](evidence/gate10.json), [`curve3.json`](evidence/curve3.json),
and [`curve60.json`](evidence/curve60.json).

## Authority

The full quorum was retained: Sol **YES + CONFIRM**, Grok **YES** after his
binding removal of the invented 30-second p95 clause, and ox-alpha **YES** on
the amended compact ballot. The signed specification is
[`../../../deterministic-30s-round-spec.md`](../../../deterministic-30s-round-spec.md).
