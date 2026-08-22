#!/usr/bin/env bash
# The two binaries this round measures on, built from the committed tree.
#
#   build.sh [gate|meas|all]
#
# `gate`  the protocol's gate binary: `jagua-experimental` and nothing else.
#         The four pinned regression gates run on this one.
# `meas`  the full shipping combo. Both arms of the audition run on it, because
#         the control is a mode-34 slice and mode 34 does not exist in a build
#         without `compression-schedule`, while `lanes=`/`pconfirm=` are unknown
#         spec keys without `parallel-compression-schedule`. Running the m26 arm
#         on the same binary is the point: the two arms differ by the mode
#         argument and nothing else.
set -eu
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f674db6b-1e0-2
BIN=/var/lib/t3/tmp/m26band/bin
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/lib/t3/tmp/m26band/tgt}"
mkdir -p "$BIN"
cd "$W"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator
WHICH="${1:-all}"

build() {
  local name="$1" features="$2"
  echo "== $name: $features"
  cargo build --release --example general_request_benchmark --features "$features"
  cp "$CARGO_TARGET_DIR/release/examples/general_request_benchmark" "$BIN/$name"
  sha256sum "$BIN/$name"
}

if [ "$WHICH" = gate ] || [ "$WHICH" = all ]; then
  build gate-base jagua-experimental
fi
if [ "$WHICH" = meas ] || [ "$WHICH" = all ]; then
  build meas "$COMBO"
fi
