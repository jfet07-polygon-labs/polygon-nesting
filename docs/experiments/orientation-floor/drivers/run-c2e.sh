#!/bin/bash
# The tier mix the c2d measurements support. Two changes, both measured rather
# than guessed:
#
#  * mode 22 moves to every round. It was 0 of 24 below on the 155.4137 state
#    and 48 of 48 below across the two c2d rounds it was allowed to run in,
#    winning two of them outright. Its yield is not steady, it is *conditional*
#    on the state having just been moved by a legalization arm - which is
#    exactly the case a once-every-four-rounds schedule misses.
#  * the legalization entry grid gains its shallow half. The productive delta
#    walked 0.25 -> 0.1 -> 0.08 -> 0.05 as the state descended, i.e. down the
#    grid toward its own floor, so the floor is lowered ahead of it.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export BENCH_BIN=/var/lib/t3/tmp/wf87/target-l10/release/examples/general_request_benchmark
export SCHED_BIN=/var/lib/t3/tmp/wf87/target-sched-l10/release/examples/general_request_benchmark
TC=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-line-cascade
export C2_FLAT=0.0005,0.001,0.0015,0.002,0.0025,0.003,0.004,0.005,0.006,0.008,0.01,0.015,0.02,0.03,0.05,0.08,0.1,0.15,0.2
export C2_LEGAL_FLAT=0.01,0.02,0.03,0.04,0.05,0.06,0.08,0.1,0.12,0.15,0.2,0.25,0.3,0.4,0.5,0.7,1.0
export C2_LEGAL_MODES=30,31
export C2_A_EVERY=1
export C2_C_EVERY=8
export C2_M26_EVERY=6
export C2_M26_DROPS=1.0,0.55,0.3
export C2_M26_SEEDS=0,1
export C2_SCHED_EVERY=12
export C2_CROSS_EVERY=12
export C2_POOL="$TC/pinned-fs-155.4223.json:$TC/pinned-fs-155.4563.json:$TC/pinned-fs-155.4633.json"
export C2_DEADLINE=$1
python3 cascade2.py "$4" "$2" "$3" 2000 5
