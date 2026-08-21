#!/usr/bin/env bash
# The measurements, on the committed binary, after the gates have passed.
#
#   run-measure.sh
#
# Order matters and is the protocol's: the calibration pass first, because every
# arm below is a *calibrated plan* arm and a battery that calibrated itself
# mid-flight would be measuring the order its own rounds ran in.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_60cc1332-b41-1
D="$W/docs/experiments/real-interruption/drivers"
E="$W/docs/experiments/real-interruption/evidence"
T=/var/lib/t3/tmp/realint
B="$T/bin"
cd "$W" || exit 3
mkdir -p "$E" "$T/out" "$T/cal"

echo "== the calibration pass, three rounds, on the committed binary"
python3 "$D/calpass.py" "$T/out/cal" "$B/ship-meas" 3 > "$T/cal.log" 2>&1
echo "calpass exit=$?"
tail -4 "$T/cal.log"
cp "$T/cal/live.json" "$E/cal-live.json"
cp "$T/cal/probe.json" "$E/cal-probe.json"

echo
echo "== the bound sweep at ten seconds, mixed-61, 3 seeds x 3 rounds"
python3 "$D/boundsweep.py" "$T/out/bound10" "$B/ship-meas" mixed-61 10000 \
  0,1,2 3 base,past25,past50,past100,pastwall,wallstop,yield2 \
  > "$T/bound10.log" 2>&1
echo "bound10 exit=$?"
cp "$T/out/bound10/summary.json" "$E/bound-10s.json"

echo
echo "== the bound sweep at thirty seconds, where the overrun lives"
python3 "$D/boundsweep.py" "$T/out/bound30" "$B/ship-meas" mixed-61 30000 \
  0,1,2 3 base,past50,past100,pastwall,wallstop \
  > "$T/bound30.log" 2>&1
echo "bound30 exit=$?"
cp "$T/out/bound30/summary.json" "$E/bound-30s.json"

echo
echo "== the density point, with the bound unlocked"
python3 "$D/boundsweep.py" "$T/out/density" "$B/ship-meas" mixed-61 10000 \
  0,1,2 2 base,grid25,past100,past100grid25,past50grid25 \
  > "$T/density.log" 2>&1
echo "density exit=$?"
cp "$T/out/density/summary.json" "$E/density-past.json"

echo
echo "== the anytime table, three fixtures, three budgets, two processes"
PLAN_LEVER="${PLAN_LEVER:-m34wallstop=1}" \
python3 "$D/anytime.py" "$T/out/anytime" "$B/ship-meas" \
  mixed-61,shapes-17,triangle-20 0,1,2 3000,10000 callive,wall,lever \
  > "$T/anytime.log" 2>&1
echo "anytime exit=$?"
cp "$T/out/anytime/anytime.json" "$E/anytime.json"

echo
echo "== the thirty-second cell, all three fixtures"
PLAN_LEVER="${PLAN_LEVER:-m34wallstop=1}" \
python3 "$D/anytime.py" "$T/out/anytime30" "$B/ship-meas" \
  mixed-61,shapes-17,triangle-20 0,1,2 30000 callive,wall,lever \
  > "$T/anytime30.log" 2>&1
echo "anytime30 exit=$?"
cp "$T/out/anytime30/anytime.json" "$E/anytime30.json"
