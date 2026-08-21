#!/usr/bin/env bash
# The three binaries this round measures on, built from one tree in one pass.
#
#   build.sh [BINDIR]
#
# Separate target directories on purpose: three feature sets sharing one
# `target/` means cargo rebuilds the world between them, and a round that had to
# wait forty minutes for a rebuild is a round that stops re-running its own
# batteries.
set -eu
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_30e47560-32c-2
BIN="${1:-/var/lib/t3/tmp/basinrace/bin}"
cd "$W"
mkdir -p "$BIN"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

build() {
  local name="$1" features="$2" tgt="$3"
  echo "== $name  [$features]"
  CARGO_TARGET_DIR="$tgt" cargo build --release \
    --example general_request_benchmark --features "$features" 2>&1 | tail -1
  cp "$tgt/release/examples/general_request_benchmark" "$BIN/$name"
  sha256sum "$BIN/$name"
}

build race-gate  "jagua-experimental"                     /var/lib/t3/tmp/basinrace/target-racegate
build race-combo "$COMBO"                                 /var/lib/t3/tmp/basinrace/target-race
build race-se2   "$COMBO,se2-rigidity-certificate"        /var/lib/t3/tmp/basinrace/target-se2
echo BUILD_OK
