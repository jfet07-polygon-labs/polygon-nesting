#!/usr/bin/env bash
# Regenerates every evidence file in this directory from the committed tree.
#
#   collect.sh
#
# Order matters: the import fixture is an input to the shadow, the shadow's raw
# verdicts are an input to the summary and the failure list, and the binary
# manifest is written last so it records the binaries that actually produced the
# run above it. Every exit status is read directly on the line after the command
# rather than through a pipe.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f674db6b-1e0-1
D="$W/docs/experiments/gate-a-sparrow-import"
E="$D/evidence"
cd "$W" || exit 3
mkdir -p "$E"

REQ="$W/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json"
POSES="$D/fixture/sparrow-10s-x86-poses.json"

# The three envelope radii the round measures, as `search_offset_allowance_mm`:
#   0.002  - the from-request default, HEAD's acceptance authority
#   0.0005 - the record lineage's trailing allowance (gates g2/g3/g4)
#   0.0    - no allowance, so the envelope radius is exactly total_padding/2,
#            which is the radius Sol review 11 and Grok review 6 A.1 both name
ALLOWANCES=0.002,0.0005,0.0
# Clipper arc tolerance in canonical grid units. 0.1 grid units = 0.0001 mm of
# inward chord deviation, a tenth of the grid step; Clipper's own default here
# would be radius/500 = 0.005 mm, five grid steps, which would decide the
# verdict by itself.
ARCTOL=0.1
# Bisect the 40 tightest pairs by material clearance, plus every failing row.
BISECT=40

echo "== build: the shadow instrument"
cargo build --release --features import-gate-shadow --example sparrow_import_gate \
  > "$E/build-shadow.log" 2>&1
B1=$?
echo "build-shadow exit=$B1"
[ "$B1" -eq 0 ] || exit 1

echo "== build: the gate binary"
cargo build --release --features jagua-experimental --example general_request_benchmark \
  > "$E/build-gate.log" 2>&1
B2=$?
echo "build-gate exit=$B2"
[ "$B2" -eq 0 ] || exit 1

echo "== step 1: import and conversion audit"
python3 "$D/drivers/import10s.py" > "$E/import-stdout.txt" 2>&1
S1=$?
echo "import exit=$S1"

echo "== step 2: the three verdicts"
./target/release/examples/sparrow_import_gate \
  "$REQ" "$POSES" 5 5 "$ALLOWANCES" "$ARCTOL" "$BISECT" > "$E/verdicts.json" 2> "$E/verdicts.err"
S2=$?
echo "verdicts exit=$S2"

echo "== step 3: the interpretation table"
python3 "$D/drivers/summarize.py" > "$E/summary-stdout.txt" 2>&1
S3=$?
echo "summarize exit=$S3"

echo "== step 4: name the refused rows"
python3 "$D/drivers/failures.py" > "$E/failures-stdout.txt" 2>&1
S4=$?
echo "failures exit=$S4"

echo "== step 4b: determinism, two processes, whole document"
python3 "$D/drivers/determinism.py" \
  "$W/target/release/examples/sparrow_import_gate" "$REQ" "$POSES" \
  "$ALLOWANCES" "$ARCTOL" "$BISECT" > "$E/determinism-stdout.txt" 2>&1
S4B=$?
echo "determinism exit=$S4B"

echo "== step 5: the re-pinned lower bound"
python3 "$W/docs/experiments/depth-lower-bound/repin-evidence.py" \
  > "$E/lower-bound-stdout.txt" 2>&1
S5=$?
echo "lower-bound exit=$S5"

echo "== binary manifest"
{
  sha256sum target/release/examples/sparrow_import_gate
  sha256sum target/release/examples/general_request_benchmark
  echo "# rustc: $(rustc --version)"
  echo "# cargo: $(cargo --version)"
  echo "# commit: $(git rev-parse HEAD)"
  echo "# python: $(python3 --version)"
} > "$E/binaries.txt"
cat "$E/binaries.txt"

echo "== stamp the manifest into every evidence document"
python3 "$D/drivers/stamp.py" "$E" \
  "$W/docs/experiments/depth-lower-bound/depth-lower-bound-exact-clearance-evidence.json" \
  > "$E/stamp-stdout.txt" 2>&1
S6=$?
echo "stamp exit=$S6"

echo "EXITS build=$B1/$B2 import=$S1 verdicts=$S2 summarize=$S3 failures=$S4 determinism=$S4B bound=$S5 stamp=$S6"
[ "$S1" -eq 0 ] && [ "$S2" -eq 0 ] && [ "$S3" -eq 0 ] && [ "$S4" -eq 0 ] \
  && [ "$S4B" -eq 0 ] && [ "$S5" -eq 0 ] && [ "$S6" -eq 0 ]
