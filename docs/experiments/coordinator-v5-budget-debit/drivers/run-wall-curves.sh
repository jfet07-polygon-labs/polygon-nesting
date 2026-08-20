#!/usr/bin/env bash
# The 3/10/30 s wall curves Sol review 6 §1 asked for, mixed-61, three seeds,
# three paired interleaved rounds per point.
#
# What this can and cannot show, stated before it is run rather than after:
# under a wall budget `BudgetMeter::debit_self_metered` returns zero by
# construction and `work_units_now()` is a constant, so the fixed and unfixed
# binaries should make *identical decisions* here. The curve is therefore not
# a search-quality A/B; it is the end-to-end version of the
# `a_wall_budget_never_debits_a_self_meter` unit test - if any depth moves,
# the no-op claim is false. Any wall-clock difference is the shared box.
set -u
D=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1/docs/experiments/coordinator-v5-budget-debit/drivers
BIN=/var/lib/t3/tmp/wf6v1/bin
cd "$D" || exit 1
V4='v3=1,sched=1,barren=16,divq=1'
for ms in 3000 10000 30000; do
  echo "########## wall=${ms}ms ##########"
  python3 pairedbattery.py "wall-${ms}" 3 mixed-61 0,1,2 \
    "wall=${ms},cells={cells},${V4}" \
    "fixed=${BIN}/sched-fixed" "unfixed=${BIN}/sched-unfixed"
done
