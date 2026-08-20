#!/usr/bin/env bash
# Builds the two "unfixed" benchmark binaries the paired battery compares
# against: the campaign line at `f32c629`, i.e. the commit *before*
# `66060f1` introduced the debit at all.
#
# It does it by swapping the two changed files out of the working tree,
# building, and swapping them back, rather than by adding a second git
# worktree: this agent owns one worktree and touching the shared repository's
# worktree metadata while two sibling agents are building in it is not worth
# the convenience. The index is left alone throughout - the files are written
# with `git show`, not `git checkout`, so nothing staged is disturbed - and the
# script restores byte-for-byte copies taken before the swap.
set -eu
ROOT=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1
SAVE=/var/lib/t3/tmp/wf6v1/save
BASE=f32c629
P=crates/polygon-nesting-core/src/search/portfolio.rs
E=crates/polygon-nesting-core/examples/general_request_benchmark.rs
cd "$ROOT"

mkdir -p "$SAVE"
cp "$P" "$SAVE/portfolio.rs"
cp "$E" "$SAVE/general_request_benchmark.rs"

restore() {
  cp "$SAVE/portfolio.rs" "$ROOT/$P"
  cp "$SAVE/general_request_benchmark.rs" "$ROOT/$E"
  echo "RESTORED"
  cd "$ROOT" && git diff --stat -- "$P" "$E" | tail -3
}
trap restore EXIT

git show "$BASE:$P" > "$P"
git show "$BASE:$E" > "$E"
echo "swapped to $BASE"

echo "=== unfixed gate build ==="
CARGO_TARGET_DIR=/var/lib/t3/tmp/wf6v1/t-gate-unfixed \
  cargo build --release --example general_request_benchmark \
  --features jagua-experimental -j 6 2>&1 | tail -3
echo "=== unfixed sched build ==="
CARGO_TARGET_DIR=/var/lib/t3/tmp/wf6v1/t-sched-unfixed \
  cargo build --release --example general_request_benchmark \
  --features jagua-experimental,compression-schedule -j 6 2>&1 | tail -3
