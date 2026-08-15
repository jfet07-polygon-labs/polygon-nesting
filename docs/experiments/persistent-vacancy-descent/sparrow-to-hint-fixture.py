#!/usr/bin/env python3
"""Convert the committed Sparrow Mixed-61 solution into an engine hint fixture.

The Sparrow calibration (docs/experiments/sparrow-mixed61/) packs the same
converted geometry with 5.0 mm separation in a strip whose depth axis is
Sparrow x and whose width axis is Sparrow y. The engine frame uses the short
axis as x (width, 2000 mm) and the long axis as y (depth), so the rigid map

    (x_e, y_e) = (2000 - y_s, x_s)

is a +90-degree rotation plus translation (determinant +1, no mirroring), and
each placement maps to

    rotationDeg = sparrowRotation + 90 (normalized to [0, 360))
    translateShortAxis = 2000 - sparrowTranslation.y
    translateLongAxis  = sparrowTranslation.x

The Sparrow layout is NOT a valid layout under the engine contract: the
engine's publication validators require totalPadding + 2*sag = 5.5 mm pair
separation and 5.25 mm boundary clearance, while Sparrow enforced 5.0 mm.
The produced fixture therefore carries HINT placements only, consumed by the
persistent-vacancy seeded-reconstruction mode, which rebuilds the layout
piece by piece under the engine's own exact gates.
"""

import hashlib
import json
import os
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
REQUEST = os.path.join(REPO, "tests", "fixtures", "mixed-61", "mixed61-request.json")
INSTANCE = os.path.join(REPO, "docs", "experiments", "sparrow-mixed61", "input.json")
SOLUTION = os.path.join(REPO, "docs", "experiments", "sparrow-mixed61", "solution-3s.json")
WIDTH_MM = 2000.0


def sha256(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def main():
    stretch = 1.0
    argv = [a for a in sys.argv[1:] if not a.startswith("--stretch-y=")]
    for a in sys.argv[1:]:
        if a.startswith("--stretch-y="):
            stretch = float(a.split("=", 1)[1])
    sys.argv = [sys.argv[0]] + argv
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        REPO,
        "docs",
        "experiments",
        "persistent-vacancy-descent",
        "seeded-reconstruction",
        "sparrow-hints.json",
    )
    request = json.load(open(REQUEST))
    solution = json.load(open(SOLUTION))["solution"]
    piece_ids = [piece["id"] for piece in request["pieces"]]

    placements = []
    for placed in solution["layout"]["placed_items"]:
        transform = placed["transformation"]
        rotation = (transform["rotation"] + 90.0) % 360.0
        tx, ty = transform["translation"]
        placements.append(
            {
                "pieceId": piece_ids[placed["item_id"]],
                "rotationDeg": rotation,
                "mirrored": False,
                "translateShortAxis": WIDTH_MM - ty,
                "translateLongAxis": tx * stretch,
            }
        )
    if len(placements) != len(piece_ids) or len(
        {placement["pieceId"] for placement in placements}
    ) != len(piece_ids):
        raise SystemExit("solution does not place every piece exactly once")

    fixture = {
        "schemaVersion": 1,
        "description": (
            "HINT fixture mapped rigidly from the committed Sparrow Mixed-61 "
            "3-second calibration (154.44858 mm at 5.0 mm separation). These "
            "placements are NOT valid under the engine's 5.5 mm pair / "
            "5.25 mm boundary contract; they seed the persistent-vacancy "
            "guided reconstruction, whose output passes the engine's "
            "unchanged dual publication gates."
        ),
        "requestSha256": hashlib.sha256(open(REQUEST, "rb").read()).hexdigest(),
        "expectedPlacementFingerprint": "hint-only",
        "reportedDepthMm": solution["strip_width"],
        "independentDepthMm": solution["strip_width"],
        "provenance": {
            "depthStretchFactor": stretch,
            "sparrowInstance": os.path.relpath(INSTANCE, REPO),
            "sparrowInstanceSha256": sha256(INSTANCE),
            "sparrowSolution": os.path.relpath(SOLUTION, REPO),
            "sparrowSolutionSha256": sha256(SOLUTION),
            "mapping": "(x_e, y_e) = (2000 - y_s, x_s); rotation + 90 degrees; no mirror",
            "sparrowSeparationMm": 5.0,
            "engineContractSeparationMm": 5.5,
        },
        "placements": placements,
    }
    with open(output, "w") as handle:
        json.dump(fixture, handle, indent=2)
        handle.write("\n")
    print(output, sha256(output))


if __name__ == "__main__":
    main()
