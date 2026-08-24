# Verdict — Minimum-Conflict Binary Close

Verdict: **FAIL — mechanism closed after a valid primary 30-second miss**.

Sol, Grok, and ox-alpha independently returned `REVIEW PASS` for the same
source commit, `bb13dad6e3eac54e7ce8095d339778d20ac0411f`. The exact candidate
executable SHA-256 was
`a6c53146a9481c3014bc33250ec5be20a2beaaa886282b4d84ca5f181908222b`.

Gate 0 ran once and passed every clause. Its true 22nd-bite causal inversion
was seed 3. The frozen and candidate binaries remained unchanged, the complete
test/vector corpus passed, the cost floor passed, and the MinCut replay was
bit-identical after removing only `wall`.

The primary battery used bare mixed-61, order 1, eight workers, strike control,
U0, seeds 0 through 8, one fresh process per arm and seed, and independent
revalidation. It cloned the signed factor-1.00 deterministic plan without
recalibration: `2759025.975468987` explore units/s,
`1408465.9444235826` compress units/s, and `27.67205079595` seconds-equivalent
work.

| metric | Centre | MinCut |
| --- | ---: | ---: |
| best raw-source depth | `160.8922949111259 mm` | `161.03476676219478 mm` |
| median raw-source depth | `162.94240595756042 mm` | `164.01195493488737 mm` |
| seeds at or below `168.484 mm` | `7/9` | `5/9` |

The paired median `Centre - MinCut` was `-1.4946405113654464 mm`; MinCut made
the median outcome worse. All 18 cells remained contract-valid, independently
revalidated, plan-matched, charge-identical, and executable-identified, with
zero invalid publications. This is a quality failure, not an authority or
instrument failure.

The Centre arm set the round's new 30-second deterministic-work best at
`160.8922949111259 mm`, improving the prior approximately `161.05 mm` mark.
That improvement is not attributable to Minimum-Conflict Binary Close.

The signed specification permits the 10- and 60-second report-only points only
after primary PASS. They were not run. No retry or rescue run is licensed.
