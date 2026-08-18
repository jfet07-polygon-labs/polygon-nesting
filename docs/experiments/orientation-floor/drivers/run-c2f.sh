#!/bin/bash
# The four-instrument interleave: the ladder rung, the deep entry grid, tier H
# (entry -> global legalization) and tier I (the rotation entry). Mode 26 and
# mode 34 stay in at low frequency because their yield is basin-shaped and this
# round keeps moving the basin.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export BENCH_BIN=/var/lib/t3/tmp/wf87/target-l10/release/examples/general_request_benchmark
export SCHED_BIN=/var/lib/t3/tmp/wf87/target-sched-l10/release/examples/general_request_benchmark
export C2_FLAT=0.0005,0.001,0.0015,0.002,0.0025,0.003,0.004,0.005,0.006,0.008,0.01,0.015,0.02,0.03,0.05,0.08,0.1,0.15,0.2
export C2_LEGAL_FLAT=0.01,0.02,0.03,0.04,0.05,0.06,0.08,0.1,0.12,0.15,0.2,0.25,0.3,0.4,0.5,0.7,1.0
export C2_LEGAL_MODES=30,31
export C2_ROT_KS=1,2,3,5
export C2_ROT_DEGS=0.00128,-0.00128,0.0032,-0.0032,0.008,-0.008,0.02,-0.02
export C2_ROT_MODES=30,33
export C2_A_EVERY=1
export C2_C_EVERY=8
export C2_M26_EVERY=8
export C2_M26_DROPS=1.0,0.3
export C2_M26_SEEDS=0
export C2_SCHED_EVERY=16
export C2_CROSS_EVERY=16
export C2_DEADLINE=$1
python3 cascade2.py "$4" "$2" "$3" 2000 5
