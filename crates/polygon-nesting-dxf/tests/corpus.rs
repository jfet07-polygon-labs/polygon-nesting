use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use polygon_nesting_dxf::{import_directory, ImportOptions};
use polygon_nesting_protocol::{encode_request, EngineProfile, SourceGeometrySegment};
use sha2::{Digest, Sha256};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/dxf")
        .join(name)
}

fn options() -> ImportOptions {
    ImportOptions {
        sheet_width: 2_000.0,
        sheet_height: 2_700.0,
        padding: 10,
        profile: EngineProfile::Compact,
        allow_mirror: true,
        timeout_ms: 300_000.0,
    }
}

#[test]
fn shapes_17_imports_the_real_configurator_curve_corpus() {
    let request =
        import_directory(&fixture("shapes-17"), &options()).expect("Shapes-17 DXFs should import");

    assert_eq!(request.pieces.len(), 17);
    assert_eq!(request.source_pieces.len(), 17);
    assert!(request
        .source_pieces
        .iter()
        .flat_map(|source| &source.geometry.segments)
        .any(|segment| matches!(segment, SourceGeometrySegment::Arc(_))));
    assert!(request
        .source_pieces
        .iter()
        .flat_map(|source| &source.geometry.segments)
        .any(|segment| matches!(
            segment,
            SourceGeometrySegment::Line(line) if line.source_curve.is_some()
        )));
    request
        .validate()
        .expect("Shapes-17 request should validate");
}

#[test]
fn mixed_61_preserves_the_golden_geometry_families_and_request_bytes() {
    let request =
        import_directory(&fixture("mixed-61"), &options()).expect("Mixed-61 DXFs should import");

    assert_eq!(request.pieces.len(), 61);
    assert_eq!(request.source_pieces.len(), 61);
    let mut family_counts = BTreeMap::new();
    for piece in &request.pieces {
        *family_counts
            .entry(
                piece
                    .interchangeability_key
                    .as_deref()
                    .expect("imported pieces have a geometry key"),
            )
            .or_insert(0_u32) += 1;
    }
    let mut family_counts = family_counts.into_values().collect::<Vec<_>>();
    family_counts.sort_unstable_by(|left, right| right.cmp(left));
    assert_eq!(family_counts, [20, 8, 5, 5, 5, 4, 4, 3, 3, 2, 2]);

    let encoded = encode_request(&request).expect("request should encode");
    assert_eq!(
        format!("{:x}", Sha256::digest(encoded)),
        "8541b4e53009ea62a3ef898621c5cc6e23206b5db4dc05381f00ede17c0f9ded"
    );
}
