#!/usr/bin/env bash
# Blocks until LOG contains COUNT lines matching ': engine='. Used only to let
# the agent driving these batteries wait without running anything else on the
# box: every timing claim here is paired and interleaved, and a second process
# during a battery would break that.
set -u
log="$1"
want="$2"
while true; do
  have=$(grep -c ': engine=' "$log" 2>/dev/null || echo 0)
  if [ "$have" -ge "$want" ]; then
    echo "READY $have/$want"
    exit 0
  fi
  sleep 10
done
