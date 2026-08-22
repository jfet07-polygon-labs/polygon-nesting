#!/usr/bin/env python3
"""Two battery documents must agree on every verdict.

    equiv.py A.json B.json

The two are produced by binaries that differ only in `fast-contract-validator`,
whose broad phase is a *proof* of clearance rather than an estimate of it and
which therefore cannot change a verdict. This is what says so on this round's
own corpus rather than by inheritance, and it doubles as a second determinism
check across two different binaries: everything but the timings has to match,
including the exact critical clearances, the refused-row index lists and the
false-accept counts.

Exits non-zero on the first disagreement.
"""

import json
import sys

TIMED = {
    "envelopeHalfMiterMs",
    "envelopeHalfRoundMs",
    "envelopeHalfRatio",
    "envelopeHalfRatioMedian",
    "confirmationMiterMs",
    "confirmationRoundMs",
    "confirmationRatio",
    "confirmationRatioMedianComparable",
    "confirmationUnionMs",
    "confirmationUnionRatio",
    "confirmationUnionRatioMedianComparable",
}
# The binary's own identity differs by construction: two binaries are the point.
IDENTITY = {"executableSha256"}


def strip(node):
    if isinstance(node, dict):
        return {
            k: strip(v)
            for k, v in sorted(node.items())
            if k not in TIMED and k not in IDENTITY
        }
    if isinstance(node, list):
        return [strip(v) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def differences(left, right, path="", out=None):
    if out is None:
        out = []
    if len(out) > 40:
        return out
    if isinstance(left, dict) and isinstance(right, dict):
        for key in sorted(set(left) | set(right)):
            differences(left.get(key), right.get(key), f"{path}/{key}", out)
    elif isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            out.append({"path": path, "left": f"len {len(left)}", "right": f"len {len(right)}"})
        else:
            for index, (a, b) in enumerate(zip(left, right)):
                differences(a, b, f"{path}/{index}", out)
    elif left != right:
        out.append({"path": path, "left": left, "right": right})
    return out


def main():
    left = strip(json.load(open(sys.argv[1])))
    right = strip(json.load(open(sys.argv[2])))
    # The plan and request hashes are shared; only the two documents' own
    # timings and identities were removed.
    diffs = differences(left, right)
    document = {
        "experiment": "round-envelope-kernel",
        "step": "the two battery binaries agree on every verdict",
        "left": sys.argv[1],
        "right": sys.argv[2],
        "strippedFields": sorted(TIMED | IDENTITY),
        "identical": not diffs,
        "differenceCount": len(diffs),
        "differences": diffs,
    }
    print(json.dumps(document, indent=1))
    return 0 if not diffs else 1


if __name__ == "__main__":
    sys.exit(main())
