#!/usr/bin/env bash
# Copies this round's measured documents out of the scratch tree and into the
# experiment's `evidence/` directory, and regenerates the derived tables.
#
# The per-run documents themselves are 100-500 KB each and there are ninety of
# them; only the battery summaries, the derived checks and the two gate
# documents are committed. The scratch paths are recorded in the README so the
# raw runs can be found on this box for as long as it lives.
set -u
E=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1/docs/experiments/coordinator-v5-budget-debit/evidence
D=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1/docs/experiments/coordinator-v5-budget-debit/drivers
T=/var/lib/t3/tmp/wf6v1
cd "$D" || exit 1
mkdir -p "$E/round6"

for name in work-40000000 work-120000000 work-52000000 barren1-40000000 \
            wall-3000 wall-10000 wall-30000; do
  if [ -f "$T/$name/battery.json" ]; then
    python3 -c "
import json, sys
doc = json.load(open('$T/$name/battery.json'))
# The per-run rows carry every schedule action and every operator call; keep
# them, drop nothing, but drop the raw archive dumps the summarizer already
# removed. They are what makes the battery re-checkable without the 500 KB
# run documents.
json.dump(doc, open('$E/round6/battery-$name.json', 'w'), indent=1)
"
    python3 batterytable.py "$T/$name/battery.json" \
      > "$E/round6/table-$name.md"
  fi
done

# Only the work-budget batteries have a true-cost story: a wall run reports no
# work units at all. Globs are expanded into a checked list rather than passed
# through, so one missing battery cannot silently empty the whole document.
COST=()
for name in work-40000000 work-120000000 work-52000000 barren1-40000000; do
  [ -f "$T/$name/battery.json" ] && COST+=("$T/$name/battery.json")
done
if [ ${#COST[@]} -gt 0 ]; then
  python3 truecost.py "${COST[@]}" > "$E/round6/truecost.json"
fi

for gate in fixed unfixed; do
  [ -f "$T/gates/gates-$gate.json" ] && \
    cp "$T/gates/gates-$gate.json" "$E/round6/gates-$gate.json"
done
[ -f "$T/gates-docdiff-round6.json" ] && \
  cp "$T/gates-docdiff-round6.json" "$E/round6/gates-docdiff.json"
for f in ordering-work40M ordering-work120M stampdelta-work40M \
         stampdelta-work120M; do
  [ -f "$T/$f.json" ] && cp "$T/$f.json" "$E/round6/$f.json"
done
[ -f "$T/rebuild-check/verdict.json" ] && \
  cp "$T/rebuild-check/verdict.json" "$E/round6/measurement-binary-rebuild.json"
for f in suite-jagua suite-jagua-sched portfolio-unit-tests; do
  [ -f "$T/$f.log" ] && tail -c 200000 "$T/$f.log" > "$E/round6/$f.log"
done

{
  echo "# The four binaries this round measured with."
  echo "# gate-*   : --features jagua-experimental"
  echo "# sched-*  : --features jagua-experimental,compression-schedule"
  echo "# *-fixed  : this branch; *-unfixed : f32c629, before the debit existed"
  sha256sum "$T"/bin/gate-fixed "$T"/bin/sched-fixed \
            "$T"/bin/gate-unfixed "$T"/bin/sched-unfixed
} > "$E/round6/binaries.txt"

git -C /var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1 \
  diff f32c629 -- crates/polygon-nesting-core/src/search/portfolio.rs \
  crates/polygon-nesting-core/examples/general_request_benchmark.rs \
  > "$E/round6/portfolio-rs.diff"

ls -la "$E/round6"
