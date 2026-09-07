#!/usr/bin/env bash
T=/var/lib/t3/tmp/astra; W=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a
until grep -q FREEZE_DEEP_DONE $T/freeze-deep.log 2>/dev/null; do sleep 15; done
cat $T/freeze-deep.log
B=$(ls -t $T/frozen-* | grep -v 'instr\|f856d19\|1e913f4\|9c38526\|2b5f16d\|6aaa4e1\|d0c459b' | head -1); echo "binary $B"
grep -q 'ALL_IDENTICAL' $T/freeze-deep.log || { echo "NOT IDENTICAL; stop"; exit 1; }
# keep the demo's four documents (same binary lineage) but re-run everything from the frozen binary for one provenance
rm -f $T/deep/deep-*.json; rm -rf $T/deep/replays
bash $T/deep-run.sh $B
bash $T/deep-replays.sh $B
echo "== deep-cut.py"; python3 $W/docs/experiments/overlap-ics/bite-microscope/deep-cut.py $T/deep/deep-*.json --replays $T/deep/replays > $T/deep/deep-cut-output.txt 2>&1; wc -l $T/deep/deep-cut-output.txt
echo DEEP_CHAIN_DONE
