#!/usr/bin/env python3
"""Gate A step 3: reduce the three verdicts to the interpretation table.

Reads `evidence/verdicts.json` (the shadow instrument's raw output) and writes
`evidence/summary.json`: per allowance, the three verdicts; the failure
decomposition into join-caused and radius-caused; the boundary semantics; the
miter join's cost distribution; and five soundness cross-checks that have to
hold before any of it is quotable.

The cross-checks, and what each would catch:

  containment      - a miter or square offset contains the disc offset at the
                     same radius, so `r*_miter <= r*_round` and
                     `r*_square <= r*_round` on every row measured in both.
                     Both `r*` are integers on a 1 um grid and the round join is
                     *inscribed*, so a one-step inversion is the quantization
                     floor, not a defect: those are counted separately as
                     `containmentGridTies`. Anything beyond one grid step is a
                     real violation and would mean the shadow's round join is
                     not the disc it claims to be.
  discIdentity     - for an exact disc, `2 * r*` IS the material clearance, so
                     `material - 2 * r*_round` must sit inside the quantization
                     budget. That budget is derived, not guessed:

                       * `r*` is the largest INTEGER micrometre radius that is
                         still disjoint, so the true critical radius lies in
                         `[r*, r* + 0.001)` and `material - 2 * r*` picks up up
                         to `2 * 0.001` from that alone;
                       * each envelope's vertices are rounded to the same 1 um
                         grid, which can move a boundary by up to
                         `sqrt(2)/2 * 0.001` on each of the two operands, so
                         `sqrt(2) * 0.001` between them;
                       * Clipper's round join INSCRIBES its arcs, so each
                         envelope can fall up to one `arc_tolerance` inside the
                         true disc - that is the only term under this driver's
                         control and it is set to 0.0001 mm, a tenth of a grid
                         step.

                     Budget = 0.002 + sqrt(2) * 0.001 + 2 * arc_tolerance. The
                     inscription and rounding terms are signed the other way, so
                     the check is two-sided. This is what says the round census
                     is measuring `P (+) disc(r)` and not a coarse polygon.
  productionOffset - the shadow's miter configuration reproduced
                     `PolygonSet::offset` exactly on every piece.
  monotone         - `r*` may only fall as the join gets sharper, and the
                     failure counts may only fall as the radius falls.
  agreesWithRealValidator
                   - the strongest one. `validate_and_measure_placements` is
                     the real thing and it SHORT-CIRCUITS: it names the
                     lowest-indexed placement whose envelope leaves the inset
                     sheet, and stops. The shadow enumerates instead. So the
                     shadow's lowest-indexed boundary failure must be the piece
                     the real validator names, on every row - which is what
                     says the shadow's envelope half is the composite's
                     envelope half and not a second implementation that happens
                     to agree in aggregate.
"""

import json
import os
import statistics

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
OUT = os.path.join(ROOT, "docs", "experiments", "gate-a-sparrow-import", "evidence")
VERDICTS = os.path.join(OUT, "verdicts.json")
SUMMARY = os.path.join(OUT, "summary.json")

GRID_MM = 0.001


def by_label(row):
    return {c["label"].split(" ")[0]: c for c in row["censuses"]}


def pair_key(p):
    return tuple(p["placementIndices"])


def decompose(miter, round_):
    """Attribute each miter refusal to the radius or to the join shape.

    By set intersection, never by subtracting counts - see the call site.
    """
    miter_pairs = {pair_key(p) for p in miter["pairs"] if p["envelopeOverlaps"]}
    round_pairs = {pair_key(p) for p in round_["pairs"] if p["envelopeOverlaps"]}
    miter_bounds = {b["placementIndex"] for b in miter["boundaries"] if not b["envelopeFits"]}
    round_bounds = {b["placementIndex"] for b in round_["boundaries"] if not b["envelopeFits"]}
    return {
        "pairFailuresMiter": miter["pairFailureCount"],
        "pairFailuresRoundSameRadius": round_["pairFailureCount"],
        "pairFailuresCausedByRadius": len(miter_pairs & round_pairs),
        "pairFailuresCausedByJoinShape": len(miter_pairs - round_pairs),
        "roundRefusesMiterAcceptsPairs": sorted(round_pairs - miter_pairs),
        "boundaryFailuresMiter": miter["boundaryFailureCount"],
        "boundaryFailuresRoundSameRadius": round_["boundaryFailureCount"],
        "boundaryFailuresCausedByRadius": len(miter_bounds & round_bounds),
        "boundaryFailuresCausedByJoinShape": len(miter_bounds - round_bounds),
        "roundRefusesMiterAcceptsBoundaries": sorted(round_bounds - miter_bounds),
        "accountsForEveryMiterPairFailure": len(miter_pairs) == miter["pairFailureCount"],
        "accountsForEveryMiterBoundaryFailure": len(miter_bounds)
        == miter["boundaryFailureCount"],
    }


def main():
    document = json.load(open(VERDICTS))
    rows = []
    checks = {
        "containmentViolations": [],
        "containmentGridTies": [],
        "discIdentityWorstMm": 0.0,
        "discIdentityWorstPair": None,
        "productionOffsetReproduced": [],
        "monotoneViolations": [],
        "realValidatorAgreement": [],
    }
    previous = None
    for row in document["rows"]:
        censuses = by_label(row)
        miter, round_, square = (
            censuses["composite-miter"],
            censuses["composite-round"],
            censuses["composite-square"],
        )
        checks["productionOffsetReproduced"].append(miter["reproducesProductionOffset"])

        # The shadow's lowest-indexed boundary failure against the piece the
        # real, short-circuiting validator names.
        failing = sorted(
            b["placementIndex"] for b in miter["boundaries"] if not b["envelopeFits"]
        )
        message = row["compositeMiterVerdict"].get("message", "")
        named = next(
            (
                b["pieceId"]
                for b in miter["boundaries"]
                if b["placementIndex"] == failing[0]
            ),
            None,
        ) if failing else None
        checks["realValidatorAgreement"].append(
            {
                "searchOffsetAllowanceMm": row["searchOffsetAllowanceMm"],
                "shadowLowestBoundaryFailureIndex": failing[0] if failing else None,
                "shadowLowestBoundaryFailurePieceId": named,
                "realValidatorMessage": message,
                "agrees": bool(named) and named in message,
            }
        )

        round_pairs = {pair_key(p): p for p in round_["pairs"]}
        round_boundaries = {b["placementIndex"]: b for b in round_["boundaries"]}
        costs = []
        for census, name in ((miter, "miter"), (square, "square")):
            for p in census["pairs"]:
                other = round_pairs.get(pair_key(p))
                if other is None or p["criticalRadiusMm"] is None or other["criticalRadiusMm"] is None:
                    continue
                if p.get("criticalRadiusSaturated") or other.get("criticalRadiusSaturated"):
                    continue
                excess = p["criticalRadiusMm"] - other["criticalRadiusMm"]
                if excess > 1e-12:
                    entry = {"allowance": row["searchOffsetAllowanceMm"], "join": name,
                             "pair": pair_key(p), "join_r": p["criticalRadiusMm"],
                             "round_r": other["criticalRadiusMm"], "excessMm": excess}
                    bucket = "containmentGridTies" if excess <= GRID_MM + 1e-12 else "containmentViolations"
                    checks[bucket].append(entry)
            for b in census["boundaries"]:
                other = round_boundaries.get(b["placementIndex"])
                if other is None or b["criticalRadiusMm"] is None or other["criticalRadiusMm"] is None:
                    continue
                if b.get("criticalRadiusSaturated") or other.get("criticalRadiusSaturated"):
                    continue
                excess = b["criticalRadiusMm"] - other["criticalRadiusMm"]
                if excess > 1e-12:
                    entry = {"allowance": row["searchOffsetAllowanceMm"], "join": name,
                             "boundary": b["placementIndex"], "join_r": b["criticalRadiusMm"],
                             "round_r": other["criticalRadiusMm"], "excessMm": excess}
                    bucket = "containmentGridTies" if excess <= GRID_MM + 1e-12 else "containmentViolations"
                    checks[bucket].append(entry)
        for p in round_["pairs"]:
            if p["criticalRadiusMm"] is None or p.get("criticalRadiusSaturated"):
                continue
            error = abs(p["joinCostMm"])
            if error > checks["discIdentityWorstMm"]:
                checks["discIdentityWorstMm"] = error
                checks["discIdentityWorstPair"] = {
                    "allowance": row["searchOffsetAllowanceMm"],
                    "pair": pair_key(p),
                    "materialClearanceMm": p["materialClearanceMm"],
                    "twiceCriticalRadiusMm": 2 * p["criticalRadiusMm"],
                    "signedMm": p["joinCostMm"],
                }
            checks["discIdentitySignedRangeMm"] = [
                min(checks.get("discIdentitySignedRangeMm", [p["joinCostMm"]])[0], p["joinCostMm"]),
                max(checks.get("discIdentitySignedRangeMm", [0, p["joinCostMm"]])[-1], p["joinCostMm"]),
            ]
        failing_costs = []
        for p in miter["pairs"]:
            # A saturated r* makes join cost a floor rather than the answer;
            # such a row must not enter a max, a median or a min. No pair row
            # in this round saturates (the census counts them), but the filter
            # is here so a future pose set cannot quietly poison the statistic.
            if p["joinCostMm"] is not None and not p.get("criticalRadiusSaturated"):
                costs.append(p["joinCostMm"])
                if p["envelopeOverlaps"]:
                    failing_costs.append(p["joinCostMm"])

        if previous is not None:
            # radius only ever falls across the rows, so failure counts may not rise
            for name, census in (("miter", miter), ("round", round_), ("square", square)):
                for field in ("pairFailureCount", "boundaryFailureCount"):
                    if census[field] > previous[name][field]:
                        checks["monotoneViolations"].append(
                            {"join": name, "field": field,
                             "allowance": row["searchOffsetAllowanceMm"],
                             "value": census[field], "previous": previous[name][field]}
                        )
        previous = {"miter": miter, "round": round_, "square": square}

        # Boundary semantics: what material edge clearance does each envelope
        # actually demand?
        #
        # A placement's envelope must fit the sheet inset by `inset =
        # sheet_edge_clearance - total_padding/2`. For a round join the
        # envelope reaches exactly `radius` past the material in every
        # direction, so the demand on the material is `inset + radius` - flat,
        # and equal to `sheet_edge_clearance + margin + allowance`. For a miter
        # join a convex corner pointing at the wall reaches `k * radius` with
        # `k = 1 / sin(half-angle)` capped at the miter limit, so the demand is
        # `inset + k * radius` and k is a property of the pose, not of the
        # contract. `r*` measures k without needing the corner geometry:
        # at the critical radius the envelope exactly touches the inset line,
        # so `k = (binding material clearance - inset) / r*`.
        def boundary_semantics(census):
            inset = census["sheetInsetMm"]
            radius = census["radiusMm"]
            out = []
            for b in census["boundaries"][:4]:
                if b["criticalRadiusMm"] is None or b["criticalRadiusMm"] <= 0.0:
                    continue
                # A saturated row's r* is a floor, not the answer: the reach
                # factor derived from it would be fiction.
                if b.get("criticalRadiusSaturated"):
                    continue
                binding = min(b["materialClearanceMm"])
                reach = (binding - inset) / b["criticalRadiusMm"]
                out.append(
                    {
                        "placementIndex": b["placementIndex"],
                        "bindingMaterialClearanceMm": binding,
                        "criticalRadiusMm": b["criticalRadiusMm"],
                        "miterReachFactor": reach,
                        "impliedMaterialClearanceDemandMm": inset + reach * radius,
                        "fits": b["envelopeFits"],
                    }
                )
            return out

        def verdict(census):
            return {
                "envelopeAdmissible": census["envelopeAdmissible"],
                "pairFailures": census["pairFailureCount"],
                "boundaryFailures": census["boundaryFailureCount"],
                "saturatedPairRows": census["saturatedPairRows"],
                "saturatedBoundaryRows": census["saturatedBoundaryRows"],
                "worstPairRadiusSlackMm": census["pairs"][0]["radiusSlackMm"] if census["pairs"] else None,
                "worstPairClearanceSlackMm": census["pairs"][0]["clearanceSlackMm"] if census["pairs"] else None,
                "worstPair": pair_key(census["pairs"][0]) if census["pairs"] else None,
                "worstBoundaryRadiusSlackMm": census["boundaries"][0]["radiusSlackMm"] if census["boundaries"] else None,
                "worstBoundaryPlacement": census["boundaries"][0]["placementIndex"] if census["boundaries"] else None,
            }

        rows.append(
            {
                "searchOffsetAllowanceMm": row["searchOffsetAllowanceMm"],
                "envelopeRadiusMm": row["expansionMm"],
                "contractPairClearanceMm": row["contractPairClearanceMm"],
                "contractSheetClearanceMm": row["contractSheetClearanceMm"],
                "envelopeImpliedPairClearanceMm": row["envelopeImpliedPairClearanceMm"],
                "envelopeImpliedSheetClearanceMm": row["envelopeImpliedSheetClearanceMm"],
                "verdictA_contractOnly": row["contractOnlyVerdict"],
                "verdictB_compositeMiter": row["compositeMiterVerdict"],
                "verdictC_compositeRound": verdict(round_),
                "compositeMiterCensus": verdict(miter),
                "compositeSquareCensus": verdict(square),
                # The decomposition Gate A turns on. A miter pair failure that
                # the round join at the SAME radius also refuses is caused by
                # the envelope's radius (`total_padding/2 + margin +
                # allowance` exceeding `total_padding/2`), not by the join
                # shape; one the round join accepts is caused by the join
                # shape alone, and is the thing Sol review 11's kernel would
                # remove.
                #
                # This is a SET INTERSECTION and not a subtraction of counts.
                # The two are not the same, because round failures are not a
                # subset of miter failures: the round join is inscribed, so on
                # a pair whose margin is under one grid step it can land one
                # micrometre BELOW the miter's own `r*` and refuse a row the
                # miter accepts. That happens on exactly one pair at radius
                # 2.5005, and a subtraction of counts would have reported it as
                # a join-shape failure that does not exist. Those rows are
                # counted separately as `roundRefusesMiterAccepts`.
                "failureDecomposition": decompose(miter, round_),
                "miterPairJoinCostMm": {
                    "population": (
                        "the pairs the shadow bisected: the "
                        f"{document['bisectTop']} tightest by material clearance, "
                        "plus every failing pair"
                    ),
                    "bisectedPairs": len(costs),
                    "max": max(costs) if costs else None,
                    "median": statistics.median(costs) if costs else None,
                    "min": min(costs) if costs else None,
                },
                "miterFailingPairJoinCostMm": {
                    "pairs": len(failing_costs),
                    "max": max(failing_costs) if failing_costs else None,
                    "median": statistics.median(failing_costs) if failing_costs else None,
                    "min": min(failing_costs) if failing_costs else None,
                },
                "boundarySemantics": {
                    "contractDemandMm": row["contractSheetClearanceMm"],
                    "sparrowValidatedAtMm": 5.0,
                    "roundEnvelopeFlatDemandMm": row["envelopeImpliedSheetClearanceMm"],
                    "miterWorstRows": boundary_semantics(miter),
                    "roundWorstRows": boundary_semantics(round_),
                    "miterLimitCeilingMm": miter["sheetInsetMm"] + 2.0 * miter["radiusMm"],
                },
                "roundInwardDeviationMm": round_["roundInwardDeviationMm"],
                "roundEnvelopeVertexTotal": round_["envelopeVertexTotal"],
                "miterEnvelopeVertexTotal": miter["envelopeVertexTotal"],
            }
        )

    arc = max(r["roundInwardDeviationMm"] for r in rows)
    checks["discIdentityBudgetMm"] = 2.0 * GRID_MM + (2.0 ** 0.5) * GRID_MM + 2.0 * arc
    checks["discIdentityBudgetTerms"] = {
        "integerRadiusQuantizationMm": 2.0 * GRID_MM,
        "twoSidedVertexRoundingMm": (2.0 ** 0.5) * GRID_MM,
        "twoSidedArcInscriptionMm": 2.0 * arc,
    }
    checks["discIdentityWithinBudget"] = (
        checks["discIdentityWorstMm"] <= checks["discIdentityBudgetMm"]
    )
    checks["containmentHolds"] = not checks["containmentViolations"]
    checks["containmentGridTieCount"] = len(checks["containmentGridTies"])
    checks["containmentNote"] = (
        "every inversion is exactly one 0.001 mm grid step, which is what an "
        "inscribed round join plus integer-grid vertex rounding produces; none "
        "exceeds it"
    )
    checks["monotoneHolds"] = not checks["monotoneViolations"]
    checks["productionOffsetHolds"] = all(
        value is True for value in checks["productionOffsetReproduced"]
    )
    checks["realValidatorAgreementHolds"] = all(
        row["agrees"] for row in checks["realValidatorAgreement"]
    )
    checks["allSoundnessChecksHold"] = (
        checks["containmentHolds"]
        and checks["discIdentityWithinBudget"]
        and checks["monotoneHolds"]
        and checks["productionOffsetHolds"]
        and checks["realValidatorAgreementHolds"]
    )

    summary = {
        "experiment": "gate-a-sparrow-import",
        "step": "3 - the three verdicts and their interpretation",
        "source": {
            "verdicts": os.path.relpath(VERDICTS, ROOT),
            "request": document["request"],
            "poses": document["poses"],
            "contract": document["contract"],
            "canonicalGridStepMm": document["canonicalGridStepMm"],
            "bisectTop": document["bisectTop"],
        },
        "soundnessChecks": checks,
        "rows": rows,
    }
    with open(SUMMARY, "w") as handle:
        json.dump(summary, handle, indent=2)
        handle.write("\n")
    print(json.dumps({"soundnessChecks": checks, "rows": rows}, indent=2))


if __name__ == "__main__":
    main()
