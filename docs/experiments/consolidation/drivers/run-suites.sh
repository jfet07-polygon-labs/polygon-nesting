#!/usr/bin/env bash
# The protocol's two suites, with the exit status captured **directly** rather
# than through a pipe.
#
#   run-suites.sh
#
# `cargo test ... | tee log` reports `tee`'s exit status and not the test
# runner's, which is how a red suite gets written up as green. Both suites
# therefore redirect to a file and read `$?` on the next line.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-1
E="$W/docs/experiments/consolidation/evidence"
mkdir -p "$E"
cd "$W" || exit 3

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/lib/t3/tmp/consol/tgt-test}"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

echo "== suite 1: jagua-experimental (the gate binary's feature set)"
cargo test --features jagua-experimental > "$E/suite-jagua.log" 2>&1
S1=$?
echo "suite-jagua exit=$S1"
grep -hE "^test result:" "$E/suite-jagua.log" | tail -6

echo "== suite 2: the protocol's full combo"
cargo test --features "$COMBO" > "$E/suite-combo.log" 2>&1
S2=$?
echo "suite-combo exit=$S2"
grep -hE "^test result:" "$E/suite-combo.log" | tail -6

echo "EXITS jagua=$S1 combo=$S2"
[ "$S1" -eq 0 ] && [ "$S2" -eq 0 ]
