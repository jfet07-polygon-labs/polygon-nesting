#!/bin/bash
# The two instruments the frontier stack asks for and the record-line cascade
# never ran: a flatten grid deep enough to move more than the top three pieces
# (its grid stopped at 0.01 mm, which on this state moves ranks 1-3), and the
# k-deepest nudge sized to the seven pieces that sit inside 0.040 mm.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export BENCH_BIN=/var/lib/t3/tmp/wf87/target-l10/release/examples/general_request_benchmark
PIN=$1
RAW=$2
python3 flatsweep.py deepflat "$PIN" "$RAW" 0.015,0.02,0.025,0.03,0.04,0.045,0.06,0.08,0.1,0.15,0.2 0.05,2.0 33 2
python3 knudge.py knudge "$PIN" "$RAW" 2,3,5,7 0.02,0.05,0.1,0.3 33,31 2
echo FRONTIER_INSTRUMENTS_DONE
