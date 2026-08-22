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
#   final   the protocol's closing gate: a FRESH target directory, the clean
#           committed tree, all four gates with the feature off and again with
#           it compiled and unarmed
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
# 20000 is above every cell's dump, so the scoring stage is a **census** of
# these six cells' whole skip pile and not a sample of it. It is affordable
# because the three composite verdicts short-circuit on the first violation and
# most of this pile violates early; the expensive full pair-and-boundary scan is
# what CENSUS budgets, and every disagreeing record gets one whatever it says.
SAMPLE=20000
CENSUS=400
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
    "$T/out/dumpladder.json" > "$E/summarize-stdout.txt" 2>&1
  S3=$?
  echo "summarize exit=$S3"
  # The whole scoring document is ~40 MB because it carries a row per frontier
  # per allowance. What a reader needs is every record that DISAGREED - those
  # are the finding - plus enough agreeing censused records to characterise the
  # rest of the pile. Both are kept; the raw document stays under $T.
  python3 - "$T/out/score.json" "$E/score-records.json" <<'PY'
import json, sys
KEEP_AGREEING = 40
score = json.load(open(sys.argv[1]))
trimmed = {k: v for k, v in score.items() if k != 'cells'}
cells = []
for cell in score['cells']:
    interesting, agreeing = [], []
    for record in cell['records']:
        if not record.get('census'):
            continue
        census = record['census']
        disagrees = (
            any(row['released'] or row['kernelRefusesMiterAccepts']
                for row in record['allowances'])
            or census['kernelAdmitsMiterRefuses']
            or census['miterAdmitsKernelRefuses']
            or census['boundaries']['kernelAdmitsMiterRefuses']
            or census['boundaries']['miterAdmitsKernelRefuses'])
        (interesting if disagrees else agreeing).append(record)
    step = max(1, len(agreeing) // KEEP_AGREEING)
    cells.append({
        **{k: v for k, v in cell.items() if k != 'records'},
        'censusedRecordsThatDisagreed': len(interesting),
        'censusedRecordsThatAgreed': len(agreeing),
        'agreeingRecordsKeptStride': step,
        'records': interesting + agreeing[::step][:KEEP_AGREEING],
    })
trimmed['cells'] = cells
json.dump(trimmed, open(sys.argv[2], 'w'), indent=1)
print(json.dumps({'disagreeingKept': sum(c['censusedRecordsThatDisagreed']
                                         for c in cells),
                  'recordsKept': sum(len(c['records']) for c in cells)}))
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

run_final() {
  # The protocol's closing requirement, run as a stage rather than by hand: a
  # FRESH target directory, the clean committed tree, all four gates with the
  # feature off. `git status --porcelain` is printed first because the claim is
  # about the committed tree and a dirty one would not be it.
  echo "== closing gate: git status --porcelain (must be empty)"
  git status --porcelain | tee "$E/final-worktree-status.txt"
  echo "== closing gate: fresh build of the clean committed tree"
  rm -rf "$T/final"
  CARGO_TARGET_DIR="$T/final" cargo build --release --example general_request_benchmark \
    --features jagua-experimental > "$E/build-final-gate.log" 2>&1
  F1=$?
  echo "build-final-gate exit=$F1"
  [ "$F1" -eq 0 ] || return 1
  cp "$T/final/release/examples/general_request_benchmark" "$T/bin/gate-final"
  CARGO_TARGET_DIR="$T/final" cargo build --release --example general_request_benchmark \
    --features "$COMBO,round-envelope-kernel,skip-pile-dump" > "$E/build-final-meas.log" 2>&1
  F2=$?
  echo "build-final-meas exit=$F2"
  [ "$F2" -eq 0 ] || return 1
  cp "$T/final/release/examples/general_request_benchmark" "$T/bin/meas-final"
  CARGO_TARGET_DIR="$T/final" cargo build --release --example skip_pile_score \
    --features round-envelope-kernel,import-gate-shadow > "$E/build-final-score.log" 2>&1
  F3=$?
  echo "build-final-score exit=$F3"
  [ "$F3" -eq 0 ] || return 1
  cp "$T/final/release/examples/skip_pile_score" "$T/bin/score-final"
  {
    sha256sum "$T/bin/gate-final" "$T/bin/meas-final" "$T/bin/score-final"
    echo "# rustc: $(rustc --version)"
    echo "# cargo: $(cargo --version)"
    echo "# commit: $(git rev-parse HEAD)"
    echo "# uname: $(uname -m) $(uname -r)"
    echo "# loadavg: $(cat /proc/loadavg)"
  } > "$E/binaries-final.txt"
  cat "$E/binaries-final.txt"
  echo "== closing gate: the four gates on the fresh feature-ABSENT binary"
  python3 "$D/drivers/gates.py" final "$T/bin/gate-final" "$T/gates/final" \
    > "$E/gates-final-stdout.txt" 2>&1
  F4=$?
  echo "gates-final exit=$F4"
  cp "$T/gates/final/gates-final.json" "$E/gates-final.json"
  grep -E '"ALL_PASS"' "$E/gates-final-stdout.txt"
  echo "== closing gate: the same on the fresh measurement binary, dump UNARMED"
  python3 "$D/drivers/gates.py" finalmeas "$T/bin/meas-final" "$T/gates/finalmeas" \
    > "$E/gates-finalmeas-stdout.txt" 2>&1
  F5=$?
  echo "gates-finalmeas exit=$F5"
  cp "$T/gates/finalmeas/gates-finalmeas.json" "$E/gates-finalmeas.json"
  grep -E '"ALL_PASS"' "$E/gates-finalmeas-stdout.txt"
  echo "FINAL exits=$F1/$F2/$F3/$F4/$F5"
}

case "$STAGE" in
  build) run_build ;;
  gates) run_gates ;;
  dump) run_dump ;;
  score) run_score ;;
  det) run_det ;;
  final) run_final ;;
  all) run_build && run_gates && run_dump && run_score && run_det && run_final ;;
  *) echo "unknown stage $STAGE"; exit 2 ;;
esac
