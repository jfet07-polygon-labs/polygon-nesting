#!/usr/bin/env bash
# Build the two binaries this round measures on, **from the committed tree**.
#
#   run-build.sh
#
# Sol review 9's provenance finding is the reason this is a script and not three
# lines in a README: the round it audited had *"una rottura netta fra sorgente,
# driver e binario misurato: probabilmente build da worktree non committato o
# driver modificato durante la raccolta"*. So this refuses to build a dirty
# tree, records the commit and the two sha256s beside the binaries, and every
# other script here takes a binary path rather than building one.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_60cc1332-b41-1
B=/var/lib/t3/tmp/realint/bin
E="$W/docs/experiments/real-interruption/evidence"
cd "$W" || exit 3
mkdir -p "$B" "$E"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

if [ -n "$(git status --porcelain)" ]; then
  echo "REFUSING: the tree is dirty. Commit first - the whole point of this"
  echo "script is that the binary and the source agree."
  git status --short
  exit 2
fi
COMMIT=$(git rev-parse HEAD)

echo "== gate binary: --features jagua-experimental"
cargo build --release --example general_request_benchmark \
  --features jagua-experimental > /var/lib/t3/tmp/realint/build-gate.log 2>&1
G=$?
echo "gate build exit=$G"
[ "$G" -eq 0 ] || exit "$G"
cp target/release/examples/general_request_benchmark "$B/gate-meas"

echo "== measurement binary: the full combo"
cargo build --release --example general_request_benchmark \
  --features "$COMBO" > /var/lib/t3/tmp/realint/build-combo.log 2>&1
C=$?
echo "combo build exit=$C"
[ "$C" -eq 0 ] || exit "$C"
cp target/release/examples/general_request_benchmark "$B/ship-meas"

{
  echo "commit $COMMIT"
  echo "features-gate jagua-experimental"
  echo "features-ship $COMBO"
  sha256sum "$B/gate-meas" "$B/ship-meas"
} | tee "$E/binaries.txt"
