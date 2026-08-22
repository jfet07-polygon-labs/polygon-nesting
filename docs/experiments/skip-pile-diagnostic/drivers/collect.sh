#!/usr/bin/env bash
# Regenerates every evidence file in this directory from the committed tree.
#
#   collect.sh [stage]
#
# Stages, in the campaign's provenance order - build from the committed tree,
# gate, then measure:
#
#   build   the three binaries
#   gates   the four pinned regression gates, on the FEATURE-ABSENT binary and
#           on the measurement binary with the feature compiled but unarmed
#   dump    the six reproduced cells, with the dump armed
#   score   the joint table
#   det     two processes, on the armed run, on the dump, and on the scorer
#   all     every stage above, in order
#
# Every exit status is read directly on the line after the command rather than
# through a pipe. Do NOT pipe this script into `tee` or `tail`.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_0c9338d3-644-1
D="$W/docs/experiments/skip-pile-diagnostic"
E="$D/evidence"
T=/var/lib/t3/tmp/skippile
PARENTS="$W/docs/experiments/contact-block/drivers/parents12.json"
MATCHED="$W/docs/experiments/round-envelope-gate/evidence/matched.json"
# `round-envelope-gate/drivers/collect.sh`'s COMBO, character for character.
COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator
# Six cells: the four cheapest-rung parents that span the pile's size range
# (99, 242, 653 and 2807 skips, the last of them the barren-probe abort that
# confirms nothing at all), plus two deep cells so the sample is not a sample of
# the shallow end of the ladder.
CELLS='10@3341379,0@3341379,2@3341379,3@3341379,4@16000000,0@32000000'
SAMPLE=300
CENSUS=60
cd "$W" || exit 3
mkdir -p "$E" "$T/bin" "$T/out" "$T/dump" "$T/gates" "$T/det"
STAGE="${1:-all}"

run_build() {
  echo "== build: the measurement binary (the gate's COMBO, the kernel, the dump)"
  CARGO_TARGET_DIR="$T/tgt" cargo build --release --example general_request_benchmark \
    --features "$COMBO,round-envelope-kernel,skip-pile-dump" > "$E/build-meas.log" 2>&1
  B1=$?
  echo "build-meas exit=$B1"
  [ "$B1" -eq 0 ] || return 1
  cp "$T/tgt/release/examples/general_request_benchmark" "$T/bin/meas"

  echo "== build: the gate binary (jagua-experimental, feature ABSENT)"
  CARGO_TARGET_DIR="$T/tgt" cargo build --release --example general_request_benchmark \
    --features jagua-experimental > "$E/build-gate.log" 2>&1
  B2=$?
  echo "build-gate exit=$B2"
  [ "$B2" -eq 0 ] || return 1
  cp "$T/tgt/release/examples/general_request_benchmark" "$T/bin/gate-base"

  echo "== build: the scorer"
  CARGO_TARGET_DIR="$T/tgt" cargo build --release --example skip_pile_score \
    --features round-envelope-kernel,import-gate-shadow > "$E/build-score.log" 2>&1
  B3=$?
  echo "build-score exit=$B3"
  [ "$B3" -eq 0 ] || return 1
  cp "$T/tgt/release/examples/skip_pile_score" "$T/bin/score"

  {
    sha256sum "$T/bin/meas" "$T/bin/gate-base" "$T/bin/score"
    echo "# rustc: $(rustc --version)"
    echo "# cargo: $(cargo --version)"
    echo "# commit: $(git rev-parse HEAD)"
    echo "# python: $(python3 --version)"
    echo "# uname: $(uname -m) $(uname -r)"
    echo "# loadavg: $(cat /proc/loadavg)"
  } > "$E/binaries.txt"
  cat "$E/binaries.txt"
}

run_gates() {
  echo "== gates: the feature-ABSENT binary, which is the protocol's gate"
  python3 "$D/drivers/gates.py" base "$T/bin/gate-base" "$T/gates/base" \
    > "$E/gates-base-stdout.txt" 2>&1
  G1=$?
  echo "gates-base exit=$G1"
  cp "$T/gates/base/gates-base.json" "$E/gates-base.json"
  grep -E '"ALL_PASS"' "$E/gates-base-stdout.txt"

  echo "== gates: the measurement binary, dump compiled and UNARMED"
  python3 "$D/drivers/gates.py" meas "$T/bin/meas" "$T/gates/meas" \
    > "$E/gates-meas-stdout.txt" 2>&1
  G2=$?
  echo "gates-meas exit=$G2"
  cp "$T/gates/meas/gates-meas.json" "$E/gates-meas.json"
  grep -E '"ALL_PASS"' "$E/gates-meas-stdout.txt"
  echo "GATES exits=$G1/$G2"
}

run_dump() {
  echo "== the six reproduced cells, dump armed, checked against the gate's evidence"
  python3 "$D/drivers/dumpladder.py" "$T/out" "$T/bin/meas" "$PARENTS" \
    "$MATCHED" "$CELLS" "$T/dump" 20000 > "$E/dumpladder-stdout.txt" 2>&1
  L1=$?
  echo "dumpladder exit=$L1"
  cp "$T/out/dumpladder.json" "$E/dumpladder.json"
  [ "$L1" -eq 0 ]
}

run_score() {
  echo "== the scoring plan"
  python3 "$D/drivers/planbuild.py" "$T/out/dumpladder.json" \
    "$T/out/score-plan.json" "$SAMPLE" "$CENSUS" 6 \
    > "$E/planbuild-stdout.txt" 2>&1
  S1=$?
  echo "planbuild exit=$S1"
  cp "$T/out/score-plan.json" "$E/score-plan.json"
  echo "== three authorities on every sampled frontier"
  "$T/bin/score" "$T/out/score-plan.json" > "$T/out/score.json" \
    2> "$E/score.err"
  S2=$?
  echo "score exit=$S2"
  python3 "$D/drivers/summarize.py" "$T/out/score.json" "$E/summary.json" \
    > "$E/summarize-stdout.txt" 2>&1
  S3=$?
  echo "summarize exit=$S3"
  # The whole scoring document is large; the released rows, the joint table and
  # every censused record's per-pair attribution are what the README reads, so
  # those are committed and the raw document stays under $T.
  python3 - "$T/out/score.json" "$E/score-records.json" <<'PY'
import json, sys
score = json.load(open(sys.argv[1]))
trimmed = {k: v for k, v in score.items() if k != 'cells'}
trimmed['cells'] = [{
    **{k: v for k, v in cell.items() if k != 'records'},
    'records': [r for r in cell['records'] if r.get('census')],
} for cell in score['cells']]
json.dump(trimmed, open(sys.argv[2], 'w'), indent=1)
print(json.dumps({'censusedRecordsKept': sum(len(c['records'])
                                             for c in trimmed['cells'])}))
PY
  S4=$?
  echo "trim exit=$S4"
  echo "SCORE exits=$S1/$S2/$S3/$S4"
}

run_det() {
  echo "== determinism: the armed run, its dump, and the scorer"
  python3 "$D/drivers/planbuild.py" "$T/out/dumpladder.json" \
    "$T/out/det-plan.json" 12 4 6 > "$E/detplan-stdout.txt" 2>&1
  echo "detplan exit=$?"
  python3 "$D/drivers/determinism.py" "$E/determinism.json" "$T/bin/meas" \
    "$T/bin/score" "$T/out/det-plan.json" 10@3341379 2@3341379 \
    > "$E/determinism-stdout.txt" 2>&1
  D1=$?
  echo "determinism exit=$D1"
  cp "$T/out/det-plan.json" "$E/det-plan.json"
}

case "$STAGE" in
  build) run_build ;;
  gates) run_gates ;;
  dump) run_dump ;;
  score) run_score ;;
  det) run_det ;;
  all) run_build && run_gates && run_dump && run_score && run_det ;;
  *) echo "unknown stage $STAGE"; exit 2 ;;
esac
