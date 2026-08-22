#!/usr/bin/env bash
# The three binaries this round measures on, built from the committed tree.
#
#   build.sh [gate|meas|cb|all]
#
# `gate`  the protocol's gate binary: `jagua-experimental` and nothing else.
#         Run against the four pinned regression gates on BOTH builds, which is
#         how "flag-off bit-reproducing" is held.
# `cb`    the gate binary plus this round's feature only, so the flag-off
#         comparison isolates the feature rather than the whole combo.
# `meas`  the full combo plus this round's feature: the shipping stack the
#         matched-arm gate runs both its arms on.
set -eu
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-2
BIN=/var/lib/t3/tmp/cblock/bin
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/lib/t3/tmp/cblock/tgt}"
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
if [ "$WHICH" = cb ] || [ "$WHICH" = all ]; then
  build gate-cb jagua-experimental,contact-block-se2
fi
if [ "$WHICH" = meas ] || [ "$WHICH" = all ]; then
  build meas "$COMBO,se2-rigidity-certificate,contact-block-se2"
  build meas-base "$COMBO"
fi
