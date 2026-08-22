#!/usr/bin/env bash
# Blocks until no `cargo test` is running, so the final suite pass is not run
# against a target directory another pass is still writing to.
set -u
while pgrep -f 'cargo test' > /dev/null 2>&1; do
  sleep 10
done
echo SUITES_IDLE
