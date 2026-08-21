#!/usr/bin/env bash
# Copy every driver's summary into `evidence/`, under the name `tables.py` reads.
#
# The raw per-run documents stay in the scratch tree: they are hundreds of
# megabytes and none of them is a claim. What lands here is every summary a
# table in the README is rendered from, plus the two calibration files
# themselves - those are inputs to a measurement and reproducing the round means
# reproducing them, so they are evidence and not scratch.
set -u
T="${1:-/var/lib/t3/tmp/robust}"
E="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/evidence"
mkdir -p "$E"

copy() {  # source, destination name
  if [ -f "$1" ]; then cp "$1" "$E/$2"; echo "  $2"; else echo "  MISSING $2"; fi
}

echo 'evidence:'
copy "$T/out/calpass/calpass.json"                    calpass.json
copy "$T/cal/live.json"                               cal-live.json
copy "$T/cal/probe.json"                              cal-probe.json
copy "$T/out/battery-loaded/planbattery.json"         battery-loaded.json
copy "$T/out/battery-quiet/planbattery.json"          battery-quiet.json
copy "$T/out/battery-head/planbattery.json"           battery-head.json
copy "$T/out/density-plan/density.json"               density-plan.json
copy "$T/out/density-plancal/density.json"            density-plancal.json
copy "$T/out/density-work/density.json"               density-work.json
copy "$T/out/equiv12/equiv.json"                      equiv12.json
copy "$T/out/battery-head/stress.log"                 stress-head.log
copy "$T/out/anytime/anytime.json"                    anytime.json
copy "$T/out/anytime30/anytime.json"                  anytime30.json
copy "$T/out/gates-ship/gates-ship.json"              gates-ship.json
copy "$T/out/gates-base/gates-base.json"              gates-base.json
copy "$T/out/equiv/equiv.json"                        equiv.json
copy "$T/out/det-work/determinism.json"               determinism-work.json
copy "$T/out/det-plan/determinism.json"               determinism-plan.json
copy "$T/out/det-callive/determinism.json"            determinism-callive.json
copy "$T/out/det-plan-loaded/determinism.json"        determinism-plan-loaded.json
copy "$T/out/det-callive-loaded/determinism.json"     determinism-callive-loaded.json
copy "$T/out/battery-loaded/stress.log"               stress-loaded.log
# One whole run document, kept because §3's 572 ns null is a claim about two
# fields of it and nothing else in `evidence/` carries `phases[].enteredSeconds`.
copy "$T/out/smoke/base.json"                         phase-zero-entry.json
# `run-suites.sh` writes its two logs straight into `evidence/`, so they are not
# copied here - they are listed instead, because a missing one is a suite that
# did not run and that has to be visible rather than assumed.
for suite in suite-jagua.log suite-combo.log; do
  if [ -f "$E/$suite" ]; then echo "  $suite (written in place)";
  else echo "  MISSING $suite"; fi
done
sha256sum "$T"/bin/* > "$E/binaries.txt"
echo "  binaries.txt"
