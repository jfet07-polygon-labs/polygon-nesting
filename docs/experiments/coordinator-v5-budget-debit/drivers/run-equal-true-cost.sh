#!/usr/bin/env bash
# The experiment that decides how to read the 40M regression.
#
# At `work=40000000` the unfixed arm's *true* spend is 41.2M / 41.8M / 51.3M
# (seeds 0/1/2) - it overran its nominal budget by up to 28% because the meter
# never felt the schedule's own price. So "fixed is 4.4 mm worse at 40M" is not
# a like-for-like statement: the two arms did not do the same amount of work.
#
# This raises the fixed arm's nominal budget to 52M, which is above every
# unfixed true spend at the 40M point, and asks the only fair question: at
# equal *true* work, does the honest accounting cost anything at all, or was
# the whole difference the overrun?
#
# The unfixed arm is run at 52M too, so the point is paired rather than
# compared against a number from another battery.
set -u
D=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1/docs/experiments/coordinator-v5-budget-debit/drivers
BIN=/var/lib/t3/tmp/wf6v1/bin
cd "$D" || exit 1
python3 pairedbattery.py work-52000000 2 mixed-61 0,1,2 \
  'work=52000000,cells={cells},v3=1,sched=1,barren=16,divq=1' \
  "fixed=${BIN}/sched-fixed" "unfixed=${BIN}/sched-unfixed"
