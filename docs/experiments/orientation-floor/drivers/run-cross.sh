#!/bin/bash
# Sibling crossover. The record-line round ran mode 23 against the *old* record
# co-states, which sit in a basin 4 mm behind, and measured 0 of 24 - a negative
# it correctly flagged as being about that pool rather than about the operator.
# This round has a pool the previous one did not: eight states of the same
# lineage inside 0.09 mm of each other, produced by four different instruments.
# If crossover is ever going to pay, a pool of genuine near-tie siblings is the
# case where it should.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export BENCH_BIN=/var/lib/t3/tmp/wf87/target-l10/release/examples/general_request_benchmark
PIN=$1
RAW=$2
P=/var/lib/t3/tmp/wf87/pins
C=/var/lib/t3/tmp/wf87/run/c2d/pins
POOL="$P/pinned-fs-155.42197.json:$P/pinned-fs-155.41373.json:$P/pinned-fs-155.39673.json"
POOL="$POOL:$C/pin-155.34181307831.json:$C/pin-155.33681307831.json"
POOL="$POOL:$C/pin-155.33281307831.json:$C/pin-155.33181307831.json"
python3 crosssweep.py cross-final "$PIN" "$RAW" "$POOL" 0.25,0.35,0.5,0.65,0.75 0 4
echo CROSS_DONE
