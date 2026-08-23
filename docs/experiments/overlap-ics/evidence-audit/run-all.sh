#!/usr/bin/env bash
# **The whole evidence audit, in order.**
#
#     bash run-all.sh [<root>] [<work-dir>]
#
# `root` defaults to the repository containing this script; `work-dir` to
# `/var/lib/t3/tmp/ics-audit`. Every stage's exit status is captured on its own
# line immediately after the command - never through a pipe - and the script's
# own status is the OR of the stages that assert something.
#
# Stages, and what each one needs:
#
#   1. counters      committed evidence only, no binary            (seconds)
#   2. bites         committed evidence only, no binary            (seconds)
#   3. strike        committed evidence only, no binary            (seconds)
#   4. rust          the release library, its own detached package (minutes)
#   5. cells         nine 10.000 s wall cells                      (~95 s)
#   6. frame         reads stage 5's documents                     (seconds)
#   7. chain         reads stage 5's and stage 8's documents       (seconds)
#   8. replay        three fixed-work replays, two processes each  (minutes)
#   9. names         reads stage 5's documents                     (seconds)
#
# Stages 1-4 and 7-9 are the audit proper. Stages 5 and 8 produce the raw
# documents the committed `wall.json` reduction dropped; they are wall runs and
# their trajectories are load-bound, so nothing in this script compares a wall
# trajectory across machines. The fixed-work replay in stage 8 does compare, and
# is the only cross-machine reproduction claim made here.
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$(cd "$HERE/../../../.." && pwd)}"
WORK="${2:-/var/lib/t3/tmp/ics-audit}"
EVIDENCE="$ROOT/docs/experiments/overlap-ics/cutclose-rerun/evidence"
mkdir -p "$WORK"
failed=""

echo "== 1. counter identities"
python3 "$HERE/counters.py" "$EVIDENCE" "$WORK/counters.json"
status=$?; echo "counters exit=$status"
[ "$status" -eq 0 ] || failed="$failed counters"

echo "== 2. bite consistency"
python3 "$HERE/bites-consistency.py" "$EVIDENCE/wall.json" "$WORK/bites-rerun.json"
status=$?; echo "bites exit=$status"
[ "$status" -eq 0 ] || failed="$failed bites"

echo "== 3. the strike repair's own populations"
python3 "$HERE/strike-effect.py" "$EVIDENCE/round1-bites-red.json" \
        "$EVIDENCE/wall.json" "$WORK/strike-effect.json" > "$WORK/strike-effect.txt"
status=$?; echo "strike exit=$status"
[ "$status" -eq 0 ] || failed="$failed strike"

echo "== 4. the Rust vectors"
( cd "$HERE/rust-vectors" && cargo build --release )
status=$?; echo "rust build exit=$status"
if [ "$status" -eq 0 ]; then
  "$HERE/rust-vectors/target/release/audit" "$ROOT" "$WORK/rust-vectors.json" > /dev/null
  status=$?; echo "rust vectors exit=$status"
fi
[ "$status" -eq 0 ] || failed="$failed rust"

echo "== 5. nine 10.000 s wall cells"
bash "$HERE/run-cells.sh" "$ROOT" "$WORK/cells9" 0 1 2 3 4 5 6 7 8
status=$?; echo "cells exit=$status"
[ "$status" -eq 0 ] || failed="$failed cells"

echo "== 6. the checkpoint clock frame"
python3 "$HERE/checkpoint-frame.py" "$WORK/cells9" "$WORK/checkpoint-frame.json" \
        > "$WORK/checkpoint-frame.txt"
status=$?; echo "frame exit=$status"

echo "== 8. three fixed-work replays"
python3 "$HERE/replay.py" "$ROOT" "$EVIDENCE/wall.json" "$WORK/replay" 0 1 5 \
        "--out=$WORK/replay.json" > "$WORK/replay.txt"
status=$?; echo "replay exit=$status"
[ "$status" -eq 0 ] || failed="$failed replay"

echo "== 7. the publication chain"
bash "$HERE/run-chain.sh" "$HERE" "$WORK/chain.json" "$WORK/cells9" "$WORK/replay"
status=$?; echo "chain exit=$status"
[ "$status" -eq 0 ] || failed="$failed chain"

echo "== 9. what the funnel's rungs count"
python3 "$HERE/funnel-names.py" "$WORK"/cells9/wall-*.json \
        "--out=$WORK/funnel-names.json" > "$WORK/funnel-names.txt"
status=$?; echo "names exit=$status"

echo "== driver repair red/green"
python3 "$HERE/driver-fix-vector.py" "$ROOT/docs/experiments/overlap-ics/drivers" \
        "$WORK/cells9" > "$WORK/driver-fix-vector.json"
status=$?; echo "driver-fix exit=$status"

if [ -n "$failed" ]; then
  echo "AUDIT_FAILED:$failed"
  exit 1
fi
echo "AUDIT_PASS"
exit 0
