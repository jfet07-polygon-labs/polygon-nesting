#!/usr/bin/env python3
"""Writes depth-lower-bound-exact-clearance-evidence.json from the re-pin run.

Runs `mixed61-lower-bound-exact-clearance.py` in-process (it returns its own
numbers dict) and wraps them with the derivation, the supersession record and
the identity check against the retired file, so the evidence document is a
product of the script rather than a transcription of its stdout.
"""

import hashlib
import importlib.util
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
SCRIPT = os.path.join(HERE, "mixed61-lower-bound-exact-clearance.py")
LEGACY = os.path.join(HERE, "depth-lower-bound-evidence.json")
OUT = os.path.join(HERE, "depth-lower-bound-exact-clearance-evidence.json")


def sha256(relative):
    with open(os.path.join(ROOT, relative), "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def main():
    spec = importlib.util.spec_from_file_location("repin", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    numbers = module.main()
    legacy = json.load(open(LEGACY))["numbers"]
    legacy_sum_2p5 = legacy["sparrow_bound_mm"] * 2000.0

    script_relative = os.path.relpath(SCRIPT, ROOT)
    fixture_relative = "tests/fixtures/mixed-61/mixed61-request-exact-clearance.json"
    document = {
        "experiment": (
            "contract-native area lower bound for Mixed-61 strip depth, "
            "RE-PINNED for the exact-clearance contract"
        ),
        "supersedes": {
            "file": "docs/experiments/depth-lower-bound/depth-lower-bound-evidence.json",
            "figure": "contract_bound_strengthened_mm = 131.97838540260466",
            "why": (
                "that construction inflates at r = 2.75 and adds 4.75 because the "
                "contract then was 5.5 mm pair / 5.25 mm boundary. This branch's "
                "contract is exact-clearance 5.0/5.0 "
                "(tests/fixtures/mixed-61/mixed61-request-exact-clearance.json, "
                "geometry sag 0.0 and clearance safety margin 0.0; every pinned "
                "gate runs positional sheet-edge-clearance 5 and pair-clearance 5), "
                "so r = 2.5 and the depth term is 5.0. Flagged by Kimi review 1, "
                "docs/kimi-review-1-the-band-audition.md:49."
            ),
            "whyTheOldFilesSparrowFigureIsNotTheReplacement": (
                "depth-lower-bound-evidence.json's sparrow_bound_mm = "
                "124.88690242765017 is SUM_2.5 / 2000: the r = 2.5 inflated area "
                "over the FULL 2000 mm width, with no boundary term and no "
                "depth-metric term. It was a calibration of an outside packer "
                "under an assumption about what Sparrow counts, not this engine's "
                "bound. The current figures divide by the usable width 1995 and "
                "add the boundary and depth terms."
            ),
            "identityCheckAgainstTheRetiredFile": {
                "retiredSparrowBoundTimesStripWidthMm2": legacy_sum_2p5,
                "repinnedInflatedSumR2p5Mm2": numbers["inflated_sum_r2_500_mm2"],
                "differenceMm2": numbers["inflated_sum_r2_500_mm2"] - legacy_sum_2p5,
                "meaning": (
                    "the same certified r = 2.5 inflated area to within one ulp, "
                    "which is what says the geometry code is unchanged and only "
                    "the contract constants moved"
                ),
            },
        },
        "question": (
            "what depth is provably unreachable under THIS branch's exact-clearance "
            "contract (5.0 mm pair separation, 5.0 mm boundary clearance, 2000 mm "
            "width), and what does the composite envelope add to it?"
        ),
        "method": (
            "the committed construction of mixed61-lower-bound.py with three "
            "constants changed and the depth term re-derived; the geometry - "
            "shoelace raw areas, exact Steiner formula for the convex pieces, "
            "certified 0.02 mm grid LOWER bound for the 9 non-convex stars - is "
            "unchanged"
        ),
        "script": script_relative,
        "scriptSha256": sha256(script_relative),
        "fixture": fixture_relative,
        "fixtureSha256": sha256(fixture_relative),
        "contract": {
            "pairClearanceMm": 5.0,
            "sheetClearanceMm": 5.0,
            "pairClearanceDerivation": (
                "total_padding + 2 * flattening_sag = 5.0 + 0.0 "
                "(validation/general_polygon.rs, validate_publication_inner)"
            ),
            "sheetClearanceDerivation": (
                "sheet_edge_clearance + flattening_sag = 5.0 + 0.0, on all four "
                "sheet edges (validate_sheet)"
            ),
            "depthConvention": (
                "D = max placed source y + sheet_edge_clearance(5.0) "
                "(raw_source_long_axis_depth_mm)"
            ),
            "compositeEnvelopeRadiusMm": 2.502,
            "compositeEnvelopeDerivation": (
                "total_padding/2 + clearance_safety_margin + search_offset_allowance "
                "= 2.5 + 0.0 + 0.002 (collision_expansion_mm, from-request "
                "allowance). A miter offset contains the disc offset at the same "
                "radius, so anything the composite publishes has material pair "
                "separation >= 5.004 and material boundary clearance >= "
                "inset + expansion = 5.002."
            ),
        },
        "derivation": [
            "r = 5.0/2 = 2.5. Inflate every placed piece by disc(r). Pair "
            "separation >= 2r, so the inflated interiors are pairwise disjoint.",
            "Material x in [5.0, 1995.0], so inflated x in [2.5, 1997.5]: usable "
            "width 2000 - 2*(5.0 - 2.5) = 1995.",
            "If the raw pieces span y-extent E, the inflated pieces lie in a band "
            "of height E + 2r = E + 5.0, so SUM_2.5 <= 1995 * (E + 5.0), i.e. "
            "E >= SUM_2.5/1995 - 5.0.",
            "D = y_max + 5.0 = E + y_min + 5.0 and y_min >= 5.0, so D >= E + 10.0 "
            ">= SUM_2.5/1995 + 5.0  [strengthened].",
            "Dropping the depth-metric argument: D >= SUM_2.5/1995  [plain].",
            "Composite variant: r = 2.502 and b = inset + r = 5.002, so b - r = 2.5 "
            "and the usable width is 1995 again; D >= SUM_2.502/1995 + "
            "(b + 5.0 - 2r) = SUM_2.502/1995 + 4.998.",
        ],
        "numbers": numbers,
        "keyFindings": [
            "RE-PINNED contract-native lower bound: depth >= "
            f"{numbers['contract_bound_strengthened_mm']} mm (plain, without the "
            f"depth-metric argument: {numbers['contract_bound_plain_mm']} mm).",
            "Under the CURRENT acceptance authority the bound is "
            f"{numbers['composite_bound_strengthened_mm']} mm: the composite "
            "envelope's 0.002 mm allowance buys "
            f"{numbers['composite_bound_strengthened_mm'] - numbers['contract_bound_strengthened_mm']:.4f} mm "
            "of lower bound, which is the whole of what the envelope adds at the "
            "bound level.",
            "The retired 131.97838540260466 mm figure is "
            f"{131.97838540260466 - numbers['contract_bound_strengthened_mm']:.4f} mm "
            "too high for this contract. Anything quoting it as the invariant is "
            "quoting the 5.5/5.25 era.",
            "The 7.09 mm of 'contract overhead' the retired file attributed to the "
            "engine-vs-Sparrow gap is GONE: Sparrow's 5.0 mm separation and this "
            "branch's contract are the same number, so there is one bound and not "
            "two. At the bound level the residual asymmetry is the "
            f"{numbers['composite_bound_strengthened_mm'] - numbers['contract_bound_strengthened_mm']:.4f} mm "
            "the search allowance adds.",
            "Neither Sparrow's 150.16451 mm (this engine's depth convention on the "
            "imported pose set) nor the campaign record 155.264 mm is excluded by "
            "area: 150.16451 sits "
            f"{150.16451 - numbers['contract_bound_strengthened_mm']:.3f} mm above "
            "the re-pinned bound. The area bound has never been the binding "
            "constraint on this instance and still is not.",
        ],
        "reference": {
            "sparrowTenSecondX86ReportedStripWidthMm": 150.16547,
            "sparrowTenSecondX86EngineConventionDepthMm": 150.16451,
            "campaignRecordMm": 155.264,
            "headroomRecordAboveRepinnedBoundMm": 155.264
            - numbers["contract_bound_strengthened_mm"],
            "headroomSparrowAboveRepinnedBoundMm": 150.16451
            - numbers["contract_bound_strengthened_mm"],
        },
    }
    with open(OUT, "w") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    print(OUT)
    print(json.dumps(document["keyFindings"], indent=1))
    print(json.dumps(document["reference"], indent=1))
    print(json.dumps(document["supersedes"]["identityCheckAgainstTheRetiredFile"], indent=1))


if __name__ == "__main__":
    main()
