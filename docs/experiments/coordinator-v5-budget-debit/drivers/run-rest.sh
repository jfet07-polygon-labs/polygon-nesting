#!/usr/bin/env bash
# The three batteries that follow the main 40M/120M work pair, in the order
# their results are needed and each run alone on the box as far as this agent
# controls it. The wall curves go last because they are the only ones whose
# numbers a busy box can move.
set -u
D=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1/docs/experiments/coordinator-v5-budget-debit/drivers
cd "$D" || exit 1
echo "##### load before: $(uptime)"
bash run-equal-true-cost.sh
echo "##### load: $(uptime)"
bash run-barren1-check.sh
echo "##### load: $(uptime)"
bash run-wall-curves.sh
echo "##### load after: $(uptime)"
