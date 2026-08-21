#!/usr/bin/env bash
# §6's equivalence run, on binaries rebuilt from the committed tree.
#
#   run-verify.sh BASEBIN
#
# Four pinned gates on both new binaries, whole-document flag-off reproduction
# against the base commit's binary, and the armed cross-process determinism
# gate. Every exit is captured directly by the drivers, never through a pipe.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_08a442a7-1aa-1
D="$W/docs/experiments/rotation-tax/drivers"
OUT="${V4_OUT:-/var/lib/t3/rt/out}"
BIN="${V4_BIN:?set V4_BIN to the committed measurement binary}"
GATE="${TAX_GATE_BIN:?set TAX_GATE_BIN to the committed gate binary}"
BASE="${1:?pass the base-commit measurement binary}"

echo "== gates, gate binary"
python3 "$D/gates.py" commit-gate "$GATE" "$OUT/gates-commit-gate"
echo "gates.py(gate) exit=$?"

echo "== gates, measurement binary, flag off"
python3 "$D/gates.py" commit-meas "$BIN" "$OUT/gates-commit-meas"
echo "gates.py(meas) exit=$?"

echo "== flag-off whole-document reproduction against the base commit"
python3 "$D/reproduce.py" reproduce "$BASE" "$BIN" \
    mixed-61,shapes-17,triangle-20 0,1,2 40000000
echo "reproduce.py exit=$?"

echo "== determinism across two processes, ARMED"
python3 "$D/determinism.py" determinism-crot \
    mixed-61,shapes-17,triangle-20 0,1,2 40000000 \
    'm34lanes=1,m34pconfirm=1,crot=1'
echo "determinism.py exit=$?"
