#!/usr/bin/env python3
"""The battery's four verdicts, read off the raw document.

    summarize.py RAW.json OUT.json [FCV_RAW.json]

Every number here is a reduction of `battery.json`; nothing is recomputed from
geometry, so this file cannot disagree with the instrument. Where it applies a
threshold — the false-accept test, the flip-point agreement — the threshold is
named in the output next to the count it produced.
"""

import collections
import json
import sys


def summarize(raw, fcv=None):
    p1 = raw["population1CanonicalCorpus"]
    by_allowance = collections.OrderedDict()
    attributed = []
    for row in p1["rows"]:
        key = repr(row["searchOffsetAllowanceMm"])
        cell = by_allowance.setdefault(
            key,
            {
                "searchOffsetAllowanceMm": row["searchOffsetAllowanceMm"],
                "layouts": 0,
                "miterAccepts": 0,
                "kernelAccepts": 0,
                "unionAccepts": 0,
                "layoutsMiterAcceptsKernelRefuses": 0,
                "layoutsMiterAcceptsUnionRefuses": 0,
                "layoutsKernelAcceptsMiterRefuses": 0,
                "pairRowsKernelAdmitsMiterRefuses": 0,
                "pairRowsMiterAdmitsKernelRefuses": 0,
                "boundaryRowsKernelAdmitsMiterRefuses": 0,
                "boundaryRowsMiterAdmitsKernelRefuses": 0,
            },
        )
        cell["layouts"] += 1
        miter = row["compositeMiterVerdict"]["accepted"]
        kernel = row["compositeRoundVerdict"]["accepted"]
        union = row["compositeUnionVerdict"]["accepted"]
        cell["miterAccepts"] += bool(miter)
        cell["kernelAccepts"] += bool(kernel)
        cell["unionAccepts"] += bool(union)
        cell["layoutsMiterAcceptsKernelRefuses"] += bool(miter and not kernel)
        cell["layoutsMiterAcceptsUnionRefuses"] += bool(miter and not union)
        cell["layoutsKernelAcceptsMiterRefuses"] += bool(kernel and not miter)
        cell["pairRowsKernelAdmitsMiterRefuses"] += row["pairs"]["kernelAdmitsMiterRefuses"]
        cell["pairRowsMiterAdmitsKernelRefuses"] += row["pairs"]["miterAdmitsKernelRefuses"]
        cell["boundaryRowsKernelAdmitsMiterRefuses"] += row["boundaries"][
            "kernelAdmitsMiterRefuses"
        ]
        cell["boundaryRowsMiterAdmitsKernelRefuses"] += row["boundaries"][
            "miterAdmitsKernelRefuses"
        ]
        for item in row["miterAdmitsKernelRefusesAttributed"]:
            attributed.append(
                {
                    "layout": row["label"],
                    "searchOffsetAllowanceMm": row["searchOffsetAllowanceMm"],
                    "placementIndices": item["placementIndices"],
                    "shortfallMicron": item["shortfallMicron"],
                    "miterEnvelopeIntersectionAreaMm2": item[
                        "miterEnvelopeIntersectionAreaMm2"
                    ],
                    "materialClearanceMm": item["materialClearanceMm"],
                    "demandedTwoRMm": 2.0 * row["kernelCensus"]["expansionMm"],
                    "materialBelowDemanded": (
                        item["materialClearanceMm"] is not None
                        and item["materialClearanceMm"]
                        < 2.0 * row["kernelCensus"]["expansionMm"]
                    ),
                }
            )

    shortfalls = sorted({round(item["shortfallMicron"], 6) for item in attributed})
    areas = sorted({item["miterEnvelopeIntersectionAreaMm2"] for item in attributed})

    p2 = raw["population2MaterialValidCanonicalInvalid"]
    p3 = raw["population3SparrowDifferential"]
    sparrow = []
    expectations = []
    for row in p3["rows"]:
        allowance = row["searchOffsetAllowanceMm"]
        sparrow.append(
            {
                "searchOffsetAllowanceMm": allowance,
                "expansionMm": row["expansionMm"],
                "radiusMicron": row["radiusMicron"],
                "contractOnlyAccepts": row["contractOnlyAccepts"],
                "compositeMiterAccepts": row["compositeMiterVerdict"]["accepted"],
                "compositeRoundAccepts": row["compositeRoundVerdict"]["accepted"],
                "compositeUnionAccepts": row["compositeUnionVerdict"]["accepted"],
                "kernelPairFailureCount": row["kernelPairFailureCount"],
                "kernelBoundaryFailureCount": row["kernelBoundaryFailureCount"],
                "kernelRefusedPairIndices": row["kernelRefusedPairIndices"],
                "pair38x39": row["pair38x39"],
                "pair50x52": row["pair50x52"],
            }
        )
        if allowance == 0.0:
            expectations.append(
                {
                    "expectation": "at r=2.500 the kernel accepts all 1830 pairs and all 61 boundaries",
                    "met": row["kernelPairFailureCount"] == 0
                    and row["kernelBoundaryFailureCount"] == 0
                    and row["compositeRoundVerdict"]["accepted"],
                }
            )
            expectations.append(
                {
                    "expectation": "pair 38x39 (pose indices 0,1) is ACCEPTED at r=2.500",
                    "met": bool(row["pair38x39"] and row["pair38x39"]["admissible"]),
                }
            )
        if allowance == 0.002:
            expectations.append(
                {
                    "expectation": "at r=2.502 the kernel refuses exactly the two "
                    "radius-caused pairs and no boundary",
                    "met": row["kernelRefusedPairIndices"] == [[0, 1], [42, 44]]
                    and row["kernelBoundaryFailureCount"] == 0,
                }
            )
            expectations.append(
                {
                    "expectation": "the contract accepts the Sparrow pose set at every radius",
                    "met": row["contractOnlyAccepts"],
                }
            )

    p4 = raw["population4Sweeps"]
    flips = collections.Counter(
        row["kernelMinusMaterialFlipSteps"] for row in p4["rows"]
    )
    sweeps = {
        "sweepCount": p4["sweepCount"],
        "stepMm": p4["stepMm"],
        "stepsEachWay": p4["stepsEachWay"],
        "monotoneInMaterial": sum(
            1 for row in p4["rows"] if row["kernelMonotoneInMaterial"]
        ),
        "monotoneInStep": sum(1 for row in p4["rows"] if row["kernelMonotone"]),
        "stepsDisagreeingOutsideCanonicalizationBudget": sum(
            row["stepsDisagreeingOutsideBudget"] for row in p4["rows"]
        ),
        "kernelMinusMaterialFlipSteps": {
            str(key): value for key, value in sorted(flips.items(), key=lambda x: (x[0] is None, x[0]))
        },
        "worstKernelVersusMaterialMm": max(
            (row["worstKernelVersusMaterialMm"] or 0.0) for row in p4["rows"]
        ),
        "sweepFloorBudgetMm": p4["rows"][0]["sweepFloorBudgetMm"],
        "outsideSweepFloorBudget": sum(
            1 for row in p4["rows"] if row["insideSweepFloorBudget"] is False
        ),
    }

    economy = {
        "envelopeHalfRatioMedian": raw["economy"]["envelopeHalfRatioMedian"],
        "envelopeHalfRatioMax": max(
            row["envelopeHalfRatio"] for row in raw["economy"]["rows"]
        ),
        "confirmationRatioMedianComparable": raw["economy"][
            "confirmationRatioMedianComparable"
        ],
        "confirmationUnionRatioMedianComparable": raw["economy"][
            "confirmationUnionRatioMedianComparable"
        ],
        "comparableCells": sum(
            1 for row in raw["economy"]["rows"] if row["confirmationComparable"]
        ),
        "cells": len(raw["economy"]["rows"]),
        "rows": raw["economy"]["rows"],
    }
    if fcv is not None:
        economy["withFastContractValidator"] = {
            "envelopeHalfRatioMedian": fcv["economy"]["envelopeHalfRatioMedian"],
            "confirmationRatioMedianComparable": fcv["economy"][
                "confirmationRatioMedianComparable"
            ],
            "comparableCells": sum(
                1 for row in fcv["economy"]["rows"] if row["confirmationComparable"]
            ),
            "rows": fcv["economy"]["rows"],
        }

    document = {
        "experiment": "round-envelope-kernel",
        "source": "docs/experiments/round-envelope-kernel/evidence/battery.json",
        "canonicalGridStepMm": raw["canonicalGridStepMm"],
        "canonicalizationBudgetMm": raw["canonicalizationBudgetMm"],
        "population1": {
            "layouts": p1["layoutCount"],
            "cells": p1["cellCount"],
            "pairRowsCompared": p1["pairRowsCompared"],
            "boundaryRowsCompared": p1["boundaryRowsCompared"],
            "byAllowance": list(by_allowance.values()),
            "p0RowCount": p1["p0RowCount"],
            "p0Cells": p1["p0Layouts"],
            "p0DistinctShortfallsMicron": shortfalls,
            "p0DistinctMiterIntersectionAreasMm2": areas,
            "p0RowsWithMaterialBelowDemanded": sum(
                1 for item in attributed if item["materialBelowDemanded"]
            ),
            "p0Attributed": attributed,
        },
        "population2": {
            "proposalCount": p2["proposalCount"],
            "kernelAcceptCount": p2["kernelAcceptCount"],
            "falseAcceptCount": p2["falseAcceptCount"],
            "insideCanonicalizationBudgetCount": p2[
                "insideCanonicalizationBudgetCount"
            ],
            "insideCanonicalizationBudget": p2["insideCanonicalizationBudget"],
        },
        "population3": {"rows": sparrow, "expectations": expectations},
        "population4": sweeps,
        "economy": economy,
        "verdicts": {
            "zeroFalseAccepts": p2["falseAcceptCount"] == 0,
            "everySparrowExpectationMet": all(item["met"] for item in expectations),
            "everySweepMonotoneInMaterial": sweeps["monotoneInMaterial"]
            == sweeps["sweepCount"],
            "everySweepFlipWithinOneGridStep": all(
                key in {"-1", "0", "1"}
                for key in sweeps["kernelMinusMaterialFlipSteps"]
            ),
            "canonicalCorpusRegressionRows": p1["p0RowCount"],
            "unionLosesNoCanonicalValidLayout": all(
                cell["layoutsMiterAcceptsUnionRefuses"] == 0
                for cell in by_allowance.values()
            ),
            "canonicalCorpusRegressionsAreAllOneGridStepConservative": (
                shortfalls == [-1.0] if shortfalls else True
            ),
            "envelopeHalfWithinBudget": economy["envelopeHalfRatioMax"] <= 1.25,
            "confirmationWithinBudget": (
                economy["confirmationRatioMedianComparable"] is not None
                and economy["confirmationRatioMedianComparable"] <= 1.25
            ),
            "unionConfirmationWithinBudget": (
                economy["confirmationUnionRatioMedianComparable"] is not None
                and economy["confirmationUnionRatioMedianComparable"] <= 1.25
            ),
        },
    }
    return document


def main():
    raw = json.load(open(sys.argv[1]))
    fcv = json.load(open(sys.argv[3])) if len(sys.argv) > 3 else None
    document = summarize(raw, fcv)
    with open(sys.argv[2], "w") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    trimmed = json.loads(json.dumps(document))
    trimmed["population1"].pop("p0Attributed")
    trimmed["economy"].pop("rows")
    trimmed["economy"].pop("withFastContractValidator", None)
    print(json.dumps(trimmed, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
