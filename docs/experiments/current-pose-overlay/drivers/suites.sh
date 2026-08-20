#!/bin/bash
# Both full suites this round owes.
#
# `jagua-experimental` is the protocol's suite. `jagua-experimental,
# compression-schedule` is the combo the overlay actually compiles under, which
# Sol review 6 §2.5 flagged as missing from the v5 round: the log it committed
# was for `jagua-experimental` alone, in which `current_pose_overlay` does not
# exist and none of the tests that cover it are compiled at all.
#
# The exit status is written to its own file rather than echoed inline: the
# point of the protocol's "separate command" is that the status reported is the
# test runner's own and not a pipeline's, and a redirect plus a `$?` capture on
# the next line preserves exactly that.
#
#   bash suites.sh OUTDIR
set -u
outdir="${1:?usage: suites.sh OUTDIR}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
mkdir -p "$outdir"
cd "$root"

cargo test --release --features jagua-experimental \
  > "$outdir/suite-jagua-experimental.log" 2>&1
echo $? > "$outdir/suite-jagua-experimental.exit"

cargo test --release --features jagua-experimental,compression-schedule \
  > "$outdir/suite-jagua-experimental-compression-schedule.log" 2>&1
echo $? > "$outdir/suite-jagua-experimental-compression-schedule.exit"

grep -hc '^test result: ok' "$outdir"/suite-*.log
