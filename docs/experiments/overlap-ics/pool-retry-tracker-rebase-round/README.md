# Pool-Retry Tracker Rebase round

This directory contains the one-shot Gate-0 driver for the exact mechanism in
[`pool-retry-tracker-rebase-spec.md`](../../../pool-retry-tracker-rebase-spec.md).

`build_candidate.py` refuses a dirty or moved tree, builds the exact reviewed
feature set, and writes an exclusive build receipt binding source tree, build
command, toolchain, binary, request, spec, and source plan. `gate0.py` accepts
that receipt plus an externally built `b1235a1` control binary.

The Gate driver clones the frozen deterministic 30-second work rates without
recalibration. For each seed it runs one prefix producer, freezes the canonical
post-rank/pre-install checkpoint read-only, and gives byte-identical copies to
fresh Saved and Rebase processes. G0.3 and G0.4 reuse copies of seed 0's exact
G0.2 artifact. It runs the complete default and feature `--lib --tests`
corpora; the repository's documented pre-existing `--all-targets`/non-Jagua
example incompatibility is not mislabeled as a round failure. It stops before
Primary30 on any miss. Raw cells and the aggregate are written to a new
directory outside the repository, by default
`/var/lib/t3/tmp/overlapics/pool-retry-tracker-rebase-gate0`; an existing path
is refused rather than overwritten.

No result exists until all three consultants return `REVIEW PASS` on the same
implementation commit and the driver is then executed once against that
commit.
