#!/usr/bin/env bash
# The whole corrected evidence chain, from the committed tree, in one command.
#
#   run-all.sh
#
# The retracted first pass is NOT re-run here: it required an operator that
# validated the wrong gate, and that operator no longer exists. Its documents are
# kept under `evidence/*-RETRACTED.json` exactly as they were produced, and §2 of
# the README says which binary produced them.
set -eu
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_d252e868-756-2
D="$W/docs/experiments/contact-block/drivers"
OUT="${CB_OUT:-/var/lib/t3/tmp/cblock/out}"
M=/var/lib/t3/tmp/cblock/bin/meas
P="$D/parents12.json"
CHEAP='trust=0.5;iters=256;block=5;rounds=64;seeds=3;band=2'
SAT='trust=1.0;iters=256;block=5;rounds=4096;seeds=8;band=4'
CHEAP_LABEL="block:${CHEAP//;/,}"
SAT_LABEL="block:${SAT//;/,}"
cd "$W"

echo "== gates, both binaries"
python3 "$D/gates.py" base /var/lib/t3/tmp/cblock/bin/gate-base > "$OUT/gates-base.log"
python3 "$D/gates.py" cb   /var/lib/t3/tmp/cblock/bin/gate-cb   > "$OUT/gates-cb.log"
python3 "$D/flagoff.py" /var/lib/t3/tmp/cblock/gates/base \
    /var/lib/t3/tmp/cblock/gates/cb base cb "$OUT/flagoff.json"

echo "== the twelve-parent probe and its decomposition"
python3 "$D/blockprobe.py" "$OUT/probe12-fixed" "$M" "$P" "$CHEAP,$SAT" \
    > "$OUT/probe12-fixed.log"
python3 "$D/why.py" "$OUT/probe12-fixed" "$OUT/probe12-fixed/blockprobe.json" \
    "$OUT/why-fixed.json" > /dev/null

echo "== the matched-arm gate"
python3 "$D/matched.py" "$OUT/matched-fixed" "$M" "$P" "$CHEAP,$SAT" \
    500000,1500000,3341379 > "$OUT/matched-fixed.log"
python3 "$D/verdict.py" "$OUT/matched-fixed/matched.json" "$CHEAP_LABEL" \
    m34:3341379 "$OUT/verdict-cheap-fixed.json" > "$OUT/verdict-cheap.log"
python3 "$D/verdict.py" "$OUT/matched-fixed/matched.json" "$SAT_LABEL" \
    m34:3341379 "$OUT/verdict-saturating-fixed.json" > "$OUT/verdict-sat.log"
python3 "$D/slicetime.py" "$OUT/matched-fixed" "$OUT/matched-fixed/matched.json" \
    "$CHEAP_LABEL" 3341379 "$OUT/slicetime-fixed.json" > "$OUT/slicetime.log"

echo "== soundness, determinism, composition"
python3 "$D/roundtrip.py" "$OUT/roundtrip-fixed" "$M" "$P" "$CHEAP" \
    > "$OUT/roundtrip-fixed.log"
python3 "$D/replaycontrol.py" "$OUT/replaycontrol" "$M" "$P" \
    > "$OUT/replaycontrol.log"
python3 "$D/determinism.py" "$OUT/determinism-fixed" "$M" "$P" "$CHEAP" \
    > "$OUT/determinism-fixed.log"
python3 "$D/compose.py" "$OUT/compose-fixed" "$M" "$P" "$OUT/roundtrip-fixed" \
    3341379 > "$OUT/compose-fixed.log"

echo "== the knob sweep"
python3 "$D/blockprobe.py" "$OUT/sweep-fixed" "$M" "$D/calib3.json" \
  "trust=0.25;iters=256;block=5;rounds=64;seeds=3;band=2,trust=1.0;iters=256;block=5;rounds=64;seeds=3;band=2,trust=2.0;iters=256;block=5;rounds=64;seeds=3;band=2,trust=1.0;iters=256;block=3;rounds=64;seeds=3;band=2,trust=1.0;iters=256;block=14;rounds=64;seeds=3;band=2,trust=1.0;iters=2000;block=5;rounds=64;seeds=3;band=2" \
    > "$OUT/sweep-fixed.log"

echo RUN_ALL_DONE
