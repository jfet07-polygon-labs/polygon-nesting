#!/usr/bin/env bash
# Every battery in this round, in the order they have to run.
#
# The order is not alphabetical and is not negotiable:
#
#   1. the wall-sensitive batteries first and alone - the plan battery, the
#      race battery and the counter tax all read the clock as their subject or
#      as their instrument;
#   2. then the work-capped ones - the gates, the equivalence battery, the
#      rate/profile fit, the determinism gate - which are functions of counters
#      and are not moved by a busy box;
#   3. then the suites, which saturate every core and would have made
#      everything before them a measurement of the box.
#
#   bash collect.sh BINDIR OUTDIR
#
# BINDIR must contain base-gate, base-combo, ship-gate, ship-combo.
set -u
BINDIR="${1:-/tmp/wc-bin}"
OUT="${2:-/tmp/wc-out}"
D="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$OUT"
export CUR2_BIN="$BINDIR/ship-combo"
export CUR2_OUT="$OUT"

set -x
# ---- 1. wall-sensitive ------------------------------------------------------
python3 "$D/planbattery.py"  "$OUT/planbattery-10s.json" "$BINDIR/ship-combo" \
        mixed-61 0,1,2 10000 3
python3 "$D/racebattery.py"  "$OUT" "$BINDIR/ship-combo" \
        mixed-61,shapes-17,triangle-20 0,1,2 10000 3:1:3
python3 "$D/countertax.py"   "$OUT/countertax.json" "$BINDIR/ship-combo" \
        mixed-61 10000 0,1,2 2

# ---- 2. work-capped ---------------------------------------------------------
python3 "$D/gates.py" base "$BINDIR/base-gate" "$OUT/gates/base" \
        > "$OUT/gates-base.json"
python3 "$D/gates.py" ship "$BINDIR/ship-gate" "$OUT/gates/ship" \
        > "$OUT/gates-ship.json"
python3 "$D/equiv.py"        "$OUT/equivalence.json" \
        "$BINDIR/base-combo" "$BINDIR/ship-combo"
python3 "$D/rates.py"        "$OUT/rates.json" "$BINDIR/ship-combo"
python3 "$D/fitprofile.py"   "$OUT/rates.json" "$OUT/profile.json"
python3 "$D/determinism.py"  "$OUT/determinism-work-cur2.json" \
        "$BINDIR/ship-combo" mixed-61,shapes-17,triangle-20 0,1,2 \
        work 40000000 cur2=1
python3 "$D/determinism.py"  "$OUT/determinism-plan-cur2.json" \
        "$BINDIR/ship-combo" mixed-61,shapes-17,triangle-20 0,1,2 \
        plan 10000 cur2=1
# The §3.3 join: only meaningful when BINDIR carries a second build of this
# tree. Skipped silently when it does not.
[ -f "$BINDIR/battery-combo" ] && python3 "$D/binequiv.py" \
        "$OUT/binequiv-cur2.json" "$BINDIR/battery-combo" \
        "$BINDIR/ship-combo" cur2=1

# ---- 3. the suites ----------------------------------------------------------
bash "$D/run-suites.sh" "$OUT"
