#!/bin/bash
# The compositions the fixpoint at 155.3304 has not been asked yet.
#
#  * the k-deepest nudge handed to the *legalization* tiers. The nudge was
#    measured barren at 0/32 against modes 33 and 31 at 155.4196 - but that was
#    before tier H existed, and the whole finding of §5 is that the entry and
#    the repair are separate choices.
#  * mode 30's seed salt. Mode 33 is seed-invariant (18 arms, bit-identical);
#    whether mode 30 is has not been measured, and tier H is the tier that is
#    paying, so a factor of six there is worth knowing about either way.
#  * the legalization grid pushed a further order of magnitude deep.
set -u
cd /var/lib/t3/tmp/wf87/drivers
export BENCH_BIN=/var/lib/t3/tmp/wf87/target-l10/release/examples/general_request_benchmark
PIN=$1
RAW=$2
python3 knudge.py knudge-legal "$PIN" "$RAW" 1,2,3,5,7,10 0.05,0.1,0.2,0.3,0.5 30,31 4 0.05
python3 flatsweep.py m30seed "$PIN" "$RAW" 0.05,0.1,0.25 0.05 30 4 0,1,2,3,4,5
python3 flatsweep.py legal-deeper "$PIN" "$RAW" 1.5,2.0,2.5,3.0,4.0,5.0,7.0,10.0 0.05,2.0 30 4
python3 flatsweep.py legal-deeper31 "$PIN" "$RAW" 1.5,2.0,2.5,3.0,4.0,5.0,7.0,10.0 0.05,2.0 31 4
echo LASTPUSH_DONE
