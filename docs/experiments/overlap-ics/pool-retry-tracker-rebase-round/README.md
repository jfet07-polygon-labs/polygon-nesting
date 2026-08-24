# Pool-Retry Tracker Rebase round

This directory contains the one-shot Gate-0 driver for the exact mechanism in
[`pool-retry-tracker-rebase-spec.md`](../../../pool-retry-tracker-rebase-spec.md).

`build_candidate.py` refuses a dirty or moved tree, builds the exact reviewed
feature set, and writes an exclusive build receipt binding source tree, build
command, toolchain, binary, request, spec, and source plan. `build_frozen.py`
does the same for a clean external `b1235a1` checkout and a new external target
directory. `gate0.py` requires both receipts and verifies every cell's reported
executable SHA against the binary it actually launched before applying the
G0.1 normalization.

The Gate driver clones the frozen deterministic 30-second work rates without
recalibration. For each seed it runs one prefix producer, freezes the canonical
post-rank/pre-install checkpoint read-only, and gives byte-identical copies to
fresh Saved and Rebase processes. G0.3 reuses copies of seed 0's exact G0.2
artifact. G0.4 runs two fresh complete producer/checkpoint/Rebase-resume
pipelines and requires both checkpoint bytes and normalized producer/retry
documents to match. It runs the complete default and feature `--lib --tests`
corpora; the repository's documented pre-existing `--all-targets`/non-Jagua
example incompatibility is not mislabeled as a round failure. It stops before
Primary30 on any miss. Raw cells and the aggregate are written to a new
directory outside the repository, by default
`/var/lib/t3/tmp/overlapics/pool-retry-tracker-rebase-gate0`; an existing path
is refused rather than overwritten.

No result exists until all three consultants return `REVIEW PASS` on the same
implementation commit and the driver is then executed once against that
commit.

The one-shot invocation is:

```text
python3 build_frozen.py <clean-b1235a1-tree> <new-frozen-target> \
  <new-frozen-receipt.json>
python3 build_candidate.py <reviewed-commit> <new-candidate-receipt.json>
python3 gate0.py <frozen-binary> <frozen-receipt.json> <reviewed-commit> \
  <candidate-receipt.json> <new-output-dir>
```
