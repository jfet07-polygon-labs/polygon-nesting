#!/usr/bin/env bash
# Rebuilds the measurement binary from the current tree and prints its hash
# beside the one the evidence was measured on.
#
#   rehash.sh
#
# Used to check that a source edit made after a measurement run - a visibility
# change, a comment - really did leave the binary alone, rather than assuming it.
set -eu
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-2
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/lib/t3/tmp/cblock/tgt}"
cd "$W"
COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator,se2-rigidity-certificate,contact-block-se2
cargo build --release --example general_request_benchmark --features "$COMBO" 2>&1 | tail -1
echo "rebuilt:  $(sha256sum "$CARGO_TARGET_DIR/release/examples/general_request_benchmark" | cut -d' ' -f1)"
echo "measured: $(sha256sum /var/lib/t3/tmp/cblock/bin/meas | cut -d' ' -f1)"
