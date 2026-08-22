#!/usr/bin/env bash
# The second half of the measurements: the mechanism test the calibrated
# battery cannot perform, and the debit's own calibration pass.
#
#   run-rest.sh
#
# Split from `run-measure.sh` only because the two halves were run in two
# windows; the order below is still the protocol's - the calibration pass
# before the battery that reads its file.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-1
D="$W/docs/experiments/consolidation/drivers"
E="$W/docs/experiments/consolidation/evidence"
T=/var/lib/t3/tmp/consol
B="$T/bin"
cd "$W" || exit 3
mkdir -p "$E" "$T/out" "$T/cal"

echo "== the calibration pass, with the debit armed, into its own file"
PLAN_CAL_LIVE="$T/cal/debit.json" \
PLAN_CAL_PROBE="$T/cal/debit-probe.json" \
PLAN_CAL_EXTRA=lanedebit=1 \
python3 "$D/calpass.py" "$T/out/caldebit" "$B/ship-meas" 3 mixed-61 0,1,2 \
  > "$T/caldebit.log" 2>&1
echo "caldebit exit=$?"
cp "$T/cal/debit.json" "$E/cal-debit.json"

echo
echo "== the shipping shape: the file the debit arm wrote, read by the debit arm"
python3 "$D/planbattery.py" "$T/out/plan10b" "$B/ship-meas" mixed-61 10000 \
  0,1,2 3 callive,caldebit,caldebitfile > "$T/plan10b.log" 2>&1
echo "plan10b exit=$?"
cp "$T/out/plan10b/planbattery.json" "$E/planbattery-10s-debitfile.json"

echo
echo "== the forced overrun: does the wall stop fire, and what does it bind"
python3 "$D/forcedoverrun.py" "$T/out/forced" "$B/ship-meas" mixed-61 10000 \
  0,1,2 2 3.0 > "$T/forced.log" 2>&1
echo "forced exit=$?"
cp "$T/out/forced/forcedoverrun.json" "$E/forced-overrun-10s.json"

echo "REST DONE"
