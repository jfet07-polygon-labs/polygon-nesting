#!/bin/bash
# Basin diversity: the finer-ladder round's first adoption came from a *lineage*
# pin 0.001 mm worse than the incumbent, not from the incumbent itself, so the
# new floor is fired at the two near-tie ancestors as well as at the incumbent.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export BENCH_BIN=/var/lib/t3/tmp/wf87/target-l10/release/examples/general_request_benchmark
TC=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-line-cascade
G=0.0005,0.001,0.0015,0.002,0.0025,0.003,0.004,0.005,0.006,0.008,0.01

# Judged against the *incumbent's* raw, not their own: a runner-up basin only
# matters here if it reaches below the line.
python3 flatsweep.py l10-flat-4563 "$TC/pinned-fs-155.4563.json" 155.42196626072334 "$G" 0.05,2.0 33 2
python3 flatsweep.py l10-flat-4633 "$TC/pinned-fs-155.4633.json" 155.42196626072334 "$G" 0.05,2.0 33 2
python3 flatsweep.py l10-flat-60914 "$TC/pinned-fs-156.0914.json" 155.42196626072334 "$G" 0.05,2.0 33 2
echo BASINS_DONE
