# Conflict-cluster budget Gate-0 verdict

Verdict: **FAIL — mechanism closed before quality**.

This is the first and only Gate-0 execution of candidate commit
`11a1a4574a9db062a9905b37384ca236cf982334`. The complete aggregate is
[`gate0.json`](gate0.json); the complete Shadow decision records, direct-arm
records, deterministic replays, exact vectors, cost cells, and test logs are
in this directory. [`raw-evidence.sha256`](raw-evidence.sha256) identifies all
31 files produced by the runner, including the large cross-binary identity
documents that are not duplicated here.

| Clause | Result | Evidence |
| --- | --- | --- |
| spec digest | PASS | `0cfdf0e2557967e5aab3a48534e4ff6508c38b3d1054344360aedd61ce284ce9` |
| quiet box | PASS | load1 `0.79 < 1.0` |
| G0.1 runtime-Off identity | PASS | all four stripped cross-binary digests equal |
| G0.2 exact vectors/tests | PASS | all 12 vector clauses; feature and default corpora exit 0 |
| G0.3 Shadow engagement | PASS | 208 eligible decisions; 168 B/D quota disagreements (`0.8076923076923077`) |
| G0.4 compute-ignore cost | **FAIL** | median rate ratio `0.9404693372400017 < 0.95` |
| G0.5 accounting/determinism | PASS | all three direct arms pass; fresh-process B replay identical after removing only wall |
| binary unchanged | PASS | candidate SHA identical before and after Gate 0 |

The five frozen G0.4 ratios, in the required order, are:

```text
AB  0.9152228025697704
BA  0.9404693372400017
AB  1.0012261746083466
BA  0.9403034413354346
AB  0.9409010682678118
```

Every paired pose, consumed order, work vector, actual slot count, expected
slot count, partition slot count, and legacy proposal count is identical;
invalid fallbacks are zero in all five cells. The miss is therefore the cost
of constructing the source field, graph, B masses, allocation, and schedule,
not behavioral drift. The median rate loss is `5.95306627599983%` (a
`6.329888748388446%` time overhead at equal completed slots).

The signed specification says: any Gate-0 miss stops the round, with no retry,
threshold repair, alternate formula, or quality scout. Accordingly no primary
30-second battery and no report-only 10- or 60-second battery was executed.
The high B/D disagreement rate shows that the proposed signal was materially
different; this round rejects it on its predeclared economics, not on quality.

## Provenance

- frozen source commit: `a6e5d1b13b14b3b776d48d7f3298af5980fb762d`
- frozen binary SHA-256: `fede5ca35a4a0be40f5913289d55c848243597c7335475d42cb78d710ca9e39e`
- candidate binary SHA-256: `b8425dfdea6d8a6d84a6d27aa8df51a3308714533f4348d4e482ba90eafe61a5`
- request SHA-256: `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3`
- raw aggregate SHA-256: `c397d44878a106d1f8cfe717f312368be1b760d71e81dcf9fc7db1eb30eb7771`
- committed aggregate SHA-256: `c484fe5e9e459c16caed054458ccbb9bf280f8e2f00a37626f1b697ce1186ec4`

The committed aggregate differs from the raw aggregate only by a final
newline added for the repository text-file convention.
