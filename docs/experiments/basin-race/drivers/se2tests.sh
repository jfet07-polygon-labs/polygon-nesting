#!/usr/bin/env bash
# The one regression test neither protocol suite reaches.
#
# `a_witness_maps_back_onto_the_parent_state_slot_for_slot` is gated on
# `se2-rigidity-certificate`, which the `jagua-experimental` suite does not
# carry and the protocol's full combo does not either - the certificate has its
# own feature and §6 explains why. So it is run here, in the build that has it,
# alongside the other seven so the whole set has one log.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_30e47560-32c-2
E="$W/docs/experiments/basin-race/evidence"
cd "$W" || exit 3
COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

CARGO_TARGET_DIR=/var/lib/t3/tmp/basinrace/target-se2 \
  cargo test --release --lib --features "$COMBO,se2-rigidity-certificate" -- \
  trigger_b_disarms a_committed_move_is_charged a_witness_maps_back \
  the_disarm_bit_cannot the_race_judges the_halving_shrinks \
  the_basin_race_is_off a_retired_basin \
  > "$E/tests-se2-certificate.log" 2>&1
echo "se2 regression tests exit=$?"
grep -E "^test |^test result:" "$E/tests-se2-certificate.log" | tail -12
