#!/usr/bin/env bash
# The protocol's four suites, plus this round's own, with every exit status
# captured **directly** rather than through a pipe.
#
#   run-suites.sh
#
# `cargo test ... | tee log` reports `tee`'s exit status and not the test
# runner's, which is how a red suite gets written up as green. Every suite
# therefore redirects to a file and reads `$?` on the next line.
#
# Suite 4 is the protocol's `jagua-experimental,round-envelope-kernel` plus this
# round's new feature, which is separate: `skip-pile-dump`. Suite 5 is that
# feature alone on top of `jagua-experimental`, so the hook is compiled without
# the kernel beside it and the two cannot be entangled by accident; suite 6 is
# the measurement binary's own feature set, which is the only set the evidence
# was taken on.
#
# Suite 3 is the example harness. `cargo test` never runs an example's
# `#[cfg(test)]` module on its own, so the benchmark example's tests are
# invisible to suites 1 and 2 and have to be asked for by name.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_0c9338d3-644-1
E="$W/docs/experiments/skip-pile-diagnostic/evidence"
mkdir -p "$E"
cd "$W" || exit 3

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$W/target}"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

echo "== suite 1: jagua-experimental (the gate binary's feature set)"
cargo test --release --features jagua-experimental > "$E/suite-jagua.log" 2>&1
S1=$?
echo "suite-jagua exit=$S1"
grep -hE "^test result:" "$E/suite-jagua.log" | tail -6
# The campaign's known-flaky
# `free_material_multi_eviction_shrinks_retained_container_capacity` asserts a
# container capacity SHRANK after eviction, which is an allocator property and
# not a search one. The protocol is to rerun once and report both.
if [ "$S1" -ne 0 ] && grep -q "free_material_multi_eviction.* FAILED" "$E/suite-jagua.log"; then
  cp "$E/suite-jagua.log" "$E/suite-jagua-run1-flaky.log"
  echo "== suite 1 rerun (known flaky test tripped; both runs are reported)"
  cargo test --release --features jagua-experimental > "$E/suite-jagua.log" 2>&1
  S1=$?
  echo "suite-jagua-rerun exit=$S1"
  grep -hE "^test result:" "$E/suite-jagua.log" | tail -6
fi

echo "== suite 2: the protocol's full combo"
cargo test --release --features "$COMBO" > "$E/suite-combo.log" 2>&1
S2=$?
echo "suite-combo exit=$S2"
grep -hE "^test result:" "$E/suite-combo.log" | tail -6
if [ "$S2" -ne 0 ] && grep -q "free_material_multi_eviction.* FAILED" "$E/suite-combo.log"; then
  cp "$E/suite-combo.log" "$E/suite-combo-run1-flaky.log"
  echo "== suite 2 rerun (known flaky test tripped; both runs are reported)"
  cargo test --release --features "$COMBO" > "$E/suite-combo.log" 2>&1
  S2=$?
  echo "suite-combo-rerun exit=$S2"
  grep -hE "^test result:" "$E/suite-combo.log" | tail -6
fi

echo "== suite 3: the example harness"
cargo test --release --features jagua-experimental --example general_request_benchmark \
  > "$E/suite-example.log" 2>&1
S3=$?
echo "suite-example exit=$S3"
grep -hE "^test result:" "$E/suite-example.log" | tail -6

echo "== suite 4: the protocol's kernel suite plus this round's new feature"
cargo test --release --features jagua-experimental,round-envelope-kernel,skip-pile-dump \
  > "$E/suite-kernel-dump.log" 2>&1
S4=$?
echo "suite-kernel-dump exit=$S4"
grep -hE "^test result:" "$E/suite-kernel-dump.log" | tail -6
if [ "$S4" -ne 0 ] && grep -q "free_material_multi_eviction.* FAILED" "$E/suite-kernel-dump.log"; then
  cp "$E/suite-kernel-dump.log" "$E/suite-kernel-dump-run1-flaky.log"
  echo "== suite 4 rerun (known flaky test tripped; both runs are reported)"
  cargo test --release --features jagua-experimental,round-envelope-kernel,skip-pile-dump \
    > "$E/suite-kernel-dump.log" 2>&1
  S4=$?
  echo "suite-kernel-dump-rerun exit=$S4"
  grep -hE "^test result:" "$E/suite-kernel-dump.log" | tail -6
fi

echo "== suite 5: this round's feature alone, without the kernel beside it"
cargo test --release --features jagua-experimental,skip-pile-dump \
  > "$E/suite-dump.log" 2>&1
S5=$?
echo "suite-dump exit=$S5"
grep -hE "^test result:" "$E/suite-dump.log" | tail -6
if [ "$S5" -ne 0 ] && grep -q "free_material_multi_eviction.* FAILED" "$E/suite-dump.log"; then
  cp "$E/suite-dump.log" "$E/suite-dump-run1-flaky.log"
  echo "== suite 5 rerun (known flaky test tripped; both runs are reported)"
  cargo test --release --features jagua-experimental,skip-pile-dump \
    > "$E/suite-dump.log" 2>&1
  S5=$?
  echo "suite-dump-rerun exit=$S5"
  grep -hE "^test result:" "$E/suite-dump.log" | tail -6
fi

echo "== suite 6: the measurement binary's own feature set"
cargo test --release --features "$COMBO,round-envelope-kernel,skip-pile-dump" \
  > "$E/suite-meas.log" 2>&1
S6=$?
echo "suite-meas exit=$S6"
grep -hE "^test result:" "$E/suite-meas.log" | tail -6
if [ "$S6" -ne 0 ] && grep -q "free_material_multi_eviction.* FAILED" "$E/suite-meas.log"; then
  cp "$E/suite-meas.log" "$E/suite-meas-run1-flaky.log"
  echo "== suite 6 rerun (known flaky test tripped; both runs are reported)"
  cargo test --release --features "$COMBO,round-envelope-kernel,skip-pile-dump" \
    > "$E/suite-meas.log" 2>&1
  S6=$?
  echo "suite-meas-rerun exit=$S6"
  grep -hE "^test result:" "$E/suite-meas.log" | tail -6
fi

echo "== suite 7 (supplementary): the scorer's own feature set builds and tests"
cargo test --release --features jagua-experimental,round-envelope-kernel,import-gate-shadow \
  --example skip_pile_score > "$E/suite-scorer.log" 2>&1
S7=$?
echo "suite-scorer exit=$S7"
grep -hE "^test result:" "$E/suite-scorer.log" | tail -6

echo "EXITS jagua=$S1 combo=$S2 example=$S3 kerneldump=$S4 dump=$S5 meas=$S6 scorer=$S7"
[ "$S1" -eq 0 ] && [ "$S2" -eq 0 ] && [ "$S3" -eq 0 ] && [ "$S4" -eq 0 ] \
  && [ "$S5" -eq 0 ] && [ "$S6" -eq 0 ] && [ "$S7" -eq 0 ]
# NOTE for whoever runs this: do NOT pipe this script into `tee` or `tail`. The
# line above is the script's exit status and a pipe reports the LAST stage's,
# which is the same defect the per-suite redirects here exist to avoid. Redirect
# to a file and read `$?`.
