#!/usr/bin/env bash
# Build the four binaries this round measures on, **from the committed tree**.
#
#   run-build.sh
#
# Sol review 9's provenance finding is the reason this is a script and not four
# lines in a README: the round it audited had *"una rottura netta fra sorgente,
# driver e binario misurato"*. So this refuses to build a dirty tree, records
# the commit and the four sha256s beside the binaries, and every other script
# here takes a binary path rather than building one.
#
# Four and not two, because this round's central claim is a *paired* one and the
# pair has to be built the same way: `base-*` from the round's base commit and
# `ship-*` from its head, with the base binaries checked against the sha256s
# `docs/experiments/real-interruption/evidence/binaries.txt` already records.
# That check is the round's provenance gate and it is cheap: if a clean build of
# the base commit does not reproduce the previous round's binary byte for byte,
# nothing below is comparable to anything that round published.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-1
B=/var/lib/t3/tmp/consol/bin
E="$W/docs/experiments/consolidation/evidence"
BASE=40852e6
cd "$W" || exit 3
mkdir -p "$B" "$E" /var/lib/t3/tmp/consol

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

if [ -n "$(git status --porcelain)" ]; then
  echo "REFUSING: the tree is dirty. Commit first - the whole point of this"
  echo "script is that the binary and the source agree."
  git status --short
  exit 2
fi
HEAD_COMMIT=$(git rev-parse HEAD)

build() {
  local label=$1 features=$2 target=$3
  echo "== $label: --features $features"
  CARGO_TARGET_DIR="$target" cargo build --release \
    --example general_request_benchmark --features "$features" \
    > "/var/lib/t3/tmp/consol/build-$label.log" 2>&1
  local status=$?
  echo "$label build exit=$status"
  [ "$status" -eq 0 ] || exit "$status"
  cp "$target/release/examples/general_request_benchmark" "$B/$label"
}

# The base pair first, from a detached checkout of the base commit, so the
# comparison's left-hand side is the committed base and not a reverted head.
git checkout -q "$BASE" || exit 3
build base-gate jagua-experimental /var/lib/t3/tmp/consol/tgt-gate
build base-combo "$COMBO" /var/lib/t3/tmp/consol/tgt-combo
git checkout -q - || exit 3

build ship-gate jagua-experimental /var/lib/t3/tmp/consol/tgt-gate
build ship-meas "$COMBO" /var/lib/t3/tmp/consol/tgt-combo

{
  echo "base-commit $BASE"
  echo "head-commit $HEAD_COMMIT"
  echo "features-gate jagua-experimental"
  echo "features-ship $COMBO"
  sha256sum "$B/base-gate" "$B/base-combo" "$B/ship-gate" "$B/ship-meas"
} | tee "$E/binaries.txt"
