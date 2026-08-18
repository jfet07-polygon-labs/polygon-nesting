#!/usr/bin/env bash
# Blocks until LOG contains at least COUNT lines matching PATTERN.
set -u
log="$1"
pattern="$2"
want="$3"
while true; do
  have=$(grep -cE "$pattern" "$log" 2>/dev/null || true)
  have=${have:-0}
  if [ "${have:-0}" -ge "$want" ]; then
    echo "READY $have/$want"
    exit 0
  fi
  sleep 10
done
