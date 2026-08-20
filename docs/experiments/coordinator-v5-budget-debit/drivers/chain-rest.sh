#!/usr/bin/env bash
# Waits for the main 40M/120M work battery to finish, then runs the remaining
# three batteries. Chained rather than launched in parallel because the box is
# shared with two sibling agents already and the wall curves at the end of
# `run-rest.sh` are the one measurement here whose numbers a busy box moves.
set -u
D=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1/docs/experiments/coordinator-v5-budget-debit/drivers
while pgrep -f 'pairedbattery.py work-120000000' > /dev/null; do
  sleep 20
done
echo "##### main work battery finished at $(date +%H:%M:%S)"
bash "$D/run-rest.sh"
