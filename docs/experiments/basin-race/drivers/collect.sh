#!/usr/bin/env bash
# Every battery in this round, in one pass, on one set of binaries.
#
#   collect.sh [BINDIR] [OUTDIR]
#
# Order matters and is not alphabetical: the wall-sensitive batteries run first
# and alone, because `plan=<ms>` reads the clock once and a loaded box moves the
# plan onto a different rung of the ladder. The suites are last for the same
# reason - they saturate sixteen cores for twenty minutes.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_30e47560-32c-2
BIN="${1:-/var/lib/t3/tmp/basinrace/bin}"
OUT="${2:-/var/lib/t3/tmp/basinrace/out}"
D="$W/docs/experiments/basin-race/drivers"
E="$W/docs/experiments/basin-race/evidence"
P="$W/docs/experiments/parallel-compression-schedule/evidence/parents.json"
F=mixed-61,shapes-17,triangle-20
mkdir -p "$E"
cd "$W" || exit 3

echo "== gates"
python3 "$D/gates.py" race "$BIN/race-gate" "$OUT/gates/race" > "$OUT/gates-race.log" 2>&1
echo "gates race exit=$?"
python3 "$D/gates.py" base "$BIN/base-gate" "$OUT/gates/base" > "$OUT/gates-base.log" 2>&1
echo "gates base exit=$?"
cp "$OUT/gates/race/gates-race.json" "$E/gates-race.json"
cp "$OUT/gates/base/gates-base.json" "$E/gates-base.json"

echo "== race battery, salted draws"
python3 "$D/racebattery.py" "$OUT/battery-draw" "$BIN/race-combo" $F 0,1,2 10000 3:1:3
cp "$OUT/battery-draw/racebattery-10000.json" "$E/racebattery-draw-10s.json"

echo "== race battery, archive arms"
python3 "$D/racebattery.py" "$OUT/battery-arch" "$BIN/race-combo" $F 0,1,2 10000 '3:1:3,racedraw=0'
cp "$OUT/battery-arch/racebattery-10000.json" "$E/racebattery-archive-10s.json"

echo "== determinism, race on"
python3 "$D/determinism.py" "$OUT/det-on" "$BIN/race-combo" $F 0,1,2 plan 10000 'race=3:1:3'
cp "$OUT/det-on/determinism.json" "$E/determinism-plan-raceon.json"

echo "== determinism, race off"
python3 "$D/determinism.py" "$OUT/det-off" "$BIN/race-combo" $F 0,1,2 plan 10000 'race=0'
cp "$OUT/det-off/determinism.json" "$E/determinism-plan-raceoff.json"

echo "== determinism, base binary, triangle-20"
python3 "$D/determinism.py" "$OUT/det-base" "$BIN/base-combo" triangle-20 0,1,2 plan 10000
cp "$OUT/det-base/determinism.json" "$E/determinism-plan-base-triangle20.json"

echo "== attribution, 12 parents"
python3 "$D/attribution.py" "$OUT/attribution" "$BIN/race-se2" "$P"
cp "$OUT/attribution/attribution.json" "$E/attribution-12parents.json"

echo "== witness A/B, 12 parents"
python3 "$D/witnessab.py" "$OUT/witnessab" "$BIN/race-se2" "$P" 0.025:64:2
cp "$OUT/witnessab/witnessab.json" "$E/witnessab-12parents.json"

# ---- the two that must run with nothing else of this round's running --------
#
# The work-mode determinism is the gate this round's own code is responsible
# for - a work budget is a function of counters, not of the clock - so it is
# run last among the batteries rather than first, where a contended box would
# have made it slower without making it different. The suites are last of all
# because they saturate every core for twenty minutes.

echo "== determinism, work mode, race on"
python3 "$D/determinism.py" "$OUT/det-work-on" "$BIN/race-combo" $F 0,1,2 \
  work 40000000 'race=3:1:3'
cp "$OUT/det-work-on/determinism.json" "$E/determinism-work-raceon.json"

echo "== determinism, work mode, race off"
python3 "$D/determinism.py" "$OUT/det-work-off" "$BIN/race-combo" $F 0,1,2 \
  work 40000000 'race=0'
cp "$OUT/det-work-off/determinism.json" "$E/determinism-work-raceoff.json"

echo "== suites"
bash "$D/run-suites.sh"
echo "suites exit=$?"

echo COLLECT_OK
