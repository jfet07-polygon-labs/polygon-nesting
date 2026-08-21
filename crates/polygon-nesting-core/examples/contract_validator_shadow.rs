//! The release-build shadow corpus for `fast-contract-validator`.
//!
//! `docs/experiments/fast-contract-validator/` could offer two kinds of
//! equivalence evidence and neither was this one. The `debug_assert` on the skip
//! re-runs both bypassed tests, but it is compiled out of a release build, so
//! the 5.9M-pair census that carries the headline ran with no checking behind it
//! at all. And `the_broad_phase_changes_no_verdict` compared the feature-on path
//! against *enumerated expectations*, which is not two implementations meeting.
//!
//! This is the missing gate, and it is deliberately a `--release` binary rather
//! than a test:
//!
//! * every check below is an explicit branch that reports, never a
//!   `debug_assert`, so it fires in exactly the build profile the campaign
//!   measures and ships;
//! * `validate_publication` (filter armed) and
//!   `validate_publication_exact_reference` (exact loop on every pair) run in the
//!   **same process** on the same input, so this is two runtime implementations
//!   meeting rather than one path meeting a hand-written table;
//! * the comparison is on the whole `Result` **including the error message**, so
//!   a filter that changed *which* pair failed first would be caught, not only
//!   one that changed the verdict;
//! * `contract_validator_shadow_audit` additionally re-runs both bypassed tests
//!   per certified pair, so a skip that happens to land on a layout whose verdict
//!   is unchanged for another reason is still checked on its own terms.
//!
//! The corpus is randomized from a fixed seed, so it is reproducible, and the
//! report prints `provedClear` beside `pairs`: **a corpus that certifies nothing
//! has tested nothing**, and that number is the reader's licence to believe the
//! zero.
//!
//! Usage: `contract_validator_shadow [CASES] [SEED]`, printing a JSON report on
//! stdout and exiting non-zero on any mismatch.

use polygon_nesting_core::domain::IrregularPoint;
use polygon_nesting_core::geometry::general_polygon::{PolygonRegion, PolygonSet};
use polygon_nesting_core::validation::general_polygon::{
    contract_validator_shadow_audit, validate_publication, validate_publication_exact_reference,
    GeneralPlacement, PublicationValidationSettings,
};

/// SplitMix64: a deterministic generator with no dependency and no shared state,
/// so a case index reproduces its own geometry exactly.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[low, high)`.
    fn range(&mut self, low: f64, high: f64) -> f64 {
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        low + unit * (high - low)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

fn point(x: f64, y: f64) -> IrregularPoint {
    IrregularPoint::new(x, y)
}

/// A convex `sides`-gon, radius `radius`, centred on the origin.
fn polygon(sides: usize, radius: f64, phase: f64) -> Option<PolygonSet> {
    let points = (0..sides)
        .map(|index| {
            let angle = phase + std::f64::consts::TAU * index as f64 / sides as f64;
            point(radius * angle.cos(), radius * angle.sin())
        })
        .collect::<Vec<_>>();
    PolygonSet::from_outer(points).ok()
}

/// An axis-aligned rectangle centred on the origin.
fn rectangle(width: f64, height: f64) -> Option<PolygonSet> {
    PolygonSet::from_outer(vec![
        point(-width / 2.0, -height / 2.0),
        point(width / 2.0, -height / 2.0),
        point(width / 2.0, height / 2.0),
        point(-width / 2.0, height / 2.0),
    ])
    .ok()
}

/// A ring with a square hole in it - the case the previous round's fixtures
/// never carried, and the one where "far apart" and "legal" come apart most
/// sharply: a piece sitting *inside* the hole is legal and close.
fn ring_with_hole(outer: f64, hole: f64) -> Option<PolygonSet> {
    let square = |side: f64| {
        vec![
            point(-side / 2.0, -side / 2.0),
            point(side / 2.0, -side / 2.0),
            point(side / 2.0, side / 2.0),
            point(-side / 2.0, side / 2.0),
        ]
    };
    let region = PolygonRegion::new(square(outer), vec![square(hole)]).ok()?;
    PolygonSet::new(vec![region]).ok()
}

/// Two disjoint squares in **one** set, so a single placement is multi-region
/// and its slabs enclose the union rather than either part.
fn two_regions(side: f64, separation: f64) -> Option<PolygonSet> {
    let square = |centre: f64| {
        vec![
            point(centre - side / 2.0, -side / 2.0),
            point(centre + side / 2.0, -side / 2.0),
            point(centre + side / 2.0, side / 2.0),
            point(centre - side / 2.0, side / 2.0),
        ]
    };
    let left = PolygonRegion::new(square(-separation), Vec::new()).ok()?;
    let right = PolygonRegion::new(square(separation), Vec::new()).ok()?;
    PolygonSet::new(vec![left, right]).ok()
}

/// A sliver: a rectangle so thin that its rings are all but collinear, which is
/// where `point_segment_distance`'s projection is worst conditioned.
fn sliver(length: f64, thickness: f64) -> Option<PolygonSet> {
    rectangle(length, thickness)
}

struct Shape {
    family: &'static str,
    set: PolygonSet,
}

fn shapes() -> Vec<Shape> {
    let mut out = Vec::new();
    let mut push = |family: &'static str, set: Option<PolygonSet>| {
        if let Some(set) = set {
            out.push(Shape { family, set });
        }
    };
    push("triangle", polygon(3, 6.0, 0.0));
    push("square", rectangle(5.0, 5.0));
    push("pentagon", polygon(5, 4.0, 0.3));
    push("dodecagon", polygon(12, 7.0, 0.1));
    push("tall-rect", rectangle(1.5, 14.0));
    push("wide-rect", rectangle(14.0, 1.5));
    push("big-square", rectangle(30.0, 30.0));
    push("holed-ring", ring_with_hole(24.0, 14.0));
    push("holed-ring-tight", ring_with_hole(18.0, 15.0));
    push("multi-region", two_regions(4.0, 9.0));
    push("multi-region-wide", two_regions(3.0, 20.0));
    push("sliver", sliver(20.0, 0.01));
    push("sliver-fine", sliver(30.0, 0.001));
    push("tiny", rectangle(0.02, 0.02));
    push("tiny-gon", polygon(7, 0.05, 0.7));
    out
}

/// The clearances the engine is actually asked for, plus the extremes either
/// side of them.
const CLEARANCES: [f64; 6] = [0.0, 0.0005, 0.002, 0.5, 5.0, 40.0];

struct Totals {
    cases: u64,
    pairs: u64,
    proved_clear: u64,
    domain_refusals: u64,
    preamble_rejected: u64,
    near_threshold: u64,
    accepted: u64,
    rejected: u64,
    tightest_certified_excess: f64,
    verdict_mismatches: Vec<String>,
    audit_mismatches: Vec<String>,
    family_clear: std::collections::BTreeMap<String, u64>,
}

impl Totals {
    fn new() -> Self {
        Self {
            cases: 0,
            pairs: 0,
            proved_clear: 0,
            domain_refusals: 0,
            preamble_rejected: 0,
            near_threshold: 0,
            accepted: 0,
            rejected: 0,
            tightest_certified_excess: f64::INFINITY,
            verdict_mismatches: Vec::new(),
            audit_mismatches: Vec::new(),
            family_clear: std::collections::BTreeMap::new(),
        }
    }
}

/// Runs one layout through both implementations and the per-pair audit.
fn check(
    label: &str,
    families: &str,
    placements: &[GeneralPlacement<'_>],
    settings: PublicationValidationSettings,
    totals: &mut Totals,
) {
    totals.cases += 1;

    // 1. Two runtime implementations, same process, whole `Result` compared -
    //    the error *message* included, not just `is_ok`.
    let filtered =
        validate_publication(placements, settings).map_err(|error| error.message().to_string());
    let exact = validate_publication_exact_reference(placements, settings)
        .map_err(|error| error.message().to_string());
    if filtered != exact {
        totals
            .verdict_mismatches
            .push(format!("{label}: filtered={filtered:?} exact={exact:?}"));
    }
    if filtered.is_ok() {
        totals.accepted += 1;
    } else {
        totals.rejected += 1;
    }

    // 2. The explicit per-pair shadow: every certificate re-checked against both
    //    tests it claimed, in release.
    let audit = contract_validator_shadow_audit(placements, settings);
    totals.pairs += audit.pairs;
    totals.proved_clear += audit.proved_clear;
    totals.domain_refusals += audit.domain_refusals;
    totals.tightest_certified_excess = totals
        .tightest_certified_excess
        .min(audit.tightest_certified_excess);
    if audit.preamble_rejected {
        totals.preamble_rejected += 1;
    }
    if audit.proved_clear > 0 {
        *totals.family_clear.entry(families.to_string()).or_insert(0) += audit.proved_clear;
    }
    for mismatch in &audit.mismatches {
        totals.audit_mismatches.push(format!("{label}: {mismatch}"));
    }
}

fn settings_for(clearance: f64, span: f64) -> PublicationValidationSettings {
    PublicationValidationSettings {
        sheet_width_mm: span,
        sheet_height_mm: span,
        total_padding_mm: clearance,
        sheet_edge_clearance_mm: Some(0.0),
        flattening_sag_tolerance_mm: 0.0,
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cases: usize = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(200_000);
    let seed: u64 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0x5EED_1234_ABCD_0001);

    let catalogue = shapes();
    let mut rng = Rng(seed);
    let mut totals = Totals::new();

    // The sheet is generous and every piece is held well inside it, because a
    // `validate_sheet` rejection short-circuits the pair loop and would silently
    // turn a geometry case into a no-op.
    const SPAN: f64 = 4000.0;
    const ORIGIN: f64 = 500.0;

    for case in 0..cases {
        let first = &catalogue[rng.below(catalogue.len())];
        let second = &catalogue[rng.below(catalogue.len())];
        let clearance = CLEARANCES[rng.below(CLEARANCES.len())];
        let settings = settings_for(clearance, SPAN);
        let families = format!("{}|{}", first.family, second.family);

        let rotation_a = rng.range(0.0, 360.0);
        let rotation_b = rng.range(0.0, 360.0);
        let mirrored_a = rng.next_u64() & 1 == 0;
        let mirrored_b = rng.next_u64() & 1 == 0;

        // Four displacement regimes, so the corpus is not accidentally all
        // "obviously far apart" - which is the corpus that proves nothing.
        //
        //   0: deep overlap / containment      2: near-threshold band
        //   1: contact and the clearance band  3: separated, where skips live
        let regime = case % 4;
        let (dx, dy) = match regime {
            0 => (rng.range(-3.0, 3.0), rng.range(-3.0, 3.0)),
            1 => (rng.range(-12.0, 12.0), rng.range(-12.0, 12.0)),
            2 => {
                // Sol's "within +-10 margin widths of clearance": the margin at
                // this coordinate scale is ~1e-9 mm, so this lands the offset on
                // a lattice of that size around a plausible touching distance.
                let margin = 1e-9 + 1e-12 * (ORIGIN + 40.0);
                let steps = rng.range(-10.0, 10.0);
                let base = rng.range(0.0, 40.0);
                totals.near_threshold += 1;
                (base + clearance + steps * margin, rng.range(-1.0, 1.0))
            }
            _ => (rng.range(-60.0, 60.0), rng.range(-60.0, 60.0)),
        };

        let placements = [
            GeneralPlacement {
                piece_id: "a",
                polygon: &first.set,
                rotation_deg: rotation_a,
                mirrored: mirrored_a,
                translate_x: ORIGIN,
                translate_y: ORIGIN,
            },
            GeneralPlacement {
                piece_id: "b",
                polygon: &second.set,
                rotation_deg: rotation_b,
                mirrored: mirrored_b,
                translate_x: ORIGIN + dx,
                translate_y: ORIGIN + dy,
            },
        ];
        check(
            &format!("case {case} regime {regime} clearance {clearance}"),
            &families,
            &placements,
            settings,
            &mut totals,
        );
    }

    // Contractual extremes, run deterministically rather than sampled: the
    // coordinate magnitudes and clearances at the edges of what the contract
    // admits, where the margin and the domain guard are closest to binding.
    let big = rectangle(2.0, 2.0).expect("unit square");
    for magnitude in [1.0e-3, 1.0, 1.0e3, 1.0e6, 1.0e9, 1.0e12] {
        for clearance in CLEARANCES {
            for gap in [0.0, 0.5, 4.0, 400.0] {
                let span = magnitude * 4.0 + 1000.0;
                let settings = settings_for(clearance, span);
                let placements = [
                    GeneralPlacement {
                        piece_id: "a",
                        polygon: &big,
                        rotation_deg: 0.0,
                        mirrored: false,
                        translate_x: magnitude,
                        translate_y: magnitude,
                    },
                    GeneralPlacement {
                        piece_id: "b",
                        polygon: &big,
                        rotation_deg: 0.0,
                        mirrored: false,
                        translate_x: magnitude + 2.0 + clearance + gap,
                        translate_y: magnitude,
                    },
                ];
                check(
                    &format!("extreme magnitude {magnitude:e} clearance {clearance} gap {gap}"),
                    "extreme",
                    &placements,
                    settings,
                    &mut totals,
                );
            }
        }
    }

    // The near-threshold sweep, deterministic and axis-aligned so the true
    // boundary distance is *known* rather than sampled.
    //
    // The randomized regime 2 above cannot do this job: a random rotation makes
    // the achieved gap random too, so it lands nowhere near the margin. Here both
    // pieces are axis-aligned rectangles and the second is displaced by exactly
    // `clearance + k * margin`, so the exact distance is the displacement and `k`
    // walks the pair straight through the threshold. `k < 0` is a **violation**
    // the exact loop must reject, and certifying one would be the unsoundness
    // this whole round is about.
    let unit = rectangle(6.0, 6.0).expect("6mm square");
    for clearance in CLEARANCES {
        // The margin the validator computes for a layout at this scale, mirrored
        // here so the sweep steps in its units rather than in arbitrary ones.
        let margin = 1e-9 + 1e-12 * (ORIGIN + 12.0);
        for step in -10..=10 {
            for diagonal in [false, true] {
                let delta = f64::from(step) * margin;
                let separation = 6.0 + clearance + delta;
                let settings = settings_for(clearance, SPAN);
                // Along the diagonal the two squares are offset in both axes, so
                // the binding direction is a (1,1)/(1,-1) slab rather than an
                // axis one - and the sqrt(2) threshold scaling is what has to be
                // right.
                let (dx, dy) = if diagonal {
                    (separation, separation)
                } else {
                    (separation, 0.0)
                };
                let placements = [
                    GeneralPlacement {
                        piece_id: "a",
                        polygon: &unit,
                        rotation_deg: 0.0,
                        mirrored: false,
                        translate_x: ORIGIN,
                        translate_y: ORIGIN,
                    },
                    GeneralPlacement {
                        piece_id: "b",
                        polygon: &unit,
                        rotation_deg: 0.0,
                        mirrored: false,
                        translate_x: ORIGIN + dx,
                        translate_y: ORIGIN + dy,
                    },
                ];
                totals.near_threshold += 1;
                check(
                    &format!("threshold clearance {clearance} step {step} diagonal {diagonal}"),
                    "threshold",
                    &placements,
                    settings,
                    &mut totals,
                );
            }
        }
    }

    // Multi-piece layouts, so the scan row's *ordering* is exercised: which pair
    // fails first is part of the message, and a filter that skipped the wrong
    // pair would change it.
    let mut rng = Rng(seed ^ 0xA5A5_A5A5_A5A5_A5A5);
    for case in 0..(cases / 20).max(200) {
        let count = 3 + rng.below(6);
        let clearance = CLEARANCES[rng.below(CLEARANCES.len())];
        let settings = settings_for(clearance, SPAN);
        let picks = (0..count)
            .map(|_| rng.below(catalogue.len()))
            .collect::<Vec<_>>();
        let offsets = (0..count)
            .map(|_| (rng.range(-45.0, 45.0), rng.range(-45.0, 45.0)))
            .collect::<Vec<_>>();
        let rotations = (0..count)
            .map(|_| rng.range(0.0, 360.0))
            .collect::<Vec<_>>();
        let placements = (0..count)
            .map(|index| GeneralPlacement {
                piece_id: PIECE_IDS[index % PIECE_IDS.len()],
                polygon: &catalogue[picks[index]].set,
                rotation_deg: rotations[index],
                mirrored: index % 3 == 0,
                translate_x: ORIGIN + offsets[index].0,
                translate_y: ORIGIN + offsets[index].1,
            })
            .collect::<Vec<_>>();
        check(
            &format!("multi {case} count {count} clearance {clearance}"),
            "multi",
            &placements,
            settings,
            &mut totals,
        );
    }

    let ok = totals.verdict_mismatches.is_empty() && totals.audit_mismatches.is_empty();
    println!("{{");
    println!("  \"seed\": {seed},");
    println!("  \"cases\": {},", totals.cases);
    println!("  \"pairs\": {},", totals.pairs);
    println!("  \"provedClear\": {},", totals.proved_clear);
    println!(
        "  \"provedClearRate\": {:.6},",
        if totals.pairs == 0 {
            0.0
        } else {
            totals.proved_clear as f64 / totals.pairs as f64
        }
    );
    println!("  \"domainRefusals\": {},", totals.domain_refusals);
    println!("  \"preambleRejected\": {},", totals.preamble_rejected);
    println!("  \"nearThresholdCases\": {},", totals.near_threshold);
    println!(
        "  \"tightestCertifiedExcessMm\": {:e},",
        totals.tightest_certified_excess
    );
    println!("  \"acceptedLayouts\": {},", totals.accepted);
    println!("  \"rejectedLayouts\": {},", totals.rejected);
    println!(
        "  \"verdictMismatches\": {},",
        totals.verdict_mismatches.len()
    );
    println!("  \"auditMismatches\": {},", totals.audit_mismatches.len());
    println!("  \"familiesCertifying\": {},", totals.family_clear.len());
    println!("  \"allClear\": {ok},");
    println!("  \"examples\": [");
    for (index, example) in totals
        .verdict_mismatches
        .iter()
        .chain(totals.audit_mismatches.iter())
        .take(8)
        .enumerate()
    {
        let comma = if index == 0 { "" } else { "," };
        println!("    {comma}{:?}", example);
    }
    println!("  ]");
    println!("}}");

    if !ok {
        eprintln!(
            "SHADOW CORPUS FAILED: {} verdict and {} audit mismatches",
            totals.verdict_mismatches.len(),
            totals.audit_mismatches.len()
        );
        std::process::exit(1);
    }
}

const PIECE_IDS: [&str; 9] = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];
