#!/usr/bin/env bash
# Regenerates every evidence file in this directory from the committed tree.
#
#   collect.sh [stage]
#
# Stages, in the campaign's provenance order - build from the committed tree,
# gate, then measure:
#
#   build     the two binaries
#   gates     the four pinned regression gates on both of them
#   matched   deliverable 1: the twelve-parent matched gate ladder
#   ladder    just the ladder half of `matched`
#   matchedrest  the arm's extra rung, the merge and the wall-ratio replicas
#   reach     deliverable 2: the reachability A/B
#   anytime   deliverable 3: the anytime table
#   exclusive the `rek=2` arm on the same parents, for the record
#   audit     every arm's publications, re-asked of both authorities, and the
#             pre-committed rule. Runs last because it audits all three.
#   det       two-process determinism
#   all       every stage above, in order
#
# Every exit status is read directly on the line after the command rather than
# through a pipe. Do NOT pipe this script into `tee` or `tail`.
#
# The measurement stages are run one at a time and never beside each other:
# deliverable 1's headline is an equal-**wall** comparison, and a second job of
# this round's own on the same box would be pollution this round chose.
set -u
W=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a
D="$W/docs/experiments/round-envelope-gate"
E="$D/evidence"
T=/var/lib/t3/tmp/rekgate
PARENTS="$W/docs/experiments/contact-block/drivers/parents12.json"
COMBO=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator
WORKS=3341379,8000000,16000000,32000000
ARMWORKS=48000000
cd "$W" || exit 3
mkdir -p "$E" "$T/bin" "$T/out"
STAGE="${1:-all}"

run_build() {
  echo "== build: the measurement binary (the combo plus the kernel)"
  CARGO_TARGET_DIR="$T/tgt" cargo build --release --example general_request_benchmark \
    --features "$COMBO,round-envelope-kernel" > "$E/build-meas.log" 2>&1
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

  echo "== build: the battery instrument (the publication audit's authority)"
  CARGO_TARGET_DIR="$T/tgt" cargo build --release --example round_envelope_battery \
    --features round-envelope-kernel,import-gate-shadow > "$E/build-battery.log" 2>&1
  B3=$?
  echo "build-battery exit=$B3"
  [ "$B3" -eq 0 ] || return 1
  cp "$T/tgt/release/examples/round_envelope_battery" "$T/bin/battery"

  {
    sha256sum "$T/bin/meas" "$T/bin/gate-base" "$T/bin/battery"
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

  echo "== gates: the measurement binary, feature compiled and UNARMED"
  python3 "$D/drivers/gates.py" meas "$T/bin/meas" "$T/gates/meas" \
    > "$E/gates-meas-stdout.txt" 2>&1
  G2=$?
  echo "gates-meas exit=$G2"
  cp "$T/gates/meas/gates-meas.json" "$E/gates-meas.json"
  grep -E '"ALL_PASS"' "$E/gates-meas-stdout.txt"
  echo "GATES exits=$G1/$G2"
}

run_ladder() {
  echo "== deliverable 1: the twelve-parent matched gate, both arms, four budgets"
  python3 "$D/drivers/matchedgate.py" "$T/out/matched" "$T/bin/meas" "$PARENTS" \
    "$WORKS" miter,union 1.0 0.002 > "$E/matched-stdout.txt" 2>&1
  M1=$?
  echo "matchedgate exit=$M1"
  [ "$M1" -eq 0 ]
}

run_matchedrest() {
  echo "== the arm's own ladder, one rung above the control's top budget"
  # Without it the equal-wall reading at the top budget is a clamp rather than a
  # measurement: the arm is the cheaper of the two, so its ladder ends at a
  # *shorter* wall than the control's and the control's top wall falls off the
  # end of it.
  python3 "$D/drivers/matchedgate.py" "$T/out/matched-top" "$T/bin/meas" "$PARENTS" \
    "$ARMWORKS" union 1.0 0.002 > "$E/matched-top-stdout.txt" 2>&1
  M2=$?
  echo "matchedgate-top exit=$M2"
  python3 "$D/drivers/mergeladders.py" "$T/out/matched/matchedgate.json" \
    "$T/out/matched-top/matchedgate.json" "$E/matched.json" \
    > "$E/mergeladders-stdout.txt" 2>&1
  M3=$?
  echo "mergeladders exit=$M3"
  echo "== the wall ratio, five interleaved replicas on three parents"
  python3 "$D/drivers/wallratio.py" "$T/out/wallratio" "$T/bin/meas" "$PARENTS" \
    16000000 0,1,2 5 "$E/wallratio.json" > "$E/wallratio-stdout.txt" 2>&1
  M4=$?
  echo "wallratio exit=$M4"
  echo "MATCHED exits=$M2/$M3/$M4"
}

run_matched() { run_ladder && run_matchedrest; }

run_exclusive() {
  # The arm the brief specified, measured rather than cited. The previous round
  # reported that six of the twelve parents are not `exclusive`-valid at the
  # 0.002 record-lineage allowance; this is that claim re-run at the cheapest
  # rung of this round's own ladder, so the choice of `union` as the promotion
  # candidate is this round's measurement and not an inherited one.
  echo "== the exclusive arm on the same twelve parents, cheapest rung"
  python3 "$D/drivers/matchedgate.py" "$T/out/exclusive" "$T/bin/meas" "$PARENTS" \
    3341379 exclusive 1.0 0.002 > "$E/exclusive-stdout.txt" 2>&1
  X1=$?
  echo "exclusive exit=$X1"
  cp "$T/out/exclusive/matchedgate.json" "$E/exclusive.json"
}

run_audit() {
  echo "== deliverable 1: every publication, re-asked of both authorities"
  python3 "$D/drivers/publications.py" "$E/matched.json" "$T/out/matched" \
    "$T/out/publications" "$T/out/publications-plan.json" \
    > "$E/publications-stdout.txt" 2>&1
  A1=$?
  echo "publications exit=$A1"
  "$T/bin/battery" "$T/out/publications-plan.json" \
    > "$T/out/publication-battery.json" 2> "$E/publication-battery.err"
  A2=$?
  echo "publication-battery exit=$A2"
  python3 "$D/drivers/pubaudit.py" "$T/out/publication-battery.json" \
    "$T/out/publications/publication-index.json" "$E/publication-audit.json" \
    > "$E/pubaudit-stdout.txt" 2>&1
  A3=$?
  echo "pubaudit exit=$A3"
  python3 "$D/drivers/sparrowcheck.py" "$T/out/publication-battery.json" \
    "$E/sparrow-republish.json" > "$E/sparrowcheck-stdout.txt" 2>&1
  A4=$?
  echo "sparrowcheck exit=$A4"
  echo "== the coordinator arms' publications, on the same authority"
  for name in reach-work reach-wall anytime anytime-wall; do
    [ -f "$E/$name.json" ] || { echo "publication-audit-$name skipped (no run)"; continue; }
    python3 "$D/drivers/publications.py" "$E/$name.json" "$T/out/$name" \
      "$T/out/pub-$name" "$T/out/pub-$name-plan.json" \
      >> "$E/publications-stdout.txt" 2>&1
    "$T/bin/battery" "$T/out/pub-$name-plan.json" \
      > "$T/out/pub-$name-battery.json" 2>> "$E/publication-battery.err"
    echo "publication-battery-$name exit=$?"
    python3 "$D/drivers/pubaudit.py" "$T/out/pub-$name-battery.json" \
      "$T/out/pub-$name/publication-index.json" \
      "$E/publication-audit-$name.json" >> "$E/pubaudit-stdout.txt" 2>&1
    echo "pubaudit-$name exit=$?"
  done
  echo "== the pre-committed rule, clause by clause"
  python3 "$D/drivers/gateverdict.py" "$E/matched.json" union miter \
    "$E/gate-verdict.json" "$E/publication-audit.json" \
    > "$E/gateverdict-stdout.txt" 2>&1
  A5=$?
  echo "gateverdict exit=$A5"
  echo "AUDIT exits=$A1/$A2/$A3/$A4/$A5"
}

run_reach() {
  echo "== deliverable 2: reachability, work=40M (the reproducible 10 s equivalent)"
  python3 "$D/drivers/coordarm.py" "$T/out/reach-work" "$T/bin/meas" mixed-61 \
    0,1,2 1 work 40000000 'base:,crot:crot=1,rek:rek=1,rekcrot:crot=1;rek=1' \
    > "$E/reach-work-stdout.txt" 2>&1
  R1=$?
  echo "reach-work exit=$R1"
  cp "$T/out/reach-work/coordarm.json" "$E/reach-work.json"
  echo "== deliverable 2: reachability, wall=10 s (the budget the -3.721 mm is on)"
  python3 "$D/drivers/coordarm.py" "$T/out/reach-wall" "$T/bin/meas" mixed-61 \
    0,1,2 3 wall 10000 'base:,crot:crot=1,rek:rek=1,rekcrot:crot=1;rek=1' \
    > "$E/reach-wall-stdout.txt" 2>&1
  R2=$?
  echo "reach-wall exit=$R2"
  cp "$T/out/reach-wall/coordarm.json" "$E/reach-wall.json"
  python3 "$D/drivers/crotflip.py" "$E/reach-work.json" "$E/reach-wall.json" \
    "$E/crot-flip.json" > "$E/crotflip-stdout.txt" 2>&1
  R3=$?
  echo "crotflip exit=$R3"
  echo "REACH exits=$R1/$R2/$R3"
}

run_anytime() {
  echo "== deliverable 3: the anytime table, plan mode, two processes per cell"
  python3 "$D/drivers/coordarm.py" "$T/out/anytime" "$T/bin/meas" mixed-61 \
    0,1,2 2 plan 3000,10000,30000 \
    'canonical:replan=1,rek:replan=1;rek=1' > "$E/anytime-stdout.txt" 2>&1
  N1=$?
  echo "anytime exit=$N1"
  cp "$T/out/anytime/coordarm.json" "$E/anytime.json"
  echo "== the same at wall=10 s, the arm every earlier millimetre is on"
  python3 "$D/drivers/coordarm.py" "$T/out/anytime-wall" "$T/bin/meas" mixed-61 \
    0,1,2 2 wall 10000 'canonical:,rek:rek=1' \
    > "$E/anytime-wall-stdout.txt" 2>&1
  N2=$?
  echo "anytime-wall exit=$N2"
  cp "$T/out/anytime-wall/coordarm.json" "$E/anytime-wall.json"
  echo "ANYTIME exits=$N1/$N2"
}

run_det() {
  echo "== determinism: two processes, whole document, timings stripped by name"
  python3 "$D/drivers/determinism.py" "$E/determinism.json" "$T/bin/meas" \
    'm34-seed0-miter|miter|m34|/var/lib/t3/tmp/csched/parents/parent-seed0.json;173.20812003998896;16000000' \
    'm34-seed0-union|union|m34|/var/lib/t3/tmp/csched/parents/parent-seed0.json;173.20812003998896;16000000' \
    'm34-seed4-union|union|m34|/var/lib/t3/tmp/csched/parents2/parent-seed4.json;170.64953207726535;16000000' \
    'v3-plan-canonical|miter|v3|0;plan=10000,cells=13:15:17:19,v3=1,replan=1' \
    'v3-plan-rek|miter|v3|0;plan=10000,cells=13:15:17:19,v3=1,replan=1,rek=1' \
    'v3-work-rekcrot|miter|v3|1;work=40000000,cells=11:15:21:27,v3=1,crot=1,rek=1' \
    > "$E/determinism-stdout.txt" 2>&1
  D1=$?
  echo "determinism exit=$D1"
}

case "$STAGE" in
  build) run_build ;;
  gates) run_gates ;;
  matched) run_matched ;;
  ladder) run_ladder ;;
  matchedrest) run_matchedrest ;;
  audit) run_audit ;;
  exclusive) run_exclusive ;;
  reach) run_reach ;;
  anytime) run_anytime ;;
  det) run_det ;;
  all)
    run_build && run_gates && run_matched && run_reach && run_anytime \
      && run_exclusive && run_audit && run_det
    ;;
  *) echo "unknown stage $STAGE"; exit 2 ;;
esac
