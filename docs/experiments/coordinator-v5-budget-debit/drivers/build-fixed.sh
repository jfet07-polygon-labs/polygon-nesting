#!/usr/bin/env bash
# Builds the two "fixed" benchmark binaries this round measures with:
# the gate build (jagua-experimental only, which is the default debit path
# with no self-metered operator compiled in) and the schedule build
# (jagua-experimental,compression-schedule, the only configuration in which
# mode 34 exists and therefore the only one in which a debit can be non-zero).
set -u
ROOT=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1
cd "$ROOT" || exit 1
echo "=== gate build (jagua-experimental) ==="
CARGO_TARGET_DIR=/var/lib/t3/tmp/wf6v1/t-gate-fixed \
  cargo build --release --example general_request_benchmark \
  --features jagua-experimental -j 6 2>&1 | tail -4
echo "GATEBUILD=${PIPESTATUS[0]}"
echo "=== sched build (jagua-experimental,compression-schedule) ==="
CARGO_TARGET_DIR=/var/lib/t3/tmp/wf6v1/t-sched-fixed \
  cargo build --release --example general_request_benchmark \
  --features jagua-experimental,compression-schedule -j 6 2>&1 | tail -4
echo "SCHEDBUILD=${PIPESTATUS[0]}"
