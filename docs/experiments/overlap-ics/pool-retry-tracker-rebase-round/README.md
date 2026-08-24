# Pool-Retry Tracker Rebase round

This directory contains the one-shot Gate-0 driver for the exact mechanism in
[`pool-retry-tracker-rebase-spec.md`](../../../pool-retry-tracker-rebase-spec.md).

The driver accepts an externally built `b1235a1` control binary and the exact
source commit reviewed by Sol, Grok, and ox-alpha. It refuses a dirty or moved
source tree, checks the immutable spec digest, clones the frozen deterministic
30-second work rates without recalibration, and stops before Primary30 on any
Gate-0 miss. Raw cells and the aggregate are written outside the repository by
default under `/var/lib/t3/tmp/overlapics/pool-retry-tracker-rebase-gate0`.

No result exists until all three consultants return `REVIEW PASS` on the same
implementation commit and the driver is then executed once against that
commit.
