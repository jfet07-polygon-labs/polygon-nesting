#!/usr/bin/env bash
# Copies the round's evidence out of the scratch run directory into
# `docs/experiments/rotation-tax/evidence/`, under the names the README cites.
#
# Only summaries and the per-cell tables are committed; the raw per-run result
# documents stay in the scratch directory, because a single 30 s from-request
# document is megabytes of placements and there are 162 of them.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_08a442a7-1aa-1
OUT="${V4_OUT:-/var/lib/t3/rt/out}"
E="$W/docs/experiments/rotation-tax/evidence"
mkdir -p "$E"

copy() {  # copy SRC DEST
  if [ -f "$1" ]; then cp "$1" "$E/$2"; echo "ok   $2"; else echo "MISS $2 ($1)"; fi
}

copy "$OUT/taxprobe-census/taxprobe.json"     taxprobe-before.json
copy "$OUT/taxprobe-fixed/taxprobe.json"      taxprobe-after.json
copy "$OUT/decomp-fixed/decompose.json"       decompose-fromrequest.json
copy "$OUT/decomp-stage2/decompose.json"      decompose-buildstages.json
copy "$OUT/ablate-final/ablate.json"          ablate.json
copy "$OUT/ablate-all/ablate.json"            ablate-with-index.json
copy "$OUT/binab-10s/binab.json"              binab-10s.json
copy "$OUT/binab-off/binab.json"              binab-flagoff-10s.json
copy "$OUT/phase-10s/phaseshare.json"         phaseshare-10s.json
copy "$OUT/curves-summary.json"               curves-summary.json
for TAG in mixed61 shapes17 triangle20; do
  copy "$OUT/curve-$TAG/battery.json"         "curve-$TAG.json"
done
for G in base commit-gate commit-meas; do
  copy "$OUT/gates-$G/gates-$G.json"          "gates-$G.json"
done
copy "$OUT/gates-base/gates-base-gate.json"   gates-base.json
copy "$OUT/probequeue-ab/ablate.json"         probequeue-ab.json
copy "$OUT/reproduce/reproduce.json"          reproduce.json
copy "$OUT/determinism-crot/determinism-crot.json" determinism-crot.json
