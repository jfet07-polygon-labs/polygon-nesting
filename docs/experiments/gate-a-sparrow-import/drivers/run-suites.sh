#!/usr/bin/env bash
# The protocol's three suites, with every exit status captured **directly**
# rather than through a pipe.
#
#   run-suites.sh
#
# `cargo test ... | tee log` reports `tee`'s exit status and not the test
# runner's, which is how a red suite gets written up as green. Every suite
# therefore redirects to a file and reads `$?` on the next line.
#
# Suite 3 is the example harness. The consolidation round proved `cargo test`
# never runs an example's `#[cfg(test)]` module on its own, so the benchmark
# example's tests are invisible to suites 1 and 2 and have to be asked for by
# name.
#
# Suite 4 is this round's addition: the shadow instrument compiles only under
# `import-gate-shadow`, which is in neither of the protocol's feature sets, so
# nothing above would have compiled it. It is a build-and-test of the feature,
# not a fourth protocol suite, and it is reported separately for that reason.
#
# It is stacked on `jagua-experimental` because of a PRE-EXISTING condition, not
# because of anything this round added: `examples/general_request_benchmark.rs`
# names `search::portfolio`, which is `#[cfg(feature = "jagua-experimental")]`,
# and it declares no `required-features`, so `cargo test` builds it for every
# invocation and ANY feature set without `jagua-experimental` fails to compile
# it. Verified on the base commit 1ca3315 with a feature set this round does not
# touch: `cargo check --features shadow-rescore --examples` produces the same
# three errors (E0432 + two E0433 on `search::portfolio`). Running suite 4 as
# `import-gate-shadow` alone would therefore have measured that pre-existing
# breakage and nothing about this round's feature.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f674db6b-1e0-1
E="$W/docs/experiments/gate-a-sparrow-import/evidence"
mkdir -p "$E"
cd "$W" || exit 3

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$W/target}"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

echo "== suite 1: jagua-experimental (the gate binary's feature set)"
cargo test --features jagua-experimental > "$E/suite-jagua.log" 2>&1
S1=$?
echo "suite-jagua exit=$S1"
grep -hE "^test result:" "$E/suite-jagua.log" | tail -6
# The campaign's known-flaky
# `free_material_multi_eviction_shrinks_retained_container_capacity` asserts a
# container capacity SHRANK after eviction, which is an allocator property and
# not a search one. It tripped on one run of this suite during this round
# (`suite-jagua-run2-flaky.log`, kept alongside). The protocol is to rerun once
# and report both, and `cargo test` stops the whole suite at the first failing
# target, so a trip also costs the other targets' coverage - hence the rerun is
# of the whole suite and not of the one test.
if [ "$S1" -ne 0 ] && grep -q "free_material_multi_eviction.* FAILED" "$E/suite-jagua.log"; then
  cp "$E/suite-jagua.log" "$E/suite-jagua-run2-flaky.log"
  echo "== suite 1 rerun (known flaky test tripped; both runs are reported)"
  cargo test --features jagua-experimental > "$E/suite-jagua.log" 2>&1
  S1=$?
  echo "suite-jagua-rerun exit=$S1"
  grep -hE "^test result:" "$E/suite-jagua.log" | tail -6
fi

echo "== suite 2: the protocol's full combo"
cargo test --features "$COMBO" > "$E/suite-combo.log" 2>&1
S2=$?
echo "suite-combo exit=$S2"
grep -hE "^test result:" "$E/suite-combo.log" | tail -6

echo "== suite 3: the example harness (cargo test never runs it on its own)"
cargo test --release --features jagua-experimental --example general_request_benchmark \
  > "$E/suite-example.log" 2>&1
S3=$?
echo "suite-example exit=$S3"
grep -hE "^test result:" "$E/suite-example.log" | tail -6

echo "== suite 4: this round's shadow feature (see the header for the stack)"
cargo test --features jagua-experimental,import-gate-shadow \
  > "$E/suite-shadow.log" 2>&1
S4=$?
echo "suite-shadow exit=$S4"
grep -hE "^test result:" "$E/suite-shadow.log" | tail -6

echo "EXITS jagua=$S1 combo=$S2 example=$S3 shadow=$S4"
[ "$S1" -eq 0 ] && [ "$S2" -eq 0 ] && [ "$S3" -eq 0 ] && [ "$S4" -eq 0 ]
# NOTE for whoever runs this: do NOT pipe this script into `tee` or `tail`. The
# line above is the script's exit status and a pipe reports the LAST stage's,
# which is the same defect the per-suite redirects here exist to avoid. Redirect
# to a file and read `$?`.
