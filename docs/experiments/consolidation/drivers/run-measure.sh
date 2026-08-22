#!/usr/bin/env bash
# The measurements, on the committed binaries, after the gates have passed.
#
#   run-measure.sh
#
# Order matters and is the protocol's: the calibration pass first, because every
# thirty-second arm below is a *calibrated plan* arm and a battery that
# calibrated itself mid-flight would be measuring the order its own rounds ran
# in.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-1
D="$W/docs/experiments/consolidation/drivers"
E="$W/docs/experiments/consolidation/evidence"
T=/var/lib/t3/tmp/consol
B="$T/bin"
cd "$W" || exit 3
mkdir -p "$E" "$T/out" "$T/cal"

echo "== the calibration pass, three rounds, on the committed binary"
python3 "$D/calpass.py" "$T/out/cal" "$B/ship-meas" 3 > "$T/cal.log" 2>&1
echo "calpass exit=$?"
cp "$T/cal/live.json" "$E/cal-live.json"
cp "$T/cal/probe.json" "$E/cal-probe.json"

echo
echo "== the debit's cost in seconds, at the canonical 10 s plan budget"
python3 "$D/workwall.py" "$T/out/ww10" "$B/ship-meas" mixed-61 0,1,2 \
  24891457 4 > "$T/ww10.log" 2>&1
echo "ww10 exit=$?"
cp "$T/out/ww10/workwall.json" "$E/workwall-25M.json"

echo
echo "== the same, at the thirty-second band, for scale"
python3 "$D/workwall.py" "$T/out/ww30" "$B/ship-meas" mixed-61 0,1,2 \
  120000000 2 > "$T/ww30.log" 2>&1
echo "ww30 exit=$?"
cp "$T/out/ww30/workwall.json" "$E/workwall-120M.json"

echo
echo "== the meter tax at a fixed wall: calibrated-plan 9 re-measured, split"
python3 "$D/metertax.py" "$T/out/metertax" "$B/ship-meas" mixed-61 10000 \
  0,1,2 3 > "$T/metertax.log" 2>&1
echo "metertax exit=$?"
cp "$T/out/metertax/metertax.json" "$E/metertax-10s.json"

echo
echo "== the plan battery at ten seconds: what a caller actually gets"
python3 "$D/planbattery.py" "$T/out/plan10" "$B/ship-meas" mixed-61 10000 \
  0,1,2 3 plan,plandebit,callive,caldebit > "$T/plan10.log" 2>&1
echo "plan10 exit=$?"
cp "$T/out/plan10/planbattery.json" "$E/planbattery-10s.json"

echo
echo "== the wall stop at thirty seconds, where the overrun lives"
python3 "$D/planbattery.py" "$T/out/wall30" "$B/ship-meas" mixed-61 30000 \
  0,1,2 3 callive,calwallstop,calwallstopall,calwallreserve \
  > "$T/wall30.log" 2>&1
echo "wall30 exit=$?"
cp "$T/out/wall30/planbattery.json" "$E/wallstop-30s.json"

echo
echo "== determinism, work mode, both arms"
python3 "$D/determinism.py" "$T/out/det-work" "$B/ship-meas" \
  mixed-61,shapes-17,triangle-20 0,1,2 work 30000000 > "$T/det-work.log" 2>&1
echo "det-work exit=$?"
cp "$T/out/det-work/determinism.json" "$E/determinism-work.json"

python3 "$D/determinism.py" "$T/out/det-debit" "$B/ship-meas" \
  mixed-61,shapes-17,triangle-20 0,1,2 work 30000000 lanedebit=1 \
  > "$T/det-debit.log" 2>&1
echo "det-debit exit=$?"
cp "$T/out/det-debit/determinism.json" "$E/determinism-work-debit.json"

echo
echo "== determinism, plan mode, the wall stop armed on every class"
python3 "$D/determinism.py" "$T/out/det-wallstop" "$B/ship-meas" \
  mixed-61 0,1,2 plan 30000 \
  "plancal=$T/cal/live.json,m34wallstopall=1" > "$T/det-wallstop.log" 2>&1
echo "det-wallstop exit=$?"
cp "$T/out/det-wallstop/determinism.json" "$E/determinism-wallstop.json"

echo
echo "== the equivalence gate: the debit arm is the same search"
python3 "$D/equiv.py" "$T/out/equiv-debit" "$B/ship-meas" "$B/ship-meas" \
  mixed-61,shapes-17,triangle-20 0,1,2 30000000 '' 'lanedebit=1' \
  > "$T/equiv-debit.log" 2>&1
echo "equiv-debit exit=$?"
cp "$T/out/equiv-debit/equiv.json" "$E/equiv-debit.json"

echo
echo "== the equivalence gate: this head is the base binary with every key off"
python3 "$D/equiv.py" "$T/out/equiv-head" "$B/base-combo" "$B/ship-meas" \
  mixed-61,shapes-17,triangle-20 0,1,2 30000000 '' '' \
  > "$T/equiv-head.log" 2>&1
echo "equiv-head exit=$?"
cp "$T/out/equiv-head/equiv.json" "$E/equiv-head.json"

echo "MEASURE DONE"
