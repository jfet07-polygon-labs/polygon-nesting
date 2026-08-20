#!/usr/bin/env bash
# The measurement binary `sched-fixed` was built before this round's last two
# edits: a doc comment on `schedule_self_cost_units` and two added
# `#[cfg(test)]` tests. Neither can reach the release example's generated code.
# "Cannot" is an argument, though, and this round is about not settling for
# those - so rebuild from the tree as committed and re-run the battery cell the
# headline rests on, seed 2 at 40M, comparing the whole document.
set -u
ROOT=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1
T=/var/lib/t3/tmp/wf6v1
cd "$ROOT" || exit 1

CARGO_TARGET_DIR=$T/t-sched-fixed cargo build --release \
  --example general_request_benchmark \
  --features jagua-experimental,compression-schedule -j 6 2>&1 | tail -3
cp "$T/t-sched-fixed/release/examples/general_request_benchmark" \
   "$T/bin/sched-fixed-rebuilt"
sha256sum "$T/bin/sched-fixed" "$T/bin/sched-fixed-rebuilt"

cd "$ROOT/docs/experiments/coordinator-v5-budget-debit/drivers" || exit 1
python3 - <<'PY'
import json
import os
import sys
sys.path.insert(0, os.getcwd())
import hashlib
import runlibv6 as runlib
import gatedocdiff

# `gatedocdiff.VOLATILE` was written for gate documents, which carry no
# portfolio timeline. A portfolio run stamps a wall-clock second on every
# archived basin, every publication and every operator call, and those are a
# reading of a shared box - the original of this cell ran at load 20+ and took
# 36.2 s, the rebuild ran nearly idle and took 19.6 s. Excluded by name, and
# named here rather than folded silently into VOLATILE.
SECONDS = {'birthSeconds', 'publishedSeconds', 'startedSeconds',
           'elapsedSeconds', 'seconds', 'occupancyOverTime', 'processSeconds'}


def strip(node):
    if isinstance(node, dict):
        return {k: strip(v) for k, v in sorted(node.items())
                if k not in gatedocdiff.gatelib.VOLATILE
                and k not in gatedocdiff.IDENTITY and k not in SECONDS}
    if isinstance(node, list):
        return [strip(v) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def digest(doc):
    return hashlib.sha256(
        json.dumps(strip(doc), sort_keys=True).encode()).hexdigest()


spec = ('work=40000000,cells=15:23:31:39,v3=1,sched=1,barren=16,divq=1')
out = f'{runlib.OUT}/rebuild-check/fixed-s2.json'
doc, wall, _ = runlib.run(f'{runlib.OUT}/bin/sched-fixed-rebuilt',
                          'mixed-61', 2, spec, out)
before = json.load(open(f'{runlib.OUT}/work-40000000/runs/fixed-s2-r0.json'))
result = {
    'depthRebuilt': doc.get('independentUsedLongAxisDepthMm'),
    'depthOriginal': before.get('independentUsedLongAxisDepthMm'),
    'workRebuilt': doc['portfolio']['workUnits'],
    'workOriginal': before['portfolio']['workUnits'],
    'debitRebuilt': sum(c.get('debitedUnits') or 0
                        for c in doc['portfolio']['operatorCalls']),
    'debitOriginal': sum(c.get('debitedUnits') or 0
                         for c in before['portfolio']['operatorCalls']),
    'identicalExceptWallClock': digest(doc) == digest(before),
    'digest': digest(doc)[:16],
    'wallRebuiltSeconds': round(wall, 2),
}
print(json.dumps(result, indent=1))
json.dump(result, open(f'{runlib.OUT}/rebuild-check/verdict.json', 'w'),
          indent=1)
PY
