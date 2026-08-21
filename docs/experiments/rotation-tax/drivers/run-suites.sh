#!/usr/bin/env bash
# §6.4's two suites, with the exit status captured **directly** rather than
# through a pipe.
#
#   run-suites.sh
#
# `cargo test ... | tee log` reports `tee`'s exit status, not the test
# runner's, which is how a red suite gets written up as green. Every suite in
# this campaign therefore redirects to a file and reads `$?` on the next line.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_08a442a7-1aa-1
E="$W/docs/experiments/rotation-tax/evidence"
mkdir -p "$E"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/lib/t3/rt/tgt-test}"

echo "== suite 1: jagua-experimental"
cargo test --features jagua-experimental > "$E/suite-jagua.log" 2>&1
S1=$?
echo "suite-jagua exit=$S1"
grep -hE "^test result:" "$E/suite-jagua.log" | tail -5

echo "== suite 2: the full combo"
cargo test --features jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,fast-contract-validator \
    > "$E/suite-combo.log" 2>&1
S2=$?
echo "suite-combo exit=$S2"
grep -hE "^test result:" "$E/suite-combo.log" | tail -5

echo "SUITE_EXITS $S1 $S2"
