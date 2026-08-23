#!/usr/bin/env bash
# Three 10.000 s wall cells, written where `checkpoint-frame.py` can read the
# per-publication `wallSeconds` the committed `wall.json` reduction dropped.
#
#     bash run-cells.sh <root> <out-dir> [seeds...]
#
# Exit status is captured per cell on its own line and never through a pipe.
set -u
ROOT="$1"
OUT="$2"
shift 2
SEEDS=("$@")
if [ "${#SEEDS[@]}" -eq 0 ]; then SEEDS=(0 2 3); fi
BIN="$ROOT/target/release/examples/overlap_ics_benchmark"
REQ="$ROOT/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json"
mkdir -p "$OUT"
status=0
for seed in "${SEEDS[@]}"; do
  "$BIN" --cell=cutclose --request="$REQ" --edge=5 --pair=5 \
         --mode=wall --wall=10.0 --workers=8 --seed="$seed" \
         > "$OUT/wall-10s-seed$seed.json"
  cell_status=$?
  echo "seed=$seed exit=$cell_status"
  if [ "$cell_status" -ne 0 ]; then status=$cell_status; fi
done
exit "$status"
