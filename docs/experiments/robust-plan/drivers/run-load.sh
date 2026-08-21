#!/usr/bin/env bash
# One battery under a competing load this round owns.
#
#   bash run-load.sh OUTDIR BINARY TARGET_MS SEEDS ROUNDS ARMS WORKERS DUTY
#
# The load is started first, given two seconds to reach steady state, and killed
# on the way out whatever happens - including on a failure, which is what the
# trap is for: a stress process outliving its battery would poison every
# measurement taken after it, and the next thing this round runs is the quiet
# battery.
#
# `stress.py`'s own stdout - its PID, its parameters and one load-average line
# per second - is kept beside the battery's, so the window can be reconstructed
# from the evidence rather than from a memory of what was running.
set -u
DRIVERS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTDIR="$1"; BINARY="$2"; TARGET="$3"; SEEDS="$4"; ROUNDS="$5"; ARMS="$6"
WORKERS="${7:-8}"; DUTY="${8:-0.7}"
mkdir -p "$OUTDIR"

python3 "$DRIVERS/stress.py" "$WORKERS" "$DUTY" > "$OUTDIR/stress.log" 2>&1 &
STRESS=$!
cleanup() {
  kill -TERM "$STRESS" 2>/dev/null
  # The workers are daemons of the parent, but a parent killed mid-fork can
  # leave one behind, so the group is swept too.
  pkill -TERM -P "$STRESS" 2>/dev/null
  wait "$STRESS" 2>/dev/null
}
trap cleanup EXIT INT TERM
sleep 2

python3 "$DRIVERS/planbattery.py" "$OUTDIR" "$BINARY" mixed-61 "$TARGET" \
  "$SEEDS" "$ROUNDS" "$ARMS" > "$OUTDIR/planbattery.log" 2>&1
STATUS=$?
echo "BATTERY_EXIT=$STATUS"
tail -3 "$OUTDIR/stress.log"
exit "$STATUS"
