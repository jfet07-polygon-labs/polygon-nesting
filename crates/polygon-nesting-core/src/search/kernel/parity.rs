//! Parity smoke test: [`JaguaKernel`] against [`LegacyKernel`].
//!
//! This is the *smallest honest* version of the agreement gate the
//! next-generation plan specifies. It is not that gate: the real one replays a
//! 250,000-pose deterministic stream with pruning disabled and an on-demand
//! `f64` oracle. This one proves that the seam admits a second implementation
//! and that the second implementation is not obviously wrong, on real geometry.
//!
//! # What is compared, and against what
//!
//! Both kernels are handed the same pieces from the Mixed-61 request and the
//! same poses, and each answers "do these two collide". Their answers are
//! compared to each other *and* to the `f64` Clipper answer for the same two
//! expanded rings, which is the truth both of them approximate.
//!
//! # The ambiguity band is derived, not tuned
//!
//! Jagua works in `f32`. Two shapes whose true separation is smaller than the
//! representation error of the coordinates involved may legitimately get a
//! different verdict, and the plan says so: agreement is required outside "an
//! explicitly derived `f32`/contact ambiguity band". So each sample is first
//! *classified* with exact `f64` geometry - by asking whether the pair is still
//! apart when both rings are grown by the band, and whether it still overlaps
//! when both are shrunk by it. Samples that are neither are counted and
//! skipped; every other sample must produce identical verdicts from both
//! kernels.
//!
//! That classification is what keeps the test from degenerating into "widen the
//! tolerance until it passes": the band enters only as a *shrink/grow of the
//! geometry being classified*, never as a slack in the comparison.

use crate::domain::ImportedPiece;
use crate::geometry::general_polygon::PolygonSet;
use crate::geometry::general_source::polygon_set_from_imported_piece;
use crate::search::general_fast::polygons_overlap_exact;
use crate::search::general_relaxed::oriented_surrogate_for_kernel;

use super::jagua::{JaguaKernel, JaguaShape};
use super::{ExplorationKernel, KernelPose, KernelProbes, LegacyKernel, PosedShape, LEGACY};

/// The Mixed-61 request's own geometry contract.
const FLATTENING_SAG_TOLERANCE_MM: f64 = 0.25;
/// A representative collision expansion: half the 10 mm piece padding plus the
/// request's 0.25 mm safety margin. The exact value does not matter to the
/// comparison, only that both kernels receive the same one.
const EXPANSION_MM: f64 = 5.25;

/// The separation within which an `f32` kernel may legitimately disagree with
/// an `f64` one, in millimetres.
///
/// A coordinate of magnitude `m` carries up to half an `f32` ulp, i.e.
/// `m * f32::EPSILON / 2`, of representation error. An edge-intersection
/// predicate combines a handful of such coordinates - differences, products,
/// and a sign - so the band is a small integer multiple of that. It is derived
/// from the magnitudes the query actually works at, and it is not consulted by
/// anything that decides feasibility.
fn ambiguity_band_mm(max_abs_coordinate_mm: f64) -> f64 {
    16.0 * (f64::from(f32::EPSILON) * max_abs_coordinate_mm)
}

/// One prepared piece, in both kernels' representations plus the exact rings
/// the classification uses.
struct ParityPiece {
    id: String,
    legacy: <LegacyKernel as ExplorationKernel>::Shape,
    jagua: JaguaShape,
    /// The exact ring grown by the band: still-apart means definitely apart.
    grown: PolygonSet,
    /// The exact ring shrunk by the band: still-overlapping means definitely
    /// overlapping.
    shrunk: PolygonSet,
}

fn load_source_pieces() -> Vec<(String, PolygonSet)> {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mixed-61/mixed61-request.json");
    let raw = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|error| panic!("failed to read {fixture_path:?}: {error}"));
    let request: serde_json::Value = serde_json::from_str(&raw).expect("fixture parses as JSON");
    let source_pieces = request
        .get("sourcePieces")
        .and_then(serde_json::Value::as_array)
        .expect("fixture has a sourcePieces array");
    source_pieces
        .iter()
        .map(|raw_piece| {
            let piece: ImportedPiece =
                serde_json::from_value(raw_piece.clone()).expect("source piece decodes");
            let polygon = polygon_set_from_imported_piece(&piece, FLATTENING_SAG_TOLERANCE_MM)
                .expect("source piece converts to a polygon set");
            (piece.id.to_string(), polygon)
        })
        .collect()
}

/// Prepares the same geometry for both kernels at one orientation.
fn prepare(
    index: usize,
    id: &str,
    source: &PolygonSet,
    rotation_deg: f64,
    mirrored: bool,
    band_mm: f64,
) -> ParityPiece {
    let pose = KernelPose::oriented(rotation_deg, mirrored);
    ParityPiece {
        id: id.to_owned(),
        legacy: oriented_surrogate_for_kernel(source, rotation_deg, mirrored, EXPANSION_MM)
            .expect("legacy surrogate builds"),
        jagua: JaguaShape::prepare(index, source, pose, EXPANSION_MM, true)
            .expect("jagua conversion succeeds"),
        grown: LEGACY
            .exact_authority()
            .collision_polygon(source, pose, EXPANSION_MM + band_mm / 2.0)
            .expect("grown collision polygon builds"),
        shrunk: LEGACY
            .exact_authority()
            .collision_polygon(source, pose, EXPANSION_MM - band_mm / 2.0)
            .expect("shrunk collision polygon builds"),
    }
}

/// Whether the exact `f64` rings overlap at these translations.
fn exact_overlap_at(
    first: &PolygonSet,
    first_x: f64,
    first_y: f64,
    second: &PolygonSet,
    second_x: f64,
    second_y: f64,
) -> bool {
    let first = first
        .translated(first_x, first_y)
        .expect("translating an exact ring succeeds");
    let second = second
        .translated(second_x, second_y)
        .expect("translating an exact ring succeeds");
    polygons_overlap_exact(&first, &second).expect("exact overlap query succeeds")
}

#[test]
fn jagua_kernel_agrees_with_the_legacy_kernel_outside_the_ambiguity_band() {
    let sources = load_source_pieces();
    assert_eq!(sources.len(), 61, "the Mixed-61 fixture carries 61 pieces");

    // A handful of pieces, at a handful of orientations, deterministically
    // chosen by index so the sample set is stable across runs and platforms.
    // Nothing here is Mixed-61-specific beyond "these are real concave pieces":
    // the same harness runs on any request's source pieces.
    let selected = [0usize, 7, 19, 30, 44];
    let orientations = [(0.0_f64, false), (37.5, false), (123.75, true)];

    // The query works at translations of this magnitude, which is what sets
    // the representation error the band has to cover.
    let base_x = 400.0_f64;
    let base_y = 900.0_f64;
    let band_mm = ambiguity_band_mm(base_x.max(base_y) + 400.0);
    assert!(
        band_mm > 0.0 && band_mm < 0.01,
        "the derived band should be micrometres, got {band_mm}"
    );

    let mut prepared = Vec::new();
    for (slot, index) in selected.iter().copied().enumerate() {
        let (id, source) = &sources[index];
        for (rotation_deg, mirrored) in orientations {
            prepared.push(prepare(slot, id, source, rotation_deg, mirrored, band_mm));
        }
    }

    // The conversion error the adapter actually measured must be inside the
    // band the classification uses; otherwise the band is not covering the
    // thing it claims to cover.
    for first in &prepared {
        for second in &prepared {
            assert!(
                first.jagua.ambiguity_mm(&second.jagua) <= band_mm,
                "measured conversion error {} exceeds the derived band {band_mm}",
                first.jagua.ambiguity_mm(&second.jagua)
            );
        }
    }

    let mut kernel = JaguaKernel::new();
    let mut legacy = LegacyKernel;
    let mut compared = 0usize;
    let mut ambiguous = 0usize;
    let mut colliding = 0usize;
    let mut separated = 0usize;

    for (first_index, first) in prepared.iter().enumerate() {
        // Pair each shape with the next few, so every sampled pair is a
        // different geometry combination rather than a shape against itself.
        for second in prepared.iter().skip(first_index + 1).take(2) {
            for step in 0..24 {
                let offset = step as f64 * 8.0;
                let (second_x, second_y) = (base_x + offset, base_y + offset / 2.0);

                let definitely_apart = !exact_overlap_at(
                    &first.grown,
                    base_x,
                    base_y,
                    &second.grown,
                    second_x,
                    second_y,
                );
                let definitely_overlapping = exact_overlap_at(
                    &first.shrunk,
                    base_x,
                    base_y,
                    &second.shrunk,
                    second_x,
                    second_y,
                );

                let mut probes = KernelProbes::default();
                let legacy_verdict = legacy.pair_collides(
                    PosedShape::new(&first.legacy, base_x, base_y),
                    PosedShape::new(&second.legacy, second_x, second_y),
                    &mut probes,
                );
                let mut jagua_probes = KernelProbes::default();
                let jagua_verdict = kernel.pair_collides(
                    PosedShape::new(&first.jagua, base_x, base_y),
                    PosedShape::new(&second.jagua, second_x, second_y),
                    &mut jagua_probes,
                );
                assert_eq!(
                    kernel.take_error(),
                    None,
                    "the jagua kernel reported an error for {} vs {}",
                    first.id,
                    second.id
                );

                if definitely_apart {
                    separated += 1;
                    compared += 1;
                    assert!(
                        !legacy_verdict,
                        "legacy reported a collision for a definitely separated pair \
                         ({} vs {} at +{offset})",
                        first.id, second.id
                    );
                    assert!(
                        !jagua_verdict,
                        "jagua reported a collision for a definitely separated pair \
                         ({} vs {} at +{offset})",
                        first.id, second.id
                    );
                } else if definitely_overlapping {
                    colliding += 1;
                    compared += 1;
                    assert!(
                        legacy_verdict,
                        "legacy missed a definitely overlapping pair \
                         ({} vs {} at +{offset})",
                        first.id, second.id
                    );
                    assert!(
                        jagua_verdict,
                        "jagua missed a definitely overlapping pair \
                         ({} vs {} at +{offset})",
                        first.id, second.id
                    );
                } else {
                    ambiguous += 1;
                    continue;
                }

                assert_eq!(
                    legacy_verdict, jagua_verdict,
                    "kernels disagreed outside the ambiguity band ({} vs {} at +{offset})",
                    first.id, second.id
                );
            }
        }
    }

    // The sample set has to actually exercise both answers, or an agreeing
    // "never collides" kernel would pass.
    assert!(
        colliding >= 20,
        "expected the sweep to produce overlapping samples, got {colliding}"
    );
    assert!(
        separated >= 20,
        "expected the sweep to produce separated samples, got {separated}"
    );
    // The band is micrometres wide and the sweep steps are millimetres, so
    // landing inside it should be rare. A large count would mean the band, not
    // the kernel, is doing the work.
    assert!(
        ambiguous * 20 <= compared,
        "too many samples fell in the ambiguity band: {ambiguous} of {} total",
        compared + ambiguous
    );
}

/// There is one exact tier, it is the legacy one, and the kernel a caller holds
/// cannot change it.
///
/// PR3 stated this as a runtime property and checked it by asking two kernels
/// the same exact question. PR6 made it a *type* property instead: the exact
/// services are not on [`ExplorationKernel`], so `kernel.exact_pair_overlaps(..)`
/// no longer names anything, and the only grant of authority is inherent to
/// [`LegacyKernel`]. The compiler now enforces what this test used to assert,
/// which is why the jagua kernel does not appear in it — there is no second
/// answer left to compare against.
///
/// What remains worth checking at runtime is that the two doors onto the tier
/// agree: the general entry point [`polygons_overlap_exact`], which every
/// constructor and deep-operator confirmation uses, and the authority method it
/// forwards to.
#[test]
fn the_exact_tier_has_one_implementation_and_it_is_the_legacy_one() {
    let sources = load_source_pieces();
    let (_, first_source) = &sources[3];
    let (_, second_source) = &sources[11];

    let mut overlapping = 0usize;
    let mut separated = 0usize;
    for (rotation_deg, mirrored) in [(0.0_f64, false), (61.25, true)] {
        let pose = KernelPose::oriented(rotation_deg, mirrored);
        let polygon = LEGACY
            .exact_authority()
            .collision_polygon(first_source, pose, EXPANSION_MM)
            .expect("the exact tier builds the collision polygon");
        let other = LEGACY
            .exact_authority()
            .collision_polygon(second_source, pose, EXPANSION_MM)
            .expect("the exact tier builds the partner collision polygon");
        for translate in [0.0_f64, 30.0, 400.0] {
            let moved = other
                .translated(translate, translate)
                .expect("translating succeeds");
            let through_authority = LEGACY
                .exact_authority()
                .pair_overlaps(&polygon, polygon.bounds(), &moved, moved.bounds())
                .expect("the exact tier answers the query");
            let through_entry_point =
                polygons_overlap_exact(&polygon, &moved).expect("the entry point answers");
            assert_eq!(
                through_authority, through_entry_point,
                "the exact tier must not depend on which door it was asked through"
            );
            if through_authority {
                overlapping += 1;
            } else {
                separated += 1;
            }
        }
    }
    // Both answers have to occur, or an "always false" tier would pass.
    assert!(overlapping > 0, "expected at least one overlapping sample");
    assert!(separated > 0, "expected at least one separated sample");
}
