#!/usr/bin/env bash
# The stages the shipped `PLAN_FIRST_TRANCHE` can reach, on the final binary.
#
# The four pinned gates and the refactor gate are re-run so the round's headline
# claims sit on the binary that is committed; the concatenation gates and the
# two work-mode determinism gates are not, and §8.1 says why: they are
# `work=`-denominated, `install_plan` and `replan` are unreachable under a work
# budget, and the delta between the two builds is one constant that only those
# two functions read.
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

echo "########## 2. the refactor gate"
python3 drivers/equiv.py out/g-refactor bin/base-meas "$BIN" \
  mixed-61,shapes-17,triangle-20 0,1,2 30000000
echo "EXIT_refactor=$?"

echo "########## 4c. determinism, plan mode with re-planning"
python3 drivers/determinism.py out/g-det-replan "$BIN" \
  mixed-61,shapes-17,triangle-20 0,1,2 plan 10000 'replan=1'
echo "EXIT_det_replan=$?"

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
