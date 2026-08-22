#!/usr/bin/env bash
# The round-boundary suites, with every exit status captured **directly**
# rather than through a pipe.
#
#   bash docs/experiments/overlap-ics/drivers/run-suites.sh
#
# `cargo test ... | tee log` reports `tee`'s exit status and not the test
# runner's, which is how a red suite gets written up as green. Every suite here
# redirects to a file and reads `$?` on the next line.
#
# Suites 1-4 are the round-envelope-gate protocol's, unchanged. Suite 4 is
# **this round's feature**: `overlap-ics`, stacked on `jagua-experimental` for a
# PRE-EXISTING reason and not because of anything this round added -
# `examples/general_request_benchmark.rs` names `search::portfolio`, which is
# `#[cfg(feature = "jagua-experimental")]`, and it declares no
# `required-features`, so `cargo test` builds it for every invocation and any
# feature set without `jagua-experimental` fails to compile it.
#
# Suite 5 is `overlap-ics` **alone**, which is the combination the FAST tier
# runs and the one the example's own `required-features` name. It is the suite
# that proves the feature does not need `jagua-experimental` to be exercised -
# the Chinese wall in the dependency graph, checked as a build and not only as a
# `cargo tree` grep.
#
# It is scoped to `--lib --tests` for the same pre-existing reason suite 4 is
# stacked: an unscoped `cargo test --features overlap-ics` tries to build
# `general_request_benchmark`, which names `search::portfolio` and has no
# `required-features`, and fails with `unresolved import
# polygon_nesting_core::search::portfolio` before running a single test. That is
# HEAD's condition, not this round's - `overlap_ics_benchmark` declares
# `required-features = ["overlap-ics"]` precisely so it never does that to
# anyone else - and `--lib --tests` is the scope that asks the question suite 5
# is for.
set -u
W="${ICS_ROOT:-/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_7f77514b-f9a-1}"
E="$W/docs/experiments/overlap-ics/evidence"
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

echo "== suite 4: this round's feature, stacked"
cargo test --release --features jagua-experimental,overlap-ics \
  > "$E/suite-overlap-ics-stacked.log" 2>&1
S4=$?
echo "suite-overlap-ics-stacked exit=$S4"
grep -hE "^test result:" "$E/suite-overlap-ics-stacked.log" | tail -6

echo "== suite 5: this round's feature ALONE (the Chinese wall as a build)"
cargo test --release --features overlap-ics --lib --tests > "$E/suite-overlap-ics.log" 2>&1
S5=$?
echo "suite-overlap-ics exit=$S5"
grep -hE "^test result:" "$E/suite-overlap-ics.log" | tail -6
if [ "$S5" -ne 0 ] && grep -q "free_material_multi_eviction.* FAILED" "$E/suite-overlap-ics.log"; then
  cp "$E/suite-overlap-ics.log" "$E/suite-overlap-ics-run1-flaky.log"
  echo "== suite 5 rerun (known flaky test tripped; both runs are reported)"
  cargo test --release --features overlap-ics --lib --tests > "$E/suite-overlap-ics.log" 2>&1
  S5=$?
  echo "suite-overlap-ics-rerun exit=$S5"
  grep -hE "^test result:" "$E/suite-overlap-ics.log" | tail -6
fi

echo "EXITS jagua=$S1 combo=$S2 example=$S3 icsstacked=$S4 icsalone=$S5"
[ "$S1" -eq 0 ] && [ "$S2" -eq 0 ] && [ "$S3" -eq 0 ] && [ "$S4" -eq 0 ] \
  && [ "$S5" -eq 0 ]
# NOTE for whoever runs this: do NOT pipe this script into `tee` or `tail`. The
# line above is the script's exit status and a pipe reports the LAST stage's,
# which is the same defect the per-suite redirects here exist to avoid.
