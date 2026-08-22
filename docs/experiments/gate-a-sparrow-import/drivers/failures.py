#!/usr/bin/env python3
"""Gate A step 4: name the rows.

Grok review 6 asks for more than a boolean - "if it rejects, name WHICH pairs
and by how much". This writes `evidence/miter-failures.json`: every pair and
every placement the composite-miter envelope refuses, at each allowance, with
the material clearance it actually has, the critical radius the miter grid
credits it, and the shortfall - and with the Sparrow `item_id` alongside the
pose index so a row can be looked up in the upstream solution.
"""

import json
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
OUT = os.path.join(ROOT, "docs", "experiments", "gate-a-sparrow-import", "evidence")
VERDICTS = os.path.join(OUT, "verdicts.json")
FAILURES = os.path.join(OUT, "miter-failures.json")
SOLUTION = os.path.join(
    ROOT, "docs", "experiments", "sparrow-mixed61", "solution-10s-x86.json"
)


def main():
    document = json.load(open(VERDICTS))
    placed = json.load(open(SOLUTION))["solution"]["layout"]["placed_items"]
    item_id = [entry["item_id"] for entry in placed]

    rows = []
    for row in document["rows"]:
        censuses = {c["label"].split(" ")[0]: c for c in row["censuses"]}
        miter, round_ = censuses["composite-miter"], censuses["composite-round"]
        round_pairs = {tuple(p["placementIndices"]): p for p in round_["pairs"]}
        round_boundaries = {b["placementIndex"]: b for b in round_["boundaries"]}
        pairs = []
        for p in miter["pairs"]:
            if not p["envelopeOverlaps"]:
                continue
            key = tuple(p["placementIndices"])
            other = round_pairs.get(key)
            pairs.append(
                {
                    "poseIndices": list(key),
                    "sparrowItemIds": [item_id[key[0]], item_id[key[1]]],
                    "pieceIds": p["pieceIds"],
                    "materialClearanceMm": p["materialClearanceMm"],
                    "contractSurplusMm": p["materialClearanceMm"]
                    - row["contractPairClearanceMm"],
                    "miterCriticalRadiusMm": p["criticalRadiusMm"],
                    "miterCreditedClearanceMm": 2.0 * p["criticalRadiusMm"]
                    if p["criticalRadiusMm"] is not None
                    else None,
                    "miterJoinCostMm": p["joinCostMm"],
                    "miterRadiusShortfallMm": p["radiusSlackMm"],
                    "miterClearanceShortfallMm": p["clearanceSlackMm"],
                    "envelopeIntersectionAreaMm2": p["envelopeIntersectionAreaMm2"],
                    "roundAtSameRadiusOverlaps": other["envelopeOverlaps"]
                    if other
                    else None,
                    "causedBy": (
                        "radius"
                        if other is not None and other["envelopeOverlaps"]
                        else "join shape"
                    ),
                }
            )
        boundaries = []
        for b in miter["boundaries"]:
            if b["envelopeFits"]:
                continue
            other = round_boundaries.get(b["placementIndex"])
            boundaries.append(
                {
                    "poseIndex": b["placementIndex"],
                    "sparrowItemId": item_id[b["placementIndex"]],
                    "pieceId": b["pieceId"],
                    "materialClearanceMm": b["materialClearanceMm"],
                    "bindingMaterialClearanceMm": min(b["materialClearanceMm"]),
                    "contractSurplusMm": min(b["materialClearanceMm"])
                    - row["contractSheetClearanceMm"],
                    "miterCriticalRadiusMm": b["criticalRadiusMm"],
                    "miterRadiusShortfallMm": b["radiusSlackMm"],
                    "envelopeExcursionMm": b["envelopeExcursionMm"],
                    "roundAtSameRadiusFits": other["envelopeFits"] if other else None,
                    "causedBy": (
                        "radius"
                        if other is not None and not other["envelopeFits"]
                        else "join shape"
                    ),
                }
            )
        rows.append(
            {
                "searchOffsetAllowanceMm": row["searchOffsetAllowanceMm"],
                "envelopeRadiusMm": row["expansionMm"],
                "pairFailures": pairs,
                "boundaryFailures": boundaries,
            }
        )

    document = {
        "experiment": "gate-a-sparrow-import",
        "step": "4 - every row the composite-miter envelope refuses",
        "source": os.path.relpath(VERDICTS, ROOT),
        "note": (
            "`causedBy` is 'radius' when the round join at the SAME radius also "
            "refuses the row - the envelope is offset by total_padding/2 + "
            "margin + search_offset_allowance and that exceeds the contract's "
            "total_padding/2 - and 'join shape' when the round join accepts it, "
            "which is the miter representation and nothing else."
        ),
        "rows": rows,
    }
    with open(FAILURES, "w") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    for row in rows:
        print(
            f"allowance {row['searchOffsetAllowanceMm']}: "
            f"{len(row['pairFailures'])} pair failures, "
            f"{len(row['boundaryFailures'])} boundary failures"
        )


if __name__ == "__main__":
    main()
