#!/usr/bin/env bash
set -euo pipefail
cd /tmp/topo-work-wf48
REQ=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
PARENT=docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-159.092/pinned-parent-159.092.json
mkdir -p /tmp/warmstart
/tmp/gate-bin-fixed-sched "$REQ" 1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 1 0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0 0 '' '' "$PARENT" 0.002 'work=40000000,cells=11:15:21:27,v3=0' > /tmp/warmstart/fixed-warm.json 2> /tmp/warmstart/fixed-warm.err
