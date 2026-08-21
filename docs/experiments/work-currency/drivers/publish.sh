#!/usr/bin/env bash
# Copy the evidence a run of `collect.sh` produced into the repository.
#
#   bash publish.sh [OUTDIR] [BINDIR]
#
# Only the summary documents are committed, not the per-run JSON: one
# `plan=10000` mixed-61 run is ~200 kB and this round made several hundred of
# them. Every summary carries the per-cell rows the tables are built from, and
# `drivers/` reproduces the rest.
set -eu
OUT="${1:-/tmp/wc-out}"
BINDIR="${2:-/tmp/wc-bin}"
E="$(cd "$(dirname "$0")/.." && pwd)/evidence"
mkdir -p "$E"

for f in rates.json profile.json equivalence.json gates-base.json \
         gates-ship.json determinism-work-cur2.json \
         determinism-plan-cur2.json racebattery-10000.json \
         planbattery-10s.json countertax.json binequiv-cur2.json; do
  [ -f "$OUT/$f" ] && cp "$OUT/$f" "$E/$f"
done
for f in suite-jagua.log suite-combo.log; do
  [ -f "$OUT/$f" ] && cp "$OUT/$f" "$E/$f"
done

# The binaries' identities. Every claim in the README is attributed to one of
# these hashes, including the two builds of this tree that §3.3 joins.
: > "$E/binaries.txt"
for b in base-gate base-combo battery-combo ship-gate ship-combo; do
  [ -f "$BINDIR/$b" ] && sha256sum "$BINDIR/$b" >> "$E/binaries.txt"
done
echo "published into $E"
ls -la "$E"
