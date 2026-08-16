use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};
use std::fmt::{Display, Formatter};

use polygon_nesting_core::geometry::predicates::orientation;
use polygon_nesting_protocol::{
    DiagnosticTraceMode, EngineProfile, EngineRequest, EngineSettings, GeometrySettings,
    HistoryMode, OptimizerSettings, PlacementPolicy, PreparedPiece, ProtocolVersion, Rect,
    RectWithMetrics, SheetSpec, SourceGeometry, SourceGeometryEntityType, SourceGeometrySegment,
    SourceLineSegment, SourcePiece,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const POLYGON_INPUT_VERSION: u32 = 1;
const MAX_POLYGON_DEFINITIONS: usize = 1_000;
const MAX_PIECE_INSTANCES: usize = 10_000;
const MAX_POLYGON_ID_CHARACTERS: usize = 256;
const MAX_VERTICES_PER_POLYGON: usize = 4_096;
const MAX_TOTAL_VERTICES: usize = 1_000_000;
const MAX_SELF_INTERSECTION_PAIR_CHECKS: usize = 10_000_000;
pub(crate) const MAX_POLYGON_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

#[derive(Debug, Clone)]
pub(crate) struct PolygonImportOptions {
    pub sheet_width: f64,
    pub sheet_height: f64,
    pub padding: u64,
    pub profile: EngineProfile,
    pub allow_mirror: bool,
    pub timeout_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolygonImportError {
    message: String,
}

impl PolygonImportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for PolygonImportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PolygonImportError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PolygonInputDocument {
    version: u32,
    polygons: Vec<PolygonDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PolygonDefinition {
    id: String,
    #[serde(default = "default_quantity")]
    quantity: u32,
    points: Vec<PolygonPoint>,
    #[serde(default = "default_true")]
    allow_rotation: bool,
    #[serde(default = "default_true")]
    allow_mirror: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
enum PolygonPoint {
    Object(PointObject),
    Pair([f64; 2]),
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointObject {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

impl PolygonPoint {
    fn into_point(self) -> Point {
        match self {
            Self::Object(point) => Point {
                x: point.x,
                y: point.y,
            },
            Self::Pair([x, y]) => Point { x, y },
        }
    }
}

#[derive(Debug)]
struct NormalizedPolygon {
    id: String,
    quantity: usize,
    allow_rotation: bool,
    allow_mirror: bool,
    real_bounds: Rect,
    segments: Vec<SourceGeometrySegment>,
}

pub(crate) fn import_polygon_json(
    input: &[u8],
    options: &PolygonImportOptions,
) -> Result<EngineRequest, PolygonImportError> {
    validate_options(options)?;
    if input.len() as u64 > MAX_POLYGON_INPUT_BYTES {
        return Err(PolygonImportError::new(format!(
            "polygon input must not exceed {MAX_POLYGON_INPUT_BYTES} bytes"
        )));
    }
    let document: PolygonInputDocument = serde_json::from_slice(input)
        .map_err(|error| PolygonImportError::new(format!("polygon JSON is invalid: {error}")))?;
    if document.version != POLYGON_INPUT_VERSION {
        return Err(PolygonImportError::new(format!(
            "polygon input version must be {POLYGON_INPUT_VERSION}; received {}",
            document.version
        )));
    }
    if document.polygons.is_empty() {
        return Err(PolygonImportError::new(
            "polygons must contain at least one polygon",
        ));
    }
    if document.polygons.len() > MAX_POLYGON_DEFINITIONS {
        return Err(PolygonImportError::new(format!(
            "polygons must contain at most {MAX_POLYGON_DEFINITIONS} definitions"
        )));
    }

    let mut source_ids = BTreeSet::new();
    let mut total_instances = 0usize;
    let mut total_vertices = 0usize;
    let mut total_intersection_pair_checks = 0usize;
    let mut normalized = Vec::with_capacity(document.polygons.len());
    for (index, polygon) in document.polygons.into_iter().enumerate() {
        if polygon.id.trim().is_empty() {
            return Err(PolygonImportError::new(format!(
                "polygons[{index}].id must be a non-empty string"
            )));
        }
        if polygon.id.chars().count() > MAX_POLYGON_ID_CHARACTERS {
            return Err(PolygonImportError::new(format!(
                "polygons[{index}].id must contain at most {MAX_POLYGON_ID_CHARACTERS} characters"
            )));
        }
        if !source_ids.insert(polygon.id.clone()) {
            return Err(PolygonImportError::new(format!(
                "polygons[{index}].id must be unique"
            )));
        }
        let quantity = usize::try_from(polygon.quantity).map_err(|_| {
            PolygonImportError::new(format!("polygons[{index}].quantity is too large"))
        })?;
        if quantity == 0 {
            return Err(PolygonImportError::new(format!(
                "polygons[{index}].quantity must be at least one"
            )));
        }
        total_instances = total_instances.checked_add(quantity).ok_or_else(|| {
            PolygonImportError::new("total polygon quantity exceeds the supported range")
        })?;
        if total_instances > MAX_PIECE_INSTANCES {
            return Err(PolygonImportError::new(format!(
                "total polygon quantity must not exceed {MAX_PIECE_INSTANCES}"
            )));
        }
        total_vertices = total_vertices
            .checked_add(polygon.points.len())
            .ok_or_else(|| PolygonImportError::new("total polygon vertex count is too large"))?;
        if total_vertices > MAX_TOTAL_VERTICES {
            return Err(PolygonImportError::new(format!(
                "total polygon vertex count must not exceed {MAX_TOTAL_VERTICES}"
            )));
        }
        let vertex_count = polygon.points.len();
        let pair_checks = vertex_count
            .checked_mul(vertex_count.saturating_sub(3))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| {
                PolygonImportError::new(
                    "polygon self-intersection work exceeds the supported range",
                )
            })?;
        total_intersection_pair_checks = total_intersection_pair_checks
            .checked_add(pair_checks)
            .ok_or_else(|| {
                PolygonImportError::new(
                    "polygon self-intersection work exceeds the supported range",
                )
            })?;
        if total_intersection_pair_checks > MAX_SELF_INTERSECTION_PAIR_CHECKS {
            return Err(PolygonImportError::new(format!(
                "polygon self-intersection checks must not exceed {MAX_SELF_INTERSECTION_PAIR_CHECKS} edge pairs"
            )));
        }
        normalized.push(normalize_polygon(polygon, index)?);
    }
    normalized.sort_by(|left, right| left.id.cmp(&right.id));

    build_request(normalized, total_instances, options)
}

fn default_quantity() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

fn validate_options(options: &PolygonImportOptions) -> Result<(), PolygonImportError> {
    for (name, value) in [
        ("sheet width", options.sheet_width),
        ("sheet height", options.sheet_height),
        ("timeout", options.timeout_ms),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(PolygonImportError::new(format!(
                "{name} must be a positive finite number"
            )));
        }
    }
    if !options.padding.is_multiple_of(2) {
        return Err(PolygonImportError::new(
            "padding must be an even number of millimetres",
        ));
    }
    Ok(())
}

fn normalize_polygon(
    polygon: PolygonDefinition,
    index: usize,
) -> Result<NormalizedPolygon, PolygonImportError> {
    if polygon.points.len() > MAX_VERTICES_PER_POLYGON {
        return Err(PolygonImportError::new(format!(
            "polygons[{index}].points must contain at most {MAX_VERTICES_PER_POLYGON} vertices"
        )));
    }
    let mut points = Vec::with_capacity(polygon.points.len());
    for mut point in polygon.points.into_iter().map(PolygonPoint::into_point) {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(PolygonImportError::new(format!(
                "polygons[{index}].points must contain only finite coordinates"
            )));
        }
        point.x = fold_negative_zero(point.x);
        point.y = fold_negative_zero(point.y);
        if points.last().is_some_and(|previous| *previous == point) {
            continue;
        }
        points.push(point);
    }
    if points.len() >= 2 && points.first() == points.last() {
        points.pop();
    }
    if points.len() < 3 {
        return Err(PolygonImportError::new(format!(
            "polygons[{index}].points must contain at least three distinct vertices"
        )));
    }
    let mut unique_points = HashSet::with_capacity(points.len());
    if points
        .iter()
        .any(|point| !unique_points.insert((point.x.to_bits(), point.y.to_bits())))
    {
        return Err(PolygonImportError::new(format!(
            "polygons[{index}].points must not repeat a vertex"
        )));
    }

    let mut min_x = points[0].x;
    let mut min_y = points[0].y;
    let mut max_x = points[0].x;
    let mut max_y = points[0].y;
    for point in &points[1..] {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(PolygonImportError::new(format!(
            "polygons[{index}] must have positive finite width and height"
        )));
    }

    let mut normalized = points
        .into_iter()
        .map(|point| Point {
            x: fold_negative_zero(point.x - min_x),
            y: fold_negative_zero(point.y - min_y),
        })
        .collect::<Vec<_>>();
    if !contains_non_collinear_triple(&normalized) {
        return Err(PolygonImportError::new(format!(
            "polygons[{index}] must contain at least three non-collinear vertices"
        )));
    }
    if has_self_intersection(&normalized) {
        return Err(PolygonImportError::new(format!(
            "polygons[{index}] must form a simple ring without self-intersections"
        )));
    }
    canonicalize_ring(&mut normalized);

    let real_width = width.ceil().max(1.0);
    let real_height = height.ceil().max(1.0);
    if real_width > MAX_SAFE_INTEGER || real_height > MAX_SAFE_INTEGER {
        return Err(PolygonImportError::new(format!(
            "polygons[{index}] dimensions exceed the supported range"
        )));
    }
    let segments = normalized
        .iter()
        .copied()
        .zip(normalized.iter().copied().cycle().skip(1))
        .take(normalized.len())
        .map(|(start, end)| {
            SourceGeometrySegment::Line(SourceLineSegment {
                x1: start.x,
                y1: start.y,
                x2: end.x,
                y2: end.y,
                bulge: None,
                source_curve: None,
            })
        })
        .collect();

    Ok(NormalizedPolygon {
        id: polygon.id,
        quantity: polygon.quantity as usize,
        allow_rotation: polygon.allow_rotation,
        allow_mirror: polygon.allow_mirror,
        real_bounds: Rect {
            x: 0.0,
            y: 0.0,
            width: real_width,
            height: real_height,
        },
        segments,
    })
}

fn fold_negative_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn contains_non_collinear_triple(points: &[Point]) -> bool {
    let first = points[0];
    let Some((second_index, second)) = points
        .iter()
        .copied()
        .enumerate()
        .skip(1)
        .find(|(_, point)| *point != first)
    else {
        return false;
    };
    points.iter().enumerate().skip(1).any(|(index, third)| {
        if index == second_index {
            return false;
        }
        let cross =
            (second.x - first.x) * (third.y - first.y) - (second.y - first.y) * (third.x - first.x);
        cross.is_finite() && cross != 0.0
    })
}

fn has_self_intersection(points: &[Point]) -> bool {
    let edge_count = points.len();
    for first_index in 0..edge_count {
        let first = (points[first_index], points[(first_index + 1) % edge_count]);
        for second_index in (first_index + 1)..edge_count {
            if second_index == first_index + 1
                || (first_index == 0 && second_index == edge_count - 1)
            {
                continue;
            }
            let second = (
                points[second_index],
                points[(second_index + 1) % edge_count],
            );
            if segments_intersect(first, second) {
                return true;
            }
        }
    }
    false
}

fn segments_intersect(first: (Point, Point), second: (Point, Point)) -> bool {
    let (first_start, first_end) = first;
    let (second_start, second_end) = second;
    let first_start_turn = point_orientation(first_start, first_end, second_start);
    let first_end_turn = point_orientation(first_start, first_end, second_end);
    let second_start_turn = point_orientation(second_start, second_end, first_start);
    let second_end_turn = point_orientation(second_start, second_end, first_end);

    if first_start_turn == 0 && point_is_on_segment(second_start, first) {
        return true;
    }
    if first_end_turn == 0 && point_is_on_segment(second_end, first) {
        return true;
    }
    if second_start_turn == 0 && point_is_on_segment(first_start, second) {
        return true;
    }
    if second_end_turn == 0 && point_is_on_segment(first_end, second) {
        return true;
    }
    first_start_turn != first_end_turn && second_start_turn != second_end_turn
}

fn point_orientation(origin: Point, first: Point, second: Point) -> i32 {
    orientation(origin.x, origin.y, first.x, first.y, second.x, second.y)
}

fn point_is_on_segment(point: Point, segment: (Point, Point)) -> bool {
    let (start, end) = segment;
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

fn canonicalize_ring(points: &mut Vec<Point>) {
    let start = points
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| compare_points(left, right))
        .map(|(index, _)| index)
        .expect("validated polygon has vertices");
    let forward = (0..points.len())
        .map(|offset| points[(start + offset) % points.len()])
        .collect::<Vec<_>>();
    let reverse = (0..points.len())
        .map(|offset| points[(start + points.len() - offset) % points.len()])
        .collect::<Vec<_>>();
    *points = if compare_rings(&forward, &reverse).is_gt() {
        reverse
    } else {
        forward
    };
}

fn compare_rings(left: &[Point], right: &[Point]) -> Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| compare_points(left, right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(Ordering::Equal)
}

fn compare_points(left: &Point, right: &Point) -> Ordering {
    left.x
        .total_cmp(&right.x)
        .then_with(|| left.y.total_cmp(&right.y))
}

fn build_request(
    polygons: Vec<NormalizedPolygon>,
    total_instances: usize,
    options: &PolygonImportOptions,
) -> Result<EngineRequest, PolygonImportError> {
    let side_padding = options.padding.div_ceil(2) as f64;
    let mut pieces = Vec::with_capacity(total_instances);
    let mut source_pieces = Vec::with_capacity(polygons.len());

    for polygon in polygons {
        let effective_allow_mirror = options.allow_mirror && polygon.allow_mirror;
        let interchangeability_key = geometry_key(
            &polygon.real_bounds,
            &polygon.segments,
            polygon.allow_rotation,
            effective_allow_mirror,
        )?;
        let padded_width = polygon.real_bounds.width + side_padding * 2.0;
        let padded_height = polygon.real_bounds.height + side_padding * 2.0;
        for ordinal in 1..=polygon.quantity {
            pieces.push(PreparedPiece {
                id: format!("{}#{ordinal}", polygon.id),
                source_piece_id: polygon.id.clone(),
                interchangeability_key: Some(interchangeability_key.clone()),
                real_bounds: polygon.real_bounds.clone(),
                padded_bounds: RectWithMetrics {
                    x: 0.0,
                    y: 0.0,
                    width: padded_width,
                    height: padded_height,
                    longest_edge: padded_width.max(padded_height),
                    area: padded_width * padded_height,
                    imbalance: (padded_width - padded_height).abs(),
                },
                padding: side_padding,
                allow_rotation: polygon.allow_rotation,
                allow_mirror: effective_allow_mirror,
                cut_row_ref: None,
            });
        }
        source_pieces.push(SourcePiece {
            id: polygon.id.clone(),
            source_file_id: polygon.id.clone(),
            source_layer: None,
            label: polygon.id,
            real_bounds: polygon.real_bounds,
            geometry: SourceGeometry {
                entity_type: SourceGeometryEntityType::PresetShape,
                closed: true,
                segments: polygon.segments,
            },
            warnings: Vec::new(),
        });
    }

    let request = EngineRequest {
        version: ProtocolVersion::CURRENT,
        timeout_ms: options.timeout_ms,
        profile: options.profile,
        sheet: SheetSpec {
            width: options.sheet_width,
            height: options.sheet_height,
            label: format!("{}x{}", options.sheet_width, options.sheet_height),
        },
        pieces,
        source_pieces,
        settings: EngineSettings {
            padding: options.padding as f64,
            sheet_edge_clearance_mm: None,
            allow_global_rotation: true,
            allow_global_mirror: options.allow_mirror,
            geometry: GeometrySettings {
                flattening_sag_tolerance_mm: 0.25,
                clearance_safety_margin_mm: 0.25,
                geometry_backend_id: "irregular-convex-v2-default".to_owned(),
                geometry_backend_version: "0".to_owned(),
            },
            optimizer: default_optimizer_settings(),
        },
        history_mode: HistoryMode::Off,
        diagnostic_trace_mode: DiagnosticTraceMode::Off,
    };
    request.validate().map_err(|error| {
        PolygonImportError::new(format!("generated EngineRequest is invalid: {error}"))
    })?;
    Ok(request)
}

fn default_optimizer_settings() -> OptimizerSettings {
    OptimizerSettings {
        order_window: 4.0,
        beam_width: 8.0,
        local_candidate_fanout: 4.0,
        local_repair_budget: 0.0,
        intrinsic_shared_archive_enabled: true,
        transform_cap: 8.0,
        transform_minimum_edge_length_mm: 1.0,
        transform_angle_deduplication_tolerance_deg: 0.01,
        configured_rotation_enabled: true,
        edge_alignment_enabled: true,
        configured_rotation_deg: Vec::new(),
        ga_enabled: false,
        baseline_only: true,
        ga_population: 12.0,
        ga_generation_budget: 2.0,
        ga_evaluation_budget: 24.0,
        ga_time_budget_ms: 0.0,
        ga_seed: "default".to_owned(),
        priority_order_mutation_enabled: true,
        transform_preference_mutation_enabled: true,
        placement_policy_mutation_enabled: true,
        placement_policy_id: PlacementPolicy::BalancedCompactness,
        placement_policy_ids: vec![
            PlacementPolicy::BalancedCompactness,
            PlacementPolicy::ShortSideFill,
            PlacementPolicy::EdgeContactThenBalancedCompactness,
        ],
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryKey<'a> {
    real_bounds: GeometryKeyBounds,
    segments: &'a [SourceGeometrySegment],
}

#[derive(Serialize)]
struct GeometryKeyBounds {
    width: f64,
    height: f64,
}

fn geometry_key(
    real_bounds: &Rect,
    segments: &[SourceGeometrySegment],
    allow_rotation: bool,
    allow_mirror: bool,
) -> Result<String, PolygonImportError> {
    let bytes = serde_json::to_vec(&GeometryKey {
        real_bounds: GeometryKeyBounds {
            width: real_bounds.width,
            height: real_bounds.height,
        },
        segments,
    })
    .map_err(|error| {
        PolygonImportError::new(format!("geometry identity could not be encoded: {error}"))
    })?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    if !allow_rotation || !allow_mirror {
        digest.update(b"\0transform-permissions-v1\0");
        digest.update([u8::from(allow_rotation), u8::from(allow_mirror)]);
    }
    let hash = digest.finalize();
    Ok(hash.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{import_polygon_json, PolygonImportOptions};
    use polygon_nesting_dxf::{import_files, ImportOptions};
    use polygon_nesting_protocol::EngineProfile;

    fn options() -> PolygonImportOptions {
        PolygonImportOptions {
            sheet_width: 200.0,
            sheet_height: 200.0,
            padding: 10,
            profile: EngineProfile::Compact,
            allow_mirror: true,
            timeout_ms: 300_000.0,
        }
    }

    #[test]
    fn imports_pair_and_object_points_with_quantities_and_normalization() {
        let input = br#"{
          "version": 1,
          "polygons": [
            {
              "id": "part",
              "quantity": 2,
              "allowRotation": false,
              "allowMirror": false,
              "points": [[-10, 5], {"x": 70, "y": 5}, [70, 45], [-10, 45], [-10, 5]]
            }
          ]
        }"#;

        let request = import_polygon_json(input, &options()).expect("polygon input should import");

        assert_eq!(request.pieces.len(), 2);
        assert_eq!(request.pieces[0].id, "part#1");
        assert_eq!(request.pieces[1].id, "part#2");
        assert!(!request.pieces[0].allow_rotation);
        assert!(!request.pieces[0].allow_mirror);
        assert_eq!(request.source_pieces[0].real_bounds.width, 80.0);
        assert_eq!(request.source_pieces[0].real_bounds.height, 40.0);
        assert_eq!(request.source_pieces[0].geometry.segments.len(), 4);
        let encoded = serde_json::to_value(request).expect("request should encode");
        assert_eq!(
            encoded["sourcePieces"][0]["geometry"]["entityType"],
            "PRESET_SHAPE"
        );
        assert_eq!(
            encoded["sourcePieces"][0]["geometry"]["segments"][0]["x1"],
            0.0
        );
        assert_eq!(
            encoded["sourcePieces"][0]["geometry"]["segments"][0]["y1"],
            0.0
        );
    }

    #[test]
    fn rejects_duplicate_ids_and_degenerate_polygons() {
        let duplicate = br#"{
          "version": 1,
          "polygons": [
            {"id": "same", "points": [[0, 0], [10, 0], [0, 10]]},
            {"id": "same", "points": [[0, 0], [20, 0], [0, 20]]}
          ]
        }"#;
        assert!(import_polygon_json(duplicate, &options())
            .expect_err("duplicate IDs should fail")
            .to_string()
            .contains("must be unique"));

        let collinear = br#"{
          "version": 1,
          "polygons": [{"id": "line", "points": [[0, 0], [10, 10], [20, 20]]}]
        }"#;
        assert!(import_polygon_json(collinear, &options())
            .expect_err("collinear polygon should fail")
            .to_string()
            .contains("non-collinear"));

        let self_intersecting = br#"{
          "version": 1,
          "polygons": [{"id": "bow-tie", "points": [[0, 0], [10, 10], [0, 10], [10, 0]]}]
        }"#;
        assert!(import_polygon_json(self_intersecting, &options())
            .expect_err("self-intersecting polygon should fail")
            .to_string()
            .contains("simple ring"));
    }

    #[test]
    fn rejects_identifiers_that_could_amplify_during_quantity_expansion() {
        let id = "x".repeat(super::MAX_POLYGON_ID_CHARACTERS + 1);
        let input = format!(
            r#"{{
              "version": 1,
              "polygons": [{{"id": {id:?}, "quantity": 10000, "points": [[0, 0], [10, 0], [0, 10]]}}]
            }}"#
        );

        assert!(import_polygon_json(input.as_bytes(), &options())
            .expect_err("oversized identifier should fail")
            .to_string()
            .contains("at most 256 characters"));
    }

    #[test]
    fn rejects_unknown_versions_and_unknown_fields() {
        let version = br#"{
          "version": 2,
          "polygons": [{"id": "part", "points": [[0, 0], [10, 0], [0, 10]]}]
        }"#;
        assert!(import_polygon_json(version, &options())
            .expect_err("unknown version should fail")
            .to_string()
            .contains("version must be 1"));

        let unknown = br#"{
          "version": 1,
          "polygons": [{"id": "part", "unexpected": true, "points": [[0, 0], [10, 0], [0, 10]]}]
        }"#;
        assert!(import_polygon_json(unknown, &options())
            .expect_err("unknown field should fail")
            .to_string()
            .contains("unknown field"));
    }

    #[test]
    fn separates_geometry_families_with_different_transform_permissions() {
        let input = br#"{
          "version": 1,
          "polygons": [
            {"id": "free", "points": [[0, 0], [10, 0], [0, 10]]},
            {
              "id": "restricted",
              "allowRotation": false,
              "allowMirror": false,
              "points": [[0, 0], [10, 0], [0, 10]]
            }
          ]
        }"#;

        let request = import_polygon_json(input, &options()).expect("input should import");

        assert_ne!(
            request.pieces[0].interchangeability_key,
            request.pieces[1].interchangeability_key
        );
    }

    #[test]
    fn rejects_documents_beyond_the_byte_limit_before_deserialization() {
        let input = vec![b' '; (super::MAX_POLYGON_INPUT_BYTES + 1) as usize];

        assert!(import_polygon_json(&input, &options())
            .expect_err("oversized input should fail")
            .to_string()
            .contains("must not exceed"));
    }

    #[test]
    fn sorts_definitions_by_id_before_expanding_quantities() {
        let forward = br#"{
          "version": 1,
          "polygons": [
            {"id": "a", "quantity": 2, "points": [[0, 0], [10, 0], [0, 10]]},
            {"id": "b", "points": [[0, 0], [20, 0], [0, 20]]}
          ]
        }"#;
        let reverse = br#"{
          "version": 1,
          "polygons": [
            {"id": "b", "points": [[20, 0], [0, 0], [0, 20]]},
            {"id": "a", "quantity": 2, "points": [[10, 0], [0, 0], [0, 10]]}
          ]
        }"#;

        let forward =
            import_polygon_json(forward, &options()).expect("forward input should import");
        let reverse =
            import_polygon_json(reverse, &options()).expect("reverse input should import");

        assert_eq!(forward, reverse);
        assert_eq!(forward.pieces[0].id, "a#1");
        assert_eq!(forward.pieces[1].id, "a#2");
        assert_eq!(forward.pieces[2].id, "b#1");
    }

    #[test]
    fn shares_generated_request_defaults_and_piece_identity_with_dxf_import() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "polygon-nesting-polygon-dxf-parity-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let dxf_path = directory.join("part.dxf");
        fs::write(
            &dxf_path,
            "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n0\n21\n10\n0\nLINE\n10\n0\n20\n10\n11\n10\n21\n0\n0\nLINE\n10\n10\n20\n0\n11\n0\n21\n0\n0\nENDSEC\n0\nEOF\n",
        )
        .expect("DXF should be written");
        let dxf_options = ImportOptions {
            sheet_width: 200.0,
            sheet_height: 200.0,
            padding: 10,
            profile: EngineProfile::Compact,
            allow_mirror: true,
            timeout_ms: 300_000.0,
        };
        let dxf_request = import_files(&[dxf_path], &dxf_options).expect("DXF input should import");
        let polygon_request = import_polygon_json(
            br#"{
              "version": 1,
              "polygons": [{"id": "part", "points": [[0, 0], [0, 10], [10, 0]]}]
            }"#,
            &options(),
        )
        .expect("polygon input should import");

        assert_eq!(polygon_request.timeout_ms, dxf_request.timeout_ms);
        assert_eq!(polygon_request.profile, dxf_request.profile);
        assert_eq!(polygon_request.sheet, dxf_request.sheet);
        assert_eq!(polygon_request.pieces, dxf_request.pieces);
        assert_eq!(polygon_request.settings, dxf_request.settings);
        assert_eq!(polygon_request.history_mode, dxf_request.history_mode);
        assert_eq!(
            polygon_request.diagnostic_trace_mode,
            dxf_request.diagnostic_trace_mode
        );
        assert_eq!(
            polygon_request.source_pieces[0].real_bounds,
            dxf_request.source_pieces[0].real_bounds
        );
        assert_eq!(
            polygon_request.source_pieces[0].geometry.segments,
            dxf_request.source_pieces[0].geometry.segments
        );

        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }
}
