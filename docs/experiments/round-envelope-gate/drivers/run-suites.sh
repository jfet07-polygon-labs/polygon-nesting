#!/usr/bin/env bash
# The protocol's four suites, with every exit status captured **directly**
# rather than through a pipe.
#
#   run-suites.sh
#
# `cargo test ... | tee log` reports `tee`'s exit status and not the test
# runner's, which is how a red suite gets written up as green. Every suite
# therefore redirects to a file and reads `$?` on the next line.
#
# Suite 3 is the example harness. `cargo test` never runs an example's
# `#[cfg(test)]` module on its own, so the benchmark example's tests are
# invisible to suites 1 and 2 and have to be asked for by name - and this
# round's whole production change is *in* that example, so suite 3 and suite 5
# are the two that actually cover it: 3 compiles the refusal half
# (`#[cfg(not(feature = "round-envelope-kernel"))]`) and 5 compiles the arming
# half.
#
# Suite 4 is this round's feature. This round adds no new cargo feature: it adds
# a second arming door to `round-envelope-kernel`, which the previous round
# introduced, so suite 4 is that feature's suite.
#
# It is stacked on `jagua-experimental` because of a PRE-EXISTING condition, not
# because of anything this round added: `examples/general_request_benchmark.rs`
# names `search::portfolio`, which is `#[cfg(feature = "jagua-experimental")]`,
# and it declares no `required-features`, so `cargo test` builds it for every
# invocation and ANY feature set without `jagua-experimental` fails to compile
# it.
set -u
W=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a
E="$W/docs/experiments/round-envelope-gate/evidence"
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
# not a search one. The protocol is to rerun once and report both, and
# `cargo test` stops the whole suite at the first failing target, so a trip also
# costs the other targets' coverage - hence the rerun is of the whole suite and
# not of the one test.
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

echo "== suite 3: the example harness, feature ABSENT (the refusal half)"
cargo test --release --features jagua-experimental --example general_request_benchmark \
  > "$E/suite-example.log" 2>&1
S3=$?
echo "suite-example exit=$S3"
grep -hE "^test result:" "$E/suite-example.log" | tail -6

echo "== suite 4: this round's feature"
cargo test --release --features jagua-experimental,round-envelope-kernel \
  > "$E/suite-kernel.log" 2>&1
S4=$?
echo "suite-kernel exit=$S4"
grep -hE "^test result:" "$E/suite-kernel.log" | tail -6

echo "== suite 5 (supplementary): the example harness, feature PRESENT (the arming half)"
cargo test --release --features jagua-experimental,round-envelope-kernel \
  --example general_request_benchmark > "$E/suite-example-rek.log" 2>&1
S5=$?
echo "suite-example-rek exit=$S5"
grep -hE "^test result:" "$E/suite-example-rek.log" | tail -6

echo "== suite 6 (supplementary): the measurement binary's own feature set"
cargo test --release --features "$COMBO,round-envelope-kernel" \
  --example general_request_benchmark > "$E/suite-example-combo-rek.log" 2>&1
S6=$?
echo "suite-example-combo-rek exit=$S6"
grep -hE "^test result:" "$E/suite-example-combo-rek.log" | tail -6

echo "EXITS jagua=$S1 combo=$S2 example=$S3 kernel=$S4 examplerek=$S5 examplecomborek=$S6"
[ "$S1" -eq 0 ] && [ "$S2" -eq 0 ] && [ "$S3" -eq 0 ] && [ "$S4" -eq 0 ] \
  && [ "$S5" -eq 0 ] && [ "$S6" -eq 0 ]
# NOTE for whoever runs this: do NOT pipe this script into `tee` or `tail`. The
# line above is the script's exit status and a pipe reports the LAST stage's,
# which is the same defect the per-suite redirects here exist to avoid. Redirect
# to a file and read `$?`.
