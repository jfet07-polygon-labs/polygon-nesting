#!/usr/bin/env bash
# Sol review 6 §1 writes the configuration to rerun as
# `v3=1,sched=1,barren=1,divq=1`. Three of those four keys are booleans in this
# binary's spec parser; `barren` is not - it is the barren-action patience,
# parsed as a `usize` (general_request_benchmark.rs:1776), whose v4 value is
# `BARREN_ACTION_PATIENCE = 16` (portfolio.rs:2822). The main battery runs the
# true v4 configuration, `barren=16`. This runs the literal reading as well, so
# the round cannot be accused of having chosen the more convenient of two
# readings - one paired round, three seeds, 40M only, which is where the debit
# was already shown to bind.
set -u
D=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1/docs/experiments/coordinator-v5-budget-debit/drivers
BIN=/var/lib/t3/tmp/wf6v1/bin
cd "$D" || exit 1
python3 pairedbattery.py barren1-40000000 1 mixed-61 0,1,2 \
  'work=40000000,cells={cells},v3=1,sched=1,barren=1,divq=1' \
  "fixed=${BIN}/sched-fixed" "unfixed=${BIN}/sched-unfixed"
