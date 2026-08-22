#!/usr/bin/env bash
# Regenerates every evidence file in this directory from the committed tree.
#
#   collect.sh
#
# Order matters, and it is the campaign's provenance rule: build from the
# committed tree, gate, then measure. Every exit status is read directly on the
# line after the command rather than through a pipe.
#
# Two battery runs, not one. The first is on a binary carrying only this round's
# feature and the shadow the false-accept test reads its material distances
# from; the second adds `fast-contract-validator`, which is verdict-preserving
# and changes only the *speed* of the contract half - so the two documents must
# agree on every verdict, and the second is the one whose confirmation timing is
# the shipping-relevant one. Their agreement is itself a check.
set -u
W=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a
D="$W/docs/experiments/round-envelope-kernel"
E="$D/evidence"
cd "$W" || exit 3
mkdir -p "$E" /var/lib/t3/tmp/rek

PLAN="$D/drivers/battery-plan.json"

echo "== build: the battery instrument"
cargo build --release --features round-envelope-kernel,import-gate-shadow \
  --example round_envelope_battery > "$E/build-battery.log" 2>&1
B1=$?
echo "build-battery exit=$B1"
[ "$B1" -eq 0 ] || exit 1
cp target/release/examples/round_envelope_battery /var/lib/t3/tmp/rek-battery-base
R1=$?

echo "== build: the battery instrument with the contract certificate"
cargo build --release \
  --features round-envelope-kernel,import-gate-shadow,fast-contract-validator \
  --example round_envelope_battery > "$E/build-battery-fcv.log" 2>&1
B2=$?
echo "build-battery-fcv exit=$B2"
[ "$B2" -eq 0 ] || exit 1
cp target/release/examples/round_envelope_battery /var/lib/t3/tmp/rek-battery-fcv
R2=$?

echo "== build: the gate binary (jagua-experimental, feature OFF)"
cargo build --release --features jagua-experimental --example general_request_benchmark \
  > "$E/build-gate.log" 2>&1
B3=$?
echo "build-gate exit=$B3"
[ "$B3" -eq 0 ] || exit 1
cp target/release/examples/general_request_benchmark /var/lib/t3/tmp/rek/bench-off
R3=$?

echo "== build: the same binary with the feature compiled in"
cargo build --release --features jagua-experimental,round-envelope-kernel \
  --example general_request_benchmark > "$E/build-gate-rek.log" 2>&1
B4=$?
echo "build-gate-rek exit=$B4"
[ "$B4" -eq 0 ] || exit 1
cp target/release/examples/general_request_benchmark /var/lib/t3/tmp/rek/bench-on
R4=$?

echo "== step 1: the four pinned regression gates, feature OFF"
# The gate *documents* go to tmp and the per-gate summary lands in evidence:
# eight 150 kB benchmark outputs that a rerun regenerates are not evidence, and
# the digest that makes them checkable is in the summary. Gate A's own driver
# does the same.
python3 "$D/drivers/gates.py" rek-base /var/lib/t3/tmp/rek/bench-off \
  /var/lib/t3/tmp/rek/gates-base > "$E/gates-stdout.txt" 2>&1
cp /var/lib/t3/tmp/rek/gates-base/gates-rek-base.json "$E/gates-rek-base.json"
S0=$?
echo "gates exit=$S0"
grep -E '"ALL_PASS"' "$E/gates-stdout.txt"

echo "== step 1b: the four pinned gates again on the FEATURE-COMPILED binary"
# Compiled is not armed: with no `rek` key the wire point is not entered at all,
# so this binary must reproduce all four pinned gates too. It is the difference
# between "the feature is off by default" and "the feature is absent".
python3 "$D/drivers/gates.py" rek-compiled /var/lib/t3/tmp/rek/bench-on \
  /var/lib/t3/tmp/rek/gates-compiled > "$E/gates-compiled-stdout.txt" 2>&1
cp /var/lib/t3/tmp/rek/gates-compiled/gates-rek-compiled.json "$E/gates-rek-compiled.json"
S0B=$?
echo "gates-compiled exit=$S0B"
grep -E '"ALL_PASS"' "$E/gates-compiled-stdout.txt"

echo "== step 1c: the spec key's own gate"
python3 "$D/drivers/smoke.py" /var/lib/t3/tmp/rek/bench-off /var/lib/t3/tmp/rek/bench-on \
  "$E/smoke.json" > "$E/smoke-stdout.txt" 2>&1
S0C=$?
echo "smoke exit=$S0C"

echo "== step 2: the battery"
/var/lib/t3/tmp/rek-battery-base "$PLAN" > "$E/battery.json" 2> "$E/battery.err"
S1=$?
echo "battery exit=$S1"

echo "== step 3: the battery again, with the contract certificate armed"
/var/lib/t3/tmp/rek-battery-fcv "$PLAN" > "$E/battery-fcv.json" 2> "$E/battery-fcv.err"
S2=$?
echo "battery-fcv exit=$S2"

echo "== step 4: the two documents must agree on every verdict"
python3 "$D/drivers/equiv.py" "$E/battery.json" "$E/battery-fcv.json" \
  > "$E/equiv-stdout.txt" 2>&1
S3=$?
echo "equiv exit=$S3"

echo "== step 5: the four verdicts"
python3 "$D/drivers/summarize.py" "$E/battery.json" "$E/summary.json" "$E/battery-fcv.json" \
  > "$E/summary-stdout.txt" 2>&1
S4=$?
echo "summarize exit=$S4"

echo "== step 6: determinism, two processes, whole document less the timings"
python3 "$D/drivers/determinism.py" /var/lib/t3/tmp/rek-battery-base "$PLAN" \
  > "$E/determinism-stdout.txt" 2>&1
S5=$?
echo "determinism exit=$S5"

echo "== binary manifest"
{
  sha256sum /var/lib/t3/tmp/rek-battery-base
  sha256sum /var/lib/t3/tmp/rek-battery-fcv
  sha256sum /var/lib/t3/tmp/rek/bench-off
  sha256sum /var/lib/t3/tmp/rek/bench-on
  echo "# rustc: $(rustc --version)"
  echo "# cargo: $(cargo --version)"
  echo "# commit: $(git rev-parse HEAD)"
  echo "# python: $(python3 --version)"
  echo "# uname: $(uname -m) $(uname -r)"
} > "$E/binaries.txt"
cat "$E/binaries.txt"

echo "== stamp the manifest into every evidence document"
python3 "$D/drivers/stamp.py" "$E" > "$E/stamp-stdout.txt" 2>&1
S6=$?
echo "stamp exit=$S6"

echo "EXITS build=$B1/$B2/$B3/$B4 copy=$R1/$R2/$R3/$R4 gates=$S0 gatescompiled=$S0B smoke=$S0C battery=$S1 batteryfcv=$S2 equiv=$S3 summarize=$S4 determinism=$S5 stamp=$S6"
[ "$S0" -eq 0 ] && [ "$S0B" -eq 0 ] && [ "$S0C" -eq 0 ] && [ "$S1" -eq 0 ] \
  && [ "$S2" -eq 0 ] && [ "$S3" -eq 0 ] && [ "$S4" -eq 0 ] && [ "$S5" -eq 0 ] \
  && [ "$S6" -eq 0 ]
