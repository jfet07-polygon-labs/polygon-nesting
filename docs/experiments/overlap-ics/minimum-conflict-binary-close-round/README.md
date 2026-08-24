# Minimum-Conflict Binary Close round

The authority for this round is
[`docs/minimum-conflict-binary-close-spec.md`](../../../minimum-conflict-binary-close-spec.md).
No Gate-0 or quality result exists until the implementation has three
`REVIEW PASS` verdicts on one source commit.

After that quorum, build the reviewed commit in release mode with
`overlap-ics,minimum-conflict-binary-close`, supply the externally built
`918d6ff` control binary, and run exactly once:

```text
python3 gate0.py <frozen-918d6ff-binary> <reviewed-source-commit> <output-dir>
```

The runner checks the signed spec digest, clean reviewed commit, quiet-box
entry, unchanged candidate binary SHA, G0.1–G0.5 in order, all nine G0.3 seeds,
the forward `ComputeIgnore / Centre` median, and two-process MinCut identity.
It stops at the first failed Gate-0 section. It contains no quality command.
