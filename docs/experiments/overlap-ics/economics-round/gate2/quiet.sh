#!/usr/bin/env bash
# **The quiet box, as a gate rather than a habit.**
#
# Every timed battery in this wave is preceded by this script. It blocks until
# `/proc/loadavg`'s one-minute figure is below 1.00 and then prints the reading
# it saw, so the document can say what the box was doing rather than that
# somebody waited.
#
#   ./quiet.sh            # wait, then print
#   ./quiet.sh 300        # give up after 300 s and exit 1
#
# Exit is the verdict: 0 when the box went quiet, 1 when it did not inside the
# deadline. A battery launched after a `1` is a battery whose seconds are about
# a machine doing something else, and the reject rule, the p95 clause and the
# session-drift record are all statements about seconds.
set -u
deadline="${1:-600}"
started=$(date +%s)
while true; do
    read -r one _ < /proc/loadavg
    if awk -v v="$one" 'BEGIN { exit !(v < 1.0) }'; then
        echo "QUIET_BOX: true loadavg1=$one waited=$(( $(date +%s) - started ))s"
        exit 0
    fi
    if [ $(( $(date +%s) - started )) -ge "$deadline" ]; then
        echo "QUIET_BOX: false loadavg1=$one deadline=${deadline}s"
        exit 1
    fi
    sleep 5
done
