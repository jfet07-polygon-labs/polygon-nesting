#!/usr/bin/env bash
# The authentic v4 work battery Sol review 6 §1 finding 1 asked for.
#
# The configuration is the one the previous round claimed and did not run:
# `v3=1,sched=1,barren=16,divq=1` on a `compression-schedule` build. The spec
# is written out key by key rather than left to the defaults so the evidence
# document carries the configuration verbatim and cannot be misread the way
# `battery-fixed-sched.json`'s `"v3": false` was.
#
# On `barren=16` rather than Sol's `barren=1`: `barren` is not a boolean in
# this binary's spec parser (general_request_benchmark.rs:1776 parses it as a
# `usize` patience), and the *true* v4 default is
# `BARREN_ACTION_PATIENCE = 16` (portfolio.rs:2822). `barren=1` would run a
# queue sixteen times more impatient than v4, which is not the v4
# configuration. The literal reading is measured separately by
# `run-barren1-check.sh` so both are on the record.
set -u
D=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1/docs/experiments/coordinator-v5-budget-debit/drivers
BIN=/var/lib/t3/tmp/wf6v1/bin
cd "$D" || exit 1
V4='v3=1,sched=1,barren=16,divq=1'
for units in 40000000 120000000; do
  echo "########## work=${units} ##########"
  python3 pairedbattery.py "work-${units}" 3 mixed-61 0,1,2 \
    "work=${units},cells={cells},${V4}" \
    "fixed=${BIN}/sched-fixed" "unfixed=${BIN}/sched-unfixed"
done
