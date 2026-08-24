# Pool-Retry Tracker Rebase — Gate-0 verdict

Verdict: **FAIL — mechanism closed before quality**.

Sol, Grok, and ox-alpha independently returned `REVIEW PASS` for the same
source commit, `fb76044424a4d5064e783422c2a2258c83fe4efc`. Gate 0 then ran
exactly once. G0.1 passed in full; G0.2 failed its precommitted publication and
causality law, so the runner stopped without executing G0.3, G0.4, Primary30,
or any 10-/60-second report-only point.

| G0.2 seed | Saved | Rebase | Causal decision changed | Structural/authority record |
| ---: | --- | --- | --- | --- |
| 0 | published at iteration 22 | unpublished at cap 400 | yes | PASS |
| 1 | unpublished at cap 400 | unpublished at cap 400 | yes | PASS |
| 2 | published at iteration 4 | published at iteration 15 | yes | PASS |
| 3 | refused at iteration 5 | refused at iteration 5 | yes | PASS |
| 4 | unpublished at cap 400 | unpublished at cap 400 | yes | PASS |
| 5 | published at iteration 3 | published at iteration 3 | yes | PASS |
| 6 | refused at iteration 17 | published at iteration 20 | yes | PASS |
| 7 | unpublished at cap 400 | unpublished at cap 400 | yes | PASS |
| 8 | unpublished at cap 400 | unpublished at cap 400 | yes | PASS |

The exact failed predicates were:

- `Saved-unpublished -> Rebase-published`: seed 6 only, `1 < 2`;
- `Saved-published -> Rebase-unpublished`: seed 0, `1 > 0`;
- causally supported treatment wins: seed 6 only, `1 < 2`.

All nine pairs completed from their literal post-rank/pre-install checkpoint.
Checkpoint identity, predecision identity, Saved and Rebase policy boundaries,
retry completeness, executable identity, independent publication revalidation,
and post-section source/binary/input integrity were green for every seed. This
is therefore a valid mechanism miss, not an instrument failure. The unofficial
seed-0 debug observation had shown the same reverse transition before review;
it was not used to retune or waive the signed Gate.

## G0.1 and provenance

- all four frozen-vs-runtime-Saved identity cells: PASS;
- printed reset, lifecycle, rollback, new-width, and nonfinite vectors: PASS;
- complete default corpus: PASS (`839` unit tests plus integrations);
- complete feature corpus: PASS (`845` unit tests plus integrations);
- frozen commit: `b1235a11cf4a57d7437accbfc2348a05692fe0be`;
- frozen binary SHA-256:
  `0a06815f2f8ffc359633f0b8a56adcbd07490e0d90a1319605157f6f2ccb9bb3`;
- candidate binary SHA-256:
  `da6b6d50d94d18f9585b55833375bdea4ca49767117b9ccd5b108cf14df578ac`;
- spec SHA-256:
  `b5038979351bf2fc114a1d7a220751f0704e362e2dd62809632dceca9245a3a1`;
- request SHA-256:
  `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3`;
- cloned source-plan SHA-256:
  `5d25a310e223e4b99cf8f49a9a45333eec5b51cd290c9a93fb7fcaf565bf53c0`;
- aggregate SHA-256:
  `ebd86befdfc8282b34a52ca650778e4ff2416f545d177178d7fcae25ac1d0faa`.

The complete aggregate is [`gate0.json`](gate0.json); the two build
attestations are [`frozen-build-receipt.json`](frozen-build-receipt.json) and
[`candidate-build-receipt.json`](candidate-build-receipt.json).

The specification permits no retry, decay, partial reset, alternate floor,
extra attempt, threshold repair, or scout after this miss. Pool-Retry Tracker
Rebase is closed. It produced no new 10- or 30-second quality result.
