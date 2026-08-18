#!/bin/bash
# The tuned interleave. Round 0 of the deep-grid cascade measured which tiers
# pay at this depth: the entry-grid tiers (flatten -> modes 33 and 32, and the
# frontier-stack nudges) took 25 of 25 arms below, and modes 22, 31, 26 and 34
# took none while charging most of the wall clock. So the barren tiers are
# reduced in *frequency*, not dropped - dropping them is exactly how the
# previous round lost mode 26 for 555 arms, and mode 26's yield is basin-shaped,
# so it has to keep being asked.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export BENCH_BIN=/var/lib/t3/tmp/wf87/target-l10/release/examples/general_request_benchmark
export SCHED_BIN=/var/lib/t3/tmp/wf87/target-sched-l10/release/examples/general_request_benchmark
TC=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-line-cascade
export C2_FLAT=0.0005,0.001,0.0015,0.002,0.0025,0.003,0.004,0.005,0.006,0.008,0.01,0.015,0.02,0.025,0.03,0.04,0.05,0.06,0.08,0.1,0.15,0.2
export C2_A_EVERY=4
export C2_C_EVERY=4
export C2_M26_EVERY=4
export C2_M26_DROPS=1.0,0.55,0.3
export C2_M26_SEEDS=0,1
export C2_SCHED_EVERY=8
export C2_CROSS_EVERY=8
export C2_POOL="$TC/pinned-fs-155.4223.json:$TC/pinned-fs-155.4563.json:$TC/pinned-fs-155.4633.json"
export C2_DEADLINE=$1
python3 cascade2.py c2c "$2" "$3" 400 5
