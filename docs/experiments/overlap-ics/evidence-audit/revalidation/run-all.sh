#!/usr/bin/env bash
# The independent re-validation, end to end.
#
#     bash run-all.sh                 # committed-evidence stages only
#     bash run-all.sh <binary>        # plus the two stages that need a binary
#
# Do NOT pipe this into `tee` or `tail`: you would read the pipe's status
# instead of the script's. Every exit status below is captured on its own line
# immediately after the command, and nothing here uses a pipeline to decide
# anything.
#
# `rv_frame.py` exits **1 on purpose** — its predicate is "no committed number
# moves", and three do. It is listed under EXPECTED_RED and does not count as a
# failure.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${RV_EVIDENCE:-$HERE/evidence}"
BIN="${1:-}"
mkdir -p "$OUT"
FAILURES=0

# `$1` is the transcript file, `$2` the expected status, `$3` the label; the
# rest is the command. The transcript redirect is applied to the command alone,
# so the stage line itself still reaches stdout.
stage() {
  local transcript="$1"; local expect="$2"; local label="$3"; shift 3
  "$@" > "${transcript}" 2>&1
  local status=$?
  local note=''
  if [ "${expect}" -ne 0 ]; then note=" (${expect} is the pass here)"; fi
  echo "[rv] ${label} EXIT=${status}${note}"
  if [ "${status}" -ne "${expect}" ]; then FAILURES=$((FAILURES + 1)); fi
}

stage "$OUT/reduction.txt" 0 \
  "reduction: committed wall.json vs the raw cell documents" \
  python3 "$HERE/rv_reduction.py" "$OUT/reduction.json"
stage "$OUT/frame.txt" 1 \
  "frame: the §0.1 clause decided on the committed round" \
  python3 "$HERE/rv_frame.py" "$OUT/frame.json"
stage "$OUT/late-publications.txt" 0 \
  "late: the post-deadline publications, named" \
  python3 "$HERE/rv_late.py" "$OUT/late-publications.json"
stage "$OUT/deadline.txt" 0 \
  "deadline: overrun in the engine's own frame" \
  python3 "$HERE/rv_deadline.py" "$OUT/deadline.json"
stage "$OUT/publications.txt" 0 \
  "publications: 18,665 identities over 1,701 publications" \
  python3 "$HERE/rv_publications.py" "$OUT/publications.json"
stage "$OUT/authorities.txt" 0 \
  "authorities: which authority refused, per checkpoint" \
  python3 "$HERE/rv_authorities.py" "$OUT/authorities.json"
stage "$OUT/bites.txt" 0 \
  "bites: the README's per-bite claims, independently reduced" \
  python3 "$HERE/rv_bites.py" "$OUT/bites.json"
stage "$OUT/crossround.txt" 0 \
  "crossround: round 1 against the rerun, and the shared prefix" \
  python3 "$HERE/rv_crossround.py" "$OUT/crossround.json"
stage "$OUT/control.txt" 0 \
  "control: the AB/BA arms" \
  python3 "$HERE/rv_control.py" "$OUT/control.json"

if [ -n "$BIN" ]; then
  stage "$OUT/poses.txt" 0 \
    "poses: S0's pins and 18 recorded arm-B layouts" \
    python3 "$HERE/rv_poses.py" "$BIN" "$OUT/poses.json"
  stage "$OUT/replay.txt" 0 \
    "replay: the nine fixed-work replays, bit for bit" \
    python3 "$HERE/rv_replay.py" "$BIN" "$OUT/replay.json"
else
  echo "[rv] poses/replay SKIPPED (no binary argument)"
fi

echo "[rv] FAILURES=${FAILURES}"
exit "$FAILURES"
