#!/usr/bin/env bash
# The diagnosis probe that turns "Phi says the state is illegal" into "the two
# exact authorities say so too".
#
#     bash docs/experiments/overlap-ics/gate0-verification/drivers/band-probe.sh
#
# It uses one committed knob, `--band`, whose own doc comment in
# examples/overlap_ics_benchmark.rs says it exists "so a failing cell can be
# asked *which half* failed - the search, which could not get inside the band,
# or the publication, which could not legalize once inside it. A widened band is
# never a verdict; every gate in cells.py runs at the derived one."
#
# That is exactly this probe's purpose. At the shipped 4 um band all three fatal
# cells make **zero** publication attempts, so the round kernel and the contract
# validator never see their states and the only witness against those states is
# Phi itself. Widening the band to 0.2 mm lets the attempt happen and puts the
# same states in front of `Exclusive` at the request radius and the untouched
# `validate_placements_against_contract`.
#
# The other two gates of `publish::attempt` are NOT touched, and they are why
# only the triangle canary produces attempts: S1 sits 7.53 um and C175 sits
# 1.18 mm outside their locked strips, and `proxy_depth <= T` is not a band.

set -u

ROOT="${ICS_ROOT:-/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_7f77514b-f9a-2}"
BIN="${ICS_BIN:-$ROOT/target/release/examples/overlap_ics_benchmark}"
OUT="${ICS_VERIFY_OUT:-/var/lib/t3/tmp/overlapics-v2}/probe"
BAND="${ICS_PROBE_BAND:-0.2}"
mkdir -p "$OUT"

MIXED="$ROOT/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json"
TRI="$ROOT/tests/fixtures/triangle-20/2000x2700-compact/request.json"
POSES="$ROOT/docs/experiments/gate-a-sparrow-import/fixture/sparrow-10s-x86-poses.json"

"$BIN" --cell=triangle --request="$TRI" --edge=5 --pair=5 \
  --target=70.742 --budget=200000 --seed=0 --checkpointevery=1 --band="$BAND" \
  > "$OUT/triangle20-band.json" 2> "$OUT/triangle20-band.err"
echo "T20_BAND_EXIT=$?"

"$BIN" --cell=s1 --request="$MIXED" --edge=5 --pair=5 --poses="$POSES" \
  --target=150.16547 --budget=200000 --seed=0 --perturbmm=0.5 --perturbdeg=2.0 \
  --checkpointevery=1 --band="$BAND" \
  > "$OUT/s1-band.json" 2> "$OUT/s1-band.err"
echo "S1_BAND_EXIT=$?"

"$BIN" --cell=c175 --request="$MIXED" --edge=5 --pair=5 \
  --budget=200000 --seed=0 --checkpointevery=1 --band="$BAND" \
  > "$OUT/c175-seed0-band.json" 2> "$OUT/c175-seed0-band.err"
echo "C175_BAND_EXIT=$?"
