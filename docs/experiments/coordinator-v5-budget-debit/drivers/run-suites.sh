#!/usr/bin/env bash
# Both full test suites, each redirected to its own log with the exit status
# taken from `cargo test` itself and never from a pipeline.
#
# Two suites, not one, and that is the point Sol review 6 flagged as missing:
# `--features jagua-experimental` is the protocol's suite, but this change's
# only live code path - the self-metered debit - exists solely under
# `compression-schedule`, so a suite that omits it never compiles, let alone
# runs, the tests that matter. Both are run.
set -u
ROOT=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-1
LOGS=/var/lib/t3/tmp/wf6v1
cd "$ROOT" || exit 1

CARGO_TARGET_DIR=$LOGS/t-gate-fixed \
  cargo test --release --features jagua-experimental -j 4 \
  > "$LOGS/suite-jagua.log" 2>&1
echo "EXIT_JAGUA=$?"

CARGO_TARGET_DIR=$LOGS/t-sched-fixed \
  cargo test --release --features jagua-experimental,compression-schedule -j 4 \
  > "$LOGS/suite-jagua-sched.log" 2>&1
echo "EXIT_JAGUA_SCHED=$?"
