#!/usr/bin/env bash
# Rebuilds the gate binary from the tree as it stands and reruns all four
# gates, plus the whole-document diff against the unfixed baseline.
#
# Run last, deliberately. The first gate pass in this round used a binary built
# before the round's final doc-comment and test edits; neither can change
# generated code, but "the gates were run against a binary built from the
# source that was committed" is a claim worth being able to make without an
# argument about what does and does not affect codegen.
set -u
ROOT=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1
D=$ROOT/docs/experiments/coordinator-v5-budget-debit/drivers
T=/var/lib/t3/tmp/wf6v1
cd "$ROOT" || exit 1

CARGO_TARGET_DIR=$T/t-gate-fixed cargo build --release \
  --example general_request_benchmark --features jagua-experimental -j 6 2>&1 \
  | tail -3
cp "$T/t-gate-fixed/release/examples/general_request_benchmark" "$T/bin/gate-fixed"

cd "$D" || exit 1
python3 gates.py fixed "$T/bin/gate-fixed" "$T/gates"
python3 gatedocdiff.py "$T/gates" fixed unfixed "$T/gates-docdiff-round6.json"
