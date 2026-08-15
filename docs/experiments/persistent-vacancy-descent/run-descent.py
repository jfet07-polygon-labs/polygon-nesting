#!/usr/bin/env python3
"""Deterministic Mixed-61 descending-target contraction driver.

Chains persistent-vacancy mode 11 runs: each completed hop's exact-valid,
dual-gate-validated layout becomes the pinned parent fixture for the next,
deeper target. Every hop is one deterministic cold engine invocation; the
driver only orchestrates targets and fixtures, so replaying the recorded
target sequence reproduces the chain byte-for-byte on the same build,
machine, and toolchain.

Usage:
  python3 docs/experiments/persistent-vacancy-descent/run-descent.py OUTPUT_DIR \
      [--binary target/release/examples/general_request_benchmark] \
      [--start-target 168.55] [--initial-delta 0.10] [--max-delta 0.40] \
      [--min-delta 0.005] [--max-hops 120] [--mode 11] \
      [--targets 168.55,168.544,...]

With --targets the adaptive schedule is bypassed and the exact recorded
sequence is replayed, which is how the committed chain evidence is verified.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
REQUEST = "tests/fixtures/mixed-61/mixed61-request.json"
ROOT_FIXTURE = "tests/fixtures/mixed-61/persistent-vacancy-parent-b9335a72.json"
CANONICAL_ARGS = (
    "1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 0 0.005 0.001 "
    "1 6 0 0 0 structured 0 10 1 0 0 0 0"
).split()
REQUEST_SHA = "dfd2ceecf02efe3475e3344dfefbfb2a2a5bd8a673008b449f5689507c933ba1"


def sha256(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def write_fixture(path, placements, fingerprint, target, independent, hop):
    fixture = {
        "schemaVersion": 1,
        "description": (
            f"Descent-chain parent produced by persistent-vacancy mode 11 "
            f"hop {hop} at target {target} mm."
        ),
        "requestSha256": REQUEST_SHA,
        "expectedPlacementFingerprint": fingerprint,
        "reportedDepthMm": target,
        "independentDepthMm": independent,
        "provenance": {
            "producedBy": f"mode 11 descent hop {hop}",
            "targetDepthMm": target,
        },
        "placements": placements,
    }
    with open(path, "w") as handle:
        json.dump(fixture, handle, indent=2)
        handle.write("\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output_dir")
    parser.add_argument(
        "--binary", default="target/release/examples/general_request_benchmark"
    )
    parser.add_argument("--start-target", type=float, default=168.55)
    parser.add_argument("--initial-delta", type=float, default=0.10)
    parser.add_argument("--max-delta", type=float, default=0.40)
    parser.add_argument("--min-delta", type=float, default=0.005)
    parser.add_argument("--max-hops", type=int, default=120)
    parser.add_argument("--mode", default="11")
    parser.add_argument("--targets", default=None)
    parser.add_argument(
        "--require-hops",
        type=int,
        default=None,
        help="fail unless at least this many hops complete (verification mode)",
    )
    parser.add_argument(
        "--require-final-depth",
        type=float,
        default=None,
        help="fail unless the final independent depth matches to 1e-9 mm",
    )
    args = parser.parse_args()
    if args.require_hops is not None and args.require_hops < 0:
        parser.error("--require-hops must be non-negative")
    if args.require_final_depth is not None and not (
        args.require_final_depth == args.require_final_depth
        and abs(args.require_final_depth) != float("inf")
    ):
        parser.error("--require-final-depth must be a finite number")

    os.makedirs(args.output_dir, exist_ok=True)
    binary = os.path.join(REPO, args.binary)
    request = os.path.join(REPO, REQUEST)
    fixture = os.path.join(REPO, ROOT_FIXTURE)
    replay_targets = (
        [float(value) for value in args.targets.split(",")] if args.targets else None
    )

    target = replay_targets[0] if replay_targets else args.start_target
    delta = args.initial_delta
    hop = 0
    attempt = 0
    chain = []
    failures = []
    while hop < args.max_hops:
        if replay_targets is not None:
            if attempt >= len(replay_targets):
                break
            target = replay_targets[attempt]
        attempt += 1
        command = [binary, request] + CANONICAL_ARGS + [
            args.mode,
            fixture,
            f"{target:.3f}",
        ]
        result = subprocess.run(command, capture_output=True, text=True)
        if result.returncode != 0:
            print(f"hop {hop}: engine error: {result.stderr[-400:]}", file=sys.stderr)
            return 1
        data = json.loads(result.stdout)
        vacancy = data["relaxedDiagnostics"]["coupledDynamicSeparator"][
            "persistentVacancyPopulation"
        ]
        if vacancy.get("exactValid"):
            independent = vacancy["independentDepthMm"]
            fingerprint = vacancy["finalPlacementFingerprint"]
            raw = os.path.join(args.output_dir, f"hop{hop:03d}-t{target:.3f}.json")
            with open(raw, "w") as handle:
                handle.write(result.stdout)
            next_fixture = os.path.join(args.output_dir, f"parent-hop{hop:03d}.json")
            write_fixture(
                next_fixture,
                vacancy["finalPlacements"],
                fingerprint,
                round(target, 3),
                independent,
                hop,
            )
            chain.append(
                {
                    "hop": hop,
                    "targetDepthMm": round(target, 3),
                    "independentDepthMm": independent,
                    "placementFingerprint": fingerprint,
                    "settleAcceptedMoves": vacancy["settle"]["acceptedMoves"],
                    "populationLayers": vacancy["layersCompleted"],
                    "rawOutput": os.path.basename(raw),
                    "rawOutputSha256": sha256(raw),
                    "parentFixture": os.path.basename(next_fixture),
                    "parentFixtureSha256": sha256(next_fixture),
                }
            )
            print(
                f"hop {hop}: COMPLETE target {target:.3f} -> {independent:.3f} mm "
                f"(settle {vacancy['settle']['acceptedMoves']}, "
                f"layers {vacancy['layersCompleted']})",
                flush=True,
            )
            fixture = next_fixture
            hop += 1
            if replay_targets is None:
                target = round(target - delta, 3)
                delta = min(delta * 2, args.max_delta)
        else:
            fail_raw = os.path.join(
                args.output_dir, f"fail{attempt - 1:03d}-t{target:.3f}.json"
            )
            with open(fail_raw, "w") as handle:
                handle.write(result.stdout)
            failures.append(
                {
                    "attempt": attempt - 1,
                    "targetDepthMm": round(target, 3),
                    "afterHop": hop,
                    "rawOutput": os.path.basename(fail_raw),
                    "rawOutputSha256": sha256(fail_raw),
                    "settleAcceptedMoves": vacancy.get("settle", {}).get("acceptedMoves"),
                    "initialInactivePieces": len(
                        vacancy.get("initialInactivePieceIds") or []
                    ),
                    "terminalBestInactivePieces": (
                        (vacancy.get("layers") or [{}])[-1].get("bestInactivePieceCount")
                    ),
                    "failureReason": vacancy.get("failureReason"),
                }
            )
            print(f"hop {hop}: fail target {target:.3f} delta {delta}", flush=True)
            if replay_targets is None:
                if delta <= args.min_delta + 1e-12:
                    break
                delta = max(delta / 2, args.min_delta)
                last = chain[-1]["targetDepthMm"] if chain else args.start_target
                target = round(last - delta, 3)
    with open(os.path.join(args.output_dir, "chain.json"), "w") as handle:
        json.dump(chain, handle, indent=2)
        handle.write("\n")
    with open(os.path.join(args.output_dir, "chain-failures.json"), "w") as handle:
        json.dump(failures, handle, indent=2)
        handle.write("\n")
    if chain:
        print(
            f"final: {chain[-1]['independentDepthMm']} mm after {len(chain)} hops",
            flush=True,
        )
    if args.require_hops is not None and len(chain) < args.require_hops:
        print(
            f"VERIFICATION FAILED: {len(chain)} hops < required {args.require_hops}",
            file=sys.stderr,
        )
        return 2
    if args.require_final_depth is not None:
        if not chain or abs(chain[-1]["independentDepthMm"] - args.require_final_depth) > 1e-9:
            observed = chain[-1]["independentDepthMm"] if chain else None
            print(
                f"VERIFICATION FAILED: final depth {observed} != {args.require_final_depth}",
                file=sys.stderr,
            )
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
