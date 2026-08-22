#!/usr/bin/env bash
# Copies the round's evidence out of the scratch output tree into the
# experiment directory, exactly as run.
#
# The per-run documents are deliberately NOT copied: the probe alone writes 24
# of them and the matched gate 60, several megabytes each. What is kept is every
# reducer's output, which is what every number in the README is read from, plus
# the three suite logs and the gate documents.
set -eu
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-2
E="$W/docs/experiments/contact-block/evidence"
OUT="${CB_OUT:-/var/lib/t3/tmp/cblock/out}"
G=/var/lib/t3/tmp/cblock/gates
mkdir -p "$E"

# The retracted first pass, kept because the retraction is a finding.
cp "$OUT/probe12/blockprobe.json"            "$E/blockprobe-contract-gate-RETRACTED.json"
cp "$OUT/matched/matched.json"               "$E/matched-contract-gate-RETRACTED.json"
cp "$OUT/roundtrip/roundtrip.json"           "$E/roundtrip-contract-gate-RETRACTED.json"
cp "$OUT/why.json"                           "$E/why-contract-gate-RETRACTED.json"
cp "$OUT/compose/compose.json"               "$E/compose-contract-gate-RETRACTED.json"
cp "$OUT/replaycontrol/replaycontrol.json"   "$E/replaycontrol.json"

# The corrected pass: the composite gate.
cp "$OUT/probe12-fixed/blockprobe.json"      "$E/blockprobe.json"
cp "$OUT/matched-fixed/matched.json"         "$E/matched.json"
cp "$OUT/roundtrip-fixed/roundtrip.json"     "$E/roundtrip.json"
cp "$OUT/determinism-fixed/determinism.json" "$E/determinism.json"
cp "$OUT/compose-fixed/compose.json"         "$E/compose.json"
cp "$OUT/why-fixed.json"                     "$E/why.json"
cp "$OUT/verdict-cheap-fixed.json"           "$E/verdict-cheap.json"
cp "$OUT/verdict-saturating-fixed.json"      "$E/verdict-saturating.json"
cp "$OUT/slicetime-fixed.json"               "$E/slicetime.json"
cp "$OUT/flagoff.json"                       "$E/flagoff.json"
cp "$OUT/reproduce.json"                     "$E/reproduce.json"

# The knob sweeps that closed the operator's own parameter space.
cp "$OUT/sweep-trust/blockprobe.json"        "$E/sweep-trust-RETRACTED.json"
cp "$OUT/sweep-block/blockprobe.json"        "$E/sweep-block-RETRACTED.json"
cp "$OUT/sweep-rounds/blockprobe.json"       "$E/sweep-rounds-RETRACTED.json"
cp "$OUT/sweep-fixed/blockprobe.json"        "$E/sweep.json"

for label in base cb; do
  cp "$G/$label/gates-$label.json" "$E/gates-$label.json"
done

{
  echo "# The binaries every number in this round was measured on."
  for b in gate-base gate-cb meas meas-base; do
    sha256sum "/var/lib/t3/tmp/cblock/bin/$b"
  done
} > "$E/binaries.txt"

ls -la "$E"
