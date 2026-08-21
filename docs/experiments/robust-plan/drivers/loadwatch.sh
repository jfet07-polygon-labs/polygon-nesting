#!/usr/bin/env bash
# The box, sampled once a second for the whole measurement window.
#
# It exists because the box was not quiet: a second campaign was measuring on
# it for part of this round. A README that says "the box was busy" and a file
# that says how busy, when, are different documents.
set -u
OUT=/var/lib/t3/tmp/robust/out/boxload.tsv
mkdir -p "$(dirname "$OUT")"
printf 'epoch\tload1\tload5\tload15\tbenchmarks\n' >> "$OUT"
while true; do
  read -r L1 L5 L15 _ < /proc/loadavg
  N=$(pgrep -c -f general_request_benchmark || true)
  printf '%s\t%s\t%s\t%s\t%s\n' "$(date +%s)" "$L1" "$L5" "$L15" "${N:-0}" >> "$OUT"
  sleep 5
done
