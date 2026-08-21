#!/usr/bin/env bash
# §2.3's isolation: the indexed probe queue against the linear scan that ships.
#
#   run-probequeue-ab.sh
#
# The census cannot answer this. Its `probeScanNanos` region includes the clock
# pair that measures it, and the before/after populations differ anyway - §2.2's
# fast path removed 47% of the calls, and the ones it removed were exactly the
# full-queue misses that made the scan look expensive. So the two
# implementations are compared the only way a shared box supports: build one
# binary per implementation off the same tree, and pair them with `ablate.py`'s
# equal-work slice comparison.
#
# The indexed variant lives in `probequeue-index.patch` rather than behind a
# feature, because it is not a lever anyone should be able to arm by accident -
# it is a measured regression at the shipped window size, kept only so the
# comparison can be re-run if `ROTATION_CACHE_PROBE_CAPACITY` ever changes.
#
# The tree is restored in a trap, so an interrupted run does not leave it
# half-patched.
set -eu
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_08a442a7-1aa-1
D="$W/docs/experiments/rotation-tax/drivers"
SRC="$W/crates/polygon-nesting-core/src/search/general_relaxed.rs"
KEEP="${TMPDIR:-/tmp}/general_relaxed.keep.rs"
BIN=/var/lib/t3/rt/bin
OUT="${V4_OUT:-/var/lib/t3/rt/out}"
FEATURES=jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,fast-contract-validator

cp "$SRC" "$KEEP"
trap 'cp "$KEEP" "$SRC"' EXIT

# `patch` fails loudly on a stale anchor, which is the point: a silent no-op
# would build the same binary twice and report 1.00x as if it had isolated
# something.
patch -p1 -d "$W" --no-backup-if-mismatch < "$D/probequeue-index.patch"

CARGO_TARGET_DIR=/var/lib/t3/rt/tgt-deque cargo build --release \
    --manifest-path "$W/Cargo.toml" \
    --example general_request_benchmark --features "$FEATURES" 2>&1 | tail -2
cp /var/lib/t3/rt/tgt-deque/release/examples/general_request_benchmark "$BIN/index-meas"
echo "built $BIN/index-meas"

cp "$KEEP" "$SRC"   # back to the shipped deque before the comparison runs

python3 "$D/ablate.py" "$OUT/probequeue-ab" \
    "$BIN/commit-meas" "$BIN/index-meas" \
    "$W/docs/experiments/parallel-compression-schedule/evidence/parents.json" \
    8 0,1,2
echo "ablate exit=$?"
