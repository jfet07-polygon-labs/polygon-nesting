#!/bin/bash
# The finer sub-grid step probe: the step= curve is non-monotone (0.25 won,
# 0.5 published nothing, 2 and 4 accept zero confirmations), so it is searched
# rather than extrapolated. Three parents: the incumbent and the two near-tie
# ancestors, because mode 34's own precondition says its outputs enter clean and
# these three are 0.04 mm apart.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export SCHED_BIN=/var/lib/t3/tmp/wf87/target-sched-l10/release/examples/general_request_benchmark
SPECS='past=1,work=20000000,step=0.25;past=1,work=20000000,step=0.1875;past=1,work=20000000,step=0.125;past=1,work=20000000,step=0.0625;past=1,work=20000000,step=0.03125;past=1,work=60000000,step=0.125;past=1,work=60000000,step=0.0625;past=1,work=20000000,step=0.125,confirm=1'
TC=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-line-cascade

python3 schedsweep.py m34fine-inc /var/lib/t3/tmp/wf87/pins/pinned-fs-155.42197.json 155.42196626072334 0.3 "$SPECS" 5,0 3
python3 schedsweep.py m34fine-4563 "$TC/pinned-fs-155.4563.json" 155.45627292304914 0.3 "$SPECS" 5,0 3
python3 schedsweep.py m34fine-4633 "$TC/pinned-fs-155.4633.json" 155.46327292304915 0.3 "$SPECS" 5,0 3
echo M34FINE_DONE
