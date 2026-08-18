#!/bin/bash
set -u
E=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1/docs/experiments/orientation-floor
for f in knudge-legal m30seed legal-deeper legal-deeper31; do
  cp "/var/lib/t3/tmp/wf87/run/$f/sweep.json" "$E/evidence/$f-155.33042.json"
done
cp /var/lib/t3/tmp/wf87/suite-jagua.log "$E/evidence/suite-jagua-experimental.log"
cp /var/lib/t3/tmp/wf87/suite-sched.log "$E/evidence/suite-compression-schedule.log"
cp /var/lib/t3/tmp/wf87/drivers/*.py "$E/drivers/"
cp /var/lib/t3/tmp/wf87/drivers/*.sh "$E/drivers/"
rm -f "$E/drivers/gate_lib_orig.py"
ls "$E/evidence" | wc -l
du -sh "$E"
