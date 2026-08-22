#!/usr/bin/env bash
# The three suites the protocol requires, each exit code captured directly from
# the command rather than through a pipe, because a pipe reports the exit of
# the last stage and a suite that failed behind `| tee` reads as a pass.
#
# The third is the one the consolidation round proved is missed by the other
# two: `cargo test` does not build or run an example's own `#[test]` functions
# unless the example is named, and this repository's benchmark harness carries
# some.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f674db6b-1e0-2
OUT=/var/lib/t3/tmp/m26band/suites
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/lib/t3/tmp/m26band/tgt}"
mkdir -p "$OUT"
cd "$W"

COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator

run() {
  local name="$1"; shift
  echo "== $name: $*"
  "$@" > "$OUT/$name.log" 2>&1
  local code=$?
  echo "$name EXIT=$code"
  echo "$code" > "$OUT/$name.exit"
  tail -3 "$OUT/$name.log"
}

run suite-jagua cargo test --release --features jagua-experimental
run suite-combo cargo test --release --features "$COMBO"
run suite-example cargo test --release --features jagua-experimental \
  --example general_request_benchmark
