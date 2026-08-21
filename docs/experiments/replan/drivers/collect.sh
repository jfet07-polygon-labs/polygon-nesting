#!/usr/bin/env bash
# Collects the summary JSON of every battery into the repository's evidence
# directory. The per-run documents stay in /var/lib/t3/tmp/replan/out - there
# are thousands of them and they are reproducible from the drivers - and what is
# committed is the summary each driver computed, plus the box's load trace.
#
# The source directory names carry the build each battery ran on, and
# `evidence/binaries.txt` is the key:
#   g-*   the committed binary, 9c049366385ecee2
#   f-*   the `PLAN_FIRST_TRANCHE = 0.6` build, 8201c5718b6c80f9
#   det-replan-stranded   the pre-stranding-fix build, 554044c3082f9184
#   cal-pilot-unbounded   the pre-horizon build, 15514f314505a97a
set -u
E=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_30e47560-32c-1/docs/experiments/replan/evidence
O=/var/lib/t3/tmp/replan/out
mkdir -p "$E"

cp "$O/gates-ship.json"  "$E/gates-ship.json"
cp "$O/gates-base.json"  "$E/gates-base.json"

copy_summary() {  # dir file target
  if [ -f "$O/$1/$2" ]; then cp "$O/$1/$2" "$E/$3"; else echo "MISSING $1/$2"; fi
}

# --- the committed binary
copy_summary g-refactor     equiv.json        refactor-equivalence.json
copy_summary g-det-replan   determinism.json  determinism-replan.json
copy_summary battery-10s    planbattery.json  battery-10s.json
copy_summary anytime        anytime.json      anytime.json
copy_summary anytime30      anytime.json      anytime30.json
copy_summary cap-30s        trancheq.json     cap-30s.json

# --- the 0.6 build: work-denominated gates and the sweep that chose 1.0
copy_summary f-concat-400000 equiv.json       concat-400k.json
copy_summary f-concat-25000  equiv.json       concat-25k.json
copy_summary f-concat-120M   equiv.json       concat-120M.json
copy_summary det-work       determinism.json  determinism-work.json
copy_summary det-plan       determinism.json  determinism-plan.json
copy_summary cal-first      trancheq.json     cal-first-tranche.json
# The same re-planning determinism gate at `planfirst=0.6`, which is the arm
# §9.3 rejected. Kept because "0.6 and 1.0 are indistinguishable at ten
# seconds" is a claim, and this is one of the places it can be checked.
copy_summary det-replan     determinism.json  determinism-replan-planfirst06.json

# --- the pre-stranding-fix build: the gate that caught it
copy_summary det-replan-stranded determinism.json determinism-replan-stranded.json
# --- the pre-horizon build: a third batch size, run before the trim
copy_summary g-concat-100000 equiv.json       concat-100k.json

# The pilot that forced `PLAN_TRANCHE_HORIZON`: it was stopped part-way, so it
# has no driver summary and is collected as the raw rows it does have.
python3 - "$O/cal-pilot-unbounded" "$E/cal-pilot-unbounded.json" <<'PY'
import glob
import json
import os
import sys

src, dst = sys.argv[1], sys.argv[2]
rows = []
for path in sorted(glob.glob(f'{src}/f*.json')):
    try:
        with open(path) as handle:
            doc = json.load(handle)
    except Exception:
        continue
    portfolio = doc.get('portfolio') or {}
    if not portfolio:
        continue
    tag = os.path.basename(path)[:-5]
    fraction, target, seed, rnd = tag.split('-')
    plan = portfolio.get('plan') or {}
    rows.append({
        'tag': tag,
        'fraction': fraction[1:],
        'targetMs': int(target[1:]),
        'seed': int(seed[1:]),
        'round': int(rnd[1:]),
        'coordinatorSeconds': portfolio['elapsedSeconds'],
        'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
        'dualGateValid': portfolio['incumbent']['dualGateValid'],
        'planUnits': plan.get('units'),
        'tranches': portfolio.get('tranches') or [],
        'trancheCalibration': portfolio.get('trancheCalibration') or [],
    })
with open(dst, 'w') as handle:
    json.dump({
        'note': ('The pilot that ran the re-plan with the extrapolation '
                 'unbounded - PLAN_TRANCHE_HORIZON did not exist yet. It was '
                 'stopped part-way through its second round once the failure '
                 'was identified, so it has no driver summary and these are '
                 'the raw rows. Binary: cal-meas, '
                 '15514f314505a97a48d87a178d860a7b6a8dbe84cca33e30c64d9826ab036619.'),
        'rows': rows,
    }, handle, indent=1)
print(f'pilot rows: {len(rows)}')
PY

cp "$O/boxload.tsv" "$E/boxload.tsv"
ls -la "$E"
