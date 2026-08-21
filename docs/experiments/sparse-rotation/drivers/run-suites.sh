#!/usr/bin/env bash
# §6's suites, with the exit status captured **directly** rather than through a
# pipe.
#
#   run-suites.sh
#
# `cargo test ... | tee log` reports `tee`'s exit status, not the test runner's,
# which is how a red suite gets written up as green. Every suite in this
# campaign therefore redirects to a file and reads `$?` on the next line.
#
# Three suites rather than the campaign's two, and the third is the point: the
# protocol's "full combo" does not name `sparse-rotation`, so running only it
# would compile none of this round's code. Suite 2 is the protocol's combo
# unchanged - it proves the round did not break the shipping feature set - and
# suite 3 is that combo plus `sparse-rotation` and `se2-rigidity-certificate`,
# which is the only build in which design C exists at all.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_4111958b-3b3-2
E="$W/docs/experiments/sparse-rotation/evidence"
mkdir -p "$E"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/lib/t3/tmp/sparserot/tgt-test}"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,fast-contract-validator

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
cargo test --features "$COMBO,sparse-rotation,se2-rigidity-certificate" \
    > "$E/suite-sparse.log" 2>&1
S3=$?
echo "suite-sparse exit=$S3"
grep -hE "^test result:" "$E/suite-sparse.log" | tail -3

echo "SUITE_EXITS $S1 $S2 $S3"
