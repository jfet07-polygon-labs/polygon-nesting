#!/usr/bin/env bash
# The rest of the programme, on the frozen binary, in one window.
#
# Stage 1 - the four pinned gates - is already done and its JSON is in
# `out/gates-{ship,base}.json`.
#
# Two trims against `run-all.sh`, both against the clock and both stated in the
# README rather than hidden: the 30-second row of the anytime table is mixed-61
# only, which is the fixture the 36.39 s overrun happened on and the only one
# whose depth moves at that budget; and the first-tranche sweep drops the 0.75
# arm, which the pilot had already measured as indistinguishable from 0.6 at ten
# seconds.
set -u
cd /var/lib/t3/tmp/replan
BIN=bin/ship-meas
export PLAN_BIN="/var/lib/t3/tmp/replan/$BIN"

echo "########## 1. the four pinned gates"
python3 drivers/gates.py ship /var/lib/t3/tmp/replan/bin/ship-gate \
  /var/lib/t3/tmp/replan/gates/ship > out/gates-ship.json
echo "EXIT_gates_ship=$?"
python3 drivers/gates.py base /var/lib/t3/tmp/replan/bin/base-gate \
  /var/lib/t3/tmp/replan/gates/base > out/gates-base.json
echo "EXIT_gates_base=$?"

echo "########## 2. the refactor gate: base binary vs this one, work budget"
python3 drivers/equiv.py out/f-refactor bin/base-meas "$BIN" \
  mixed-61,shapes-17,triangle-20 0,1,2 30000000
echo "EXIT_refactor=$?"

echo "########## 3. Sol's concatenation gate"
for N in 400000 25000; do
  python3 drivers/equiv.py "out/f-concat-$N" "$BIN" "$BIN" \
    mixed-61,shapes-17,triangle-20 0,1,2 30000000 '' "m34batch=$N"
  echo "EXIT_concat_$N=$?"
done
python3 drivers/equiv.py out/f-concat-120M "$BIN" "$BIN" \
  mixed-61 0,1,2 120000000 '' 'm34batch=100000'
echo "EXIT_concat_120M=$?"

echo "########## 4. determinism across two processes"
python3 drivers/determinism.py out/det-work "$BIN" \
  mixed-61,shapes-17,triangle-20 0,1,2 work 30000000
echo "EXIT_det_work=$?"
python3 drivers/determinism.py out/det-plan "$BIN" \
  mixed-61,shapes-17,triangle-20 0,1,2 plan 10000
echo "EXIT_det_plan=$?"
python3 drivers/determinism.py out/det-replan "$BIN" \
  mixed-61,shapes-17,triangle-20 0,1,2 plan 10000 'replan=1'
echo "EXIT_det_replan=$?"

echo "########## 4b. the first tranche, with the horizon bounded"
python3 drivers/trancheq.py out/cal-first "$BIN" mixed-61 \
  10000,30000 0,1,2 off,1.0,0.6 2
echo "EXIT_cal_first=$?"

echo "########## 5. the twenty-round battery, mixed-61 at ten seconds"
python3 drivers/planbattery.py out/battery-10s "$BIN" mixed-61 10000 0,1,2 20 \
  plan,replan,wall
echo "EXIT_battery=$?"

echo "########## 6a. the anytime table, three fixtures, 3 s and 10 s"
python3 drivers/anytime.py out/anytime "$BIN" \
  mixed-61,shapes-17,triangle-20 0,1,2 3000,10000 plan,replan,wall
echo "EXIT_anytime=$?"

echo "########## 6b. the thirty-second cell, mixed-61"
python3 drivers/anytime.py out/anytime30 "$BIN" \
  mixed-61 0,1,2 30000 plan,replan,wall
echo "EXIT_anytime30=$?"

echo "########## 7. the checkpoint's consumer: cap the slice at what is left"
python3 drivers/trancheq.py out/cap-30s "$BIN" mixed-61 30000 0,1,2 \
  capoff,capon 2
echo "EXIT_cap=$?"

echo "ALLDONE"
