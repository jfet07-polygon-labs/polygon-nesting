#!/bin/bash
set -u
E=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1/docs/experiments/orientation-floor
cp /var/lib/t3/tmp/wf87/run/cert-final2.json "$E/evidence/cert-final2.json"
cp /var/lib/t3/tmp/wf87/run/replay-final.json "$E/evidence/replay-final.json"
cp /var/lib/t3/tmp/wf87/run/replay-final-base.json "$E/evidence/replay-final-basebinary.json"
cp /var/lib/t3/tmp/wf87/c2f-state.json "$E/evidence/cascade-c2f-state.json"
cp /var/lib/t3/tmp/wf87/c2f-cascade.log "$E/evidence/cascade-c2f.log"
cp /var/lib/t3/tmp/wf87/run/rotentry/sweep.json "$E/evidence/rotentry-155.33042.json"
cp /var/lib/t3/tmp/wf87/drivers/*.py "$E/drivers/"
cp /var/lib/t3/tmp/wf87/drivers/*.sh "$E/drivers/"
rm -f "$E/drivers/gate_lib_orig.py"
ls "$E/evidence" | wc -l
du -sh "$E"
