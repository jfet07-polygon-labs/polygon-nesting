#!/usr/bin/env bash
# The protocol's suites, with the exit status captured **directly** rather than
# through a pipe.
#
#   run-suites.sh
#
# `cargo test ... | tee log` reports `tee`'s exit status, not the test runner's,
# which is how a red suite gets written up as green. Every suite here therefore
# redirects to a file and reads `$?` on the next line.
#
# Three suites. Suites 1 and 2 are the protocol's two, unchanged, and prove the
# round did not break the shipping feature set. Suite 3 is the combo plus
# `se2-rigidity-certificate` and `contact-block-se2`, which is the only build in
# which this round's code exists at all - without it the protocol's combo would
# compile none of it.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-2
E="$W/docs/experiments/contact-block/evidence"
mkdir -p "$E"
cd "$W"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/lib/t3/tmp/cblock/tgt-test}"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

echo "== suite 1: jagua-experimental"
cargo test --features jagua-experimental > "$E/suite-jagua.log" 2>&1
S1=$?
echo "suite-jagua exit=$S1"
grep -hE "^test result:" "$E/suite-jagua.log" | tail -3

echo "== suite 2: the protocol's full combo"
cargo test --features "$COMBO" > "$E/suite-combo.log" 2>&1
S2=$?
echo "suite-combo exit=$S2"
grep -hE "^test result:" "$E/suite-combo.log" | tail -3

echo "== suite 3: the combo plus this round"
cargo test --features "$COMBO,se2-rigidity-certificate,contact-block-se2" \
    > "$E/suite-block.log" 2>&1
S3=$?
echo "suite-block exit=$S3"
grep -hE "^test result:" "$E/suite-block.log" | tail -3

echo "SUITE_EXITS $S1 $S2 $S3"
