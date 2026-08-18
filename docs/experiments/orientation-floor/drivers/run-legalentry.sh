#!/bin/bash
# The entry->legalization composition. Mode 33 refuses a *partial* repair - it
# reports componentsRepaired=1, componentsRefused=1 and then rejects the whole
# arm - so on a deep flatten it throws away work it has already done. The global
# legalization tiers (modes 30 and 31) do not enumerate insertion orders at all;
# they push the whole layout under a displacement cap, so they fail differently
# and, on this state, they fail later. The record-line cascade only ever handed
# a flattened fixture to modes 32 and 33, so this composition is untried.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export BENCH_BIN=/var/lib/t3/tmp/wf87/target-l10/release/examples/general_request_benchmark
PIN=$1
RAW=$2
DEEP=0.05,0.08,0.1,0.12,0.15,0.2,0.25,0.3,0.4,0.5,0.7,1.0,1.5,2.0
python3 flatsweep.py legal-m30 "$PIN" "$RAW" "$DEEP" 0.05,2.0 30 4
python3 flatsweep.py legal-m31 "$PIN" "$RAW" "$DEEP" 0.05,2.0 31 4
python3 flatsweep.py legal-m27 "$PIN" "$RAW" "$DEEP" 0.05,2.0 27 4
echo LEGALENTRY_DONE
