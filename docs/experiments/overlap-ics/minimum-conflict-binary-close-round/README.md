# Minimum-Conflict Binary Close round — closed

The authority for this round is
[`docs/minimum-conflict-binary-close-spec.md`](../../../minimum-conflict-binary-close-spec.md).
The implementation received three `REVIEW PASS` verdicts from Sol, Grok, and
ox-alpha on source commit `bb13dad6e3eac54e7ce8095d339778d20ac0411f`.

After that quorum, build the reviewed commit in release mode with
`overlap-ics,minimum-conflict-binary-close`, supply the externally built
`918d6ff` control binary, and run exactly once:

```text
python3 gate0.py <frozen-918d6ff-binary> <reviewed-source-commit> <output-dir>
```

The runner checks the signed spec digest, clean reviewed commit, quiet-box
entry, exact G0.1 feature sets and executable SHAs, unchanged control/candidate
binary SHAs, G0.1–G0.5 in order, all nine G0.3 seeds,
the forward `ComputeIgnore / Centre` median, and two-process MinCut identity.
It stops at the first failed Gate-0 section. It contains no quality command.

## Result

Gate 0 passed once, in full. In particular, seed 3 supplied the required true
causal inversion at the 22nd bite, the median forward
`ComputeIgnore / Centre` rate ratio was `0.9924168878994567` against the
predeclared `0.95` floor, and the two fresh MinCut processes were identical
after removing only `wall`.

The licensed 30-second deterministic-work battery then failed validly:

| clause | required | observed | result |
| --- | ---: | ---: | --- |
| MinCut median | at most `163.00461 mm` | `164.01195493488737 mm` | FAIL |
| MinCut floor | at least `7/9` at or below `168.484 mm` | `5/9` | FAIL |
| paired median `Centre - MinCut` | at least `1.000 mm` | `-1.4946405113654464 mm` | FAIL |
| validity and authority | all green | all 18 cells green; zero invalid publications | PASS |

The contemporaneous Centre control produced a new round best of
`160.8922949111259 mm`, a median of `162.94240595756042 mm`, and `7/9` seeds at
or below `168.484 mm`. MinCut's best was `161.03476676219478 mm`. The treatment
therefore did not cause the control improvement and made the population worse.

Section 8 licenses the 10- and 60-second report-only curve points only after a
primary PASS. Neither was run. Minimum-Conflict Binary Close is closed without
retuning, alternate labels, another cut rule, or a rescue run.

Committed evidence:

- [`evidence/gate0.json`](evidence/gate0.json), SHA-256
  `0b8d3963cac4cd427bd6a654595690696a54255a245d13b3167b69a8f5279eb4`;
- [`evidence/primary30.json`](evidence/primary30.json), SHA-256
  `6f6aeed19901a88b46d1dbf009e1888d31ac6be9141aba1444f7cb71ba62267f`;
- [`evidence/plan-f100-mbc.icscal.json`](evidence/plan-f100-mbc.icscal.json),
  SHA-256
  `5d25a310e223e4b99cf8f49a9a45333eec5b51cd290c9a93fb7fcaf565bf53c0`.

The 18 raw quality documents remain in
`/var/lib/t3/tmp/overlapics/minimum-conflict-binary-close-quality/cells`; every
one is bound into `primary30.json` by path and SHA-256.
