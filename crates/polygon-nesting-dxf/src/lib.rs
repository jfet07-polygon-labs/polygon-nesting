use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use polygon_nesting_protocol::{
    DiagnosticTraceMode, EllipseSource, EllipseSourceKind, EngineProfile, EngineRequest,
    EngineSettings, GeometrySettings, HistoryMode, OptimizerSettings, PlacementPolicy,
    PreparedPiece, ProtocolVersion, Rect, RectWithMetrics, SheetSpec, SourceArcSegment,
    SourceGeometry, SourceGeometryEntityType, SourceGeometrySegment, SourceLineSegment,
    SourcePiece,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_DXF_FILES: usize = 1_000;
const CURVE_FALLBACK_SEGMENTS: usize = 32;
const MIN_ELLIPSE_SEGMENTS: usize = 32;
const MAX_ELLIPSE_SEGMENTS: usize = 2_048;
const ELLIPSE_SAG_TOLERANCE_MM: f64 = 0.25;

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub sheet_width: f64,
    pub sheet_height: f64,
    pub padding: u64,
    pub profile: EngineProfile,
    pub allow_mirror: bool,
    pub timeout_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxfImportError {
    message: String,
    path: Option<PathBuf>,
}

impl DxfImportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: None,
        }
    }

    fn at(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: Some(path.into()),
        }
    }
}

impl Display for DxfImportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(path) => write!(formatter, "{}: {}", path.display(), self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

impl std::error::Error for DxfImportError {}

#[derive(Debug, Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone)]
struct Line {
    start: Point,
    end: Point,
}

#[derive(Debug, Clone)]
struct Arc {
    center: Point,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
}

#[derive(Debug, Clone)]
struct Circle {
    center: Point,
    radius: f64,
}

#[derive(Debug, Clone)]
struct Ellipse {
    center: Point,
    major_axis: Point,
    axis_ratio: f64,
    start_param: f64,
    end_param: f64,
}

#[derive(Debug)]
enum Entity {
    Line(Line),
    Arc(Arc),
    Circle(Circle),
    Ellipse(Ellipse),
}

#[derive(Debug, Default)]
struct Entities {
    ordered: Vec<Entity>,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn width(self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(self) -> f64 {
        self.max_y - self.min_y
    }
}

#[derive(Debug)]
struct ImportedPiece {
    id: String,
    real_width: f64,
    real_height: f64,
    segments: Vec<SourceGeometrySegment>,
}

pub fn import_directory(
    directory: &Path,
    options: &ImportOptions,
) -> Result<EngineRequest, DxfImportError> {
    let paths = discover_directory(directory)?;
    import_files(&paths, options)
}

pub fn discover_directory(directory: &Path) -> Result<Vec<PathBuf>, DxfImportError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        DxfImportError::at(
            directory,
            format!("DXF directory could not be read: {error}"),
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            DxfImportError::at(
                directory,
                format!("DXF directory entry could not be read: {error}"),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            DxfImportError::at(
                entry.path(),
                format!("DXF file type could not be read: {error}"),
            )
        })?;
        let path = entry.path();
        if file_type.is_file() && has_dxf_extension(&path) {
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| DxfImportError::at(&path, "DXF filename must be valid UTF-8"))?
                .to_owned();
            paths.push((filename, path));
        }
    }
    paths.sort_by(|(left, _), (right, _)| left.cmp(right));

    if paths.is_empty() {
        return Err(DxfImportError::at(
            directory,
            "directory contains no regular .dxf files",
        ));
    }
    if paths.len() > MAX_DXF_FILES {
        return Err(DxfImportError::at(
            directory,
            format!("directory contains more than {MAX_DXF_FILES} DXF files"),
        ));
    }

    Ok(paths.into_iter().map(|(_, path)| path).collect())
}

pub fn import_files(
    paths: &[PathBuf],
    options: &ImportOptions,
) -> Result<EngineRequest, DxfImportError> {
    validate_options(options)?;
    if paths.is_empty() {
        return Err(DxfImportError::new("no DXF files were supplied"));
    }
    if paths.len() > MAX_DXF_FILES {
        return Err(DxfImportError::new(format!(
            "more than {MAX_DXF_FILES} DXF files were supplied"
        )));
    }
    let imported = paths
        .iter()
        .map(|path| import_file(path))
        .collect::<Result<Vec<_>, _>>()?;
    build_request(imported, options)
}

fn validate_options(options: &ImportOptions) -> Result<(), DxfImportError> {
    for (name, value) in [
        ("sheet width", options.sheet_width),
        ("sheet height", options.sheet_height),
        ("timeout", options.timeout_ms),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(DxfImportError::new(format!(
                "{name} must be a positive finite number"
            )));
        }
    }
    if !options.padding.is_multiple_of(2) {
        return Err(DxfImportError::new(
            "padding must be an even number of millimetres",
        ));
    }
    Ok(())
}

fn has_dxf_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dxf"))
}

fn import_file(path: &Path) -> Result<ImportedPiece, DxfImportError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| DxfImportError::at(path, "DXF filename must be valid UTF-8"))?;
    let id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .ok_or_else(|| DxfImportError::at(path, "DXF filename must have a non-empty stem"))?
        .to_owned();
    let bytes = fs::read(path).map_err(|error| {
        DxfImportError::at(path, format!("DXF file could not be read: {error}"))
    })?;
    let text = String::from_utf8_lossy(&bytes);
    let entities =
        parse_entities(&text).map_err(|error| DxfImportError::at(path, error.message))?;
    let bounds = compute_bounds(&entities).ok_or_else(|| {
        DxfImportError::at(path, "no supported LINE, ARC, CIRCLE, or ELLIPSE geometry")
    })?;
    if ![
        bounds.min_x,
        bounds.min_y,
        bounds.max_x,
        bounds.max_y,
        bounds.width(),
        bounds.height(),
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        return Err(DxfImportError::at(
            path,
            "DXF geometry bounds must be finite",
        ));
    }

    let segments = source_segments(&entities, bounds)
        .map_err(|error| DxfImportError::at(path, error.message))?;
    if segments.is_empty() {
        return Err(DxfImportError::at(
            path,
            "supported DXF entities produced no usable geometry segments",
        ));
    }

    let real_width = bounds.width().ceil().max(1.0);
    let real_height = bounds.height().ceil().max(1.0);
    if !real_width.is_finite() || !real_height.is_finite() {
        return Err(DxfImportError::at(
            path,
            "DXF dimensions exceed the supported range",
        ));
    }

    Ok(ImportedPiece {
        id,
        real_width,
        real_height,
        segments,
    })
}

fn build_request(
    imported: Vec<ImportedPiece>,
    options: &ImportOptions,
) -> Result<EngineRequest, DxfImportError> {
    let side_padding = options.padding.div_ceil(2) as f64;
    let mut pieces = Vec::with_capacity(imported.len());
    let mut source_pieces = Vec::with_capacity(imported.len());

    for source in imported {
        let real_bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: source.real_width,
            height: source.real_height,
        };
        let interchangeability_key =
            geometry_key(&real_bounds, &source.segments, true, options.allow_mirror)?;
        let padded_width = source.real_width + side_padding * 2.0;
        let padded_height = source.real_height + side_padding * 2.0;
        pieces.push(PreparedPiece {
            id: format!("{}#1", source.id),
            source_piece_id: source.id.clone(),
            interchangeability_key: Some(interchangeability_key),
            real_bounds: real_bounds.clone(),
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
            allow_rotation: true,
            allow_mirror: options.allow_mirror,
            cut_row_ref: None,
        });
        source_pieces.push(SourcePiece {
            id: source.id.clone(),
            source_file_id: source.id.clone(),
            source_layer: None,
            label: source.id,
            real_bounds,
            geometry: SourceGeometry {
                entity_type: SourceGeometryEntityType::DxfShape,
                closed: true,
                segments: source.segments,
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
            optimizer: configurator_optimizer_settings(),
        },
        history_mode: HistoryMode::Off,
        diagnostic_trace_mode: DiagnosticTraceMode::Off,
    };
    request.validate().map_err(|error| {
        DxfImportError::new(format!("generated EngineRequest is invalid: {error}"))
    })?;
    Ok(request)
}

fn configurator_optimizer_settings() -> OptimizerSettings {
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
) -> Result<String, DxfImportError> {
    let bytes = serde_json::to_vec(&GeometryKey {
        real_bounds: GeometryKeyBounds {
            width: real_bounds.width,
            height: real_bounds.height,
        },
        segments,
    })
    .map_err(|error| {
        DxfImportError::new(format!("geometry identity could not be encoded: {error}"))
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

fn parse_entities(text: &str) -> Result<Entities, DxfImportError> {
    let lines = text.lines().map(str::trim).collect::<Vec<_>>();
    let mut pairs = Vec::new();
    let mut chunks = lines.chunks_exact(2);
    for chunk in &mut chunks {
        let Ok(code) = chunk[0].parse::<i32>() else {
            continue;
        };
        pairs.push((code, chunk[1]));
    }
    if !chunks.remainder().is_empty() {
        return Err(DxfImportError::new(
            "DXF contains an incomplete group-code pair",
        ));
    }

    let mut entities = Entities::default();
    let mut index = 0;
    let mut in_entities_section = false;
    while index < pairs.len() {
        let (code, value) = pairs[index];
        if code == 0 && value == "SECTION" {
            index += 1;
            let mut section_name = None;
            while index < pairs.len() && pairs[index].0 != 0 {
                if pairs[index].0 == 2 {
                    section_name = Some(pairs[index].1);
                }
                index += 1;
            }
            in_entities_section = section_name == Some("ENTITIES");
            continue;
        }
        if code == 0 && value == "ENDSEC" {
            in_entities_section = false;
            index += 1;
            continue;
        }
        if !in_entities_section
            || code != 0
            || !matches!(value, "LINE" | "ARC" | "CIRCLE" | "ELLIPSE")
        {
            index += 1;
            continue;
        }
        let entity_type = value;
        let start = index + 1;
        index = start;
        while index < pairs.len() && pairs[index].0 != 0 {
            index += 1;
        }
        let entity_pairs = &pairs[start..index];
        if is_paper_space(entity_pairs)? {
            continue;
        }
        match entity_type {
            "LINE" => entities
                .ordered
                .push(Entity::Line(parse_line(entity_pairs)?)),
            "ARC" => {
                let arc = parse_arc(entity_pairs)?;
                entities.ordered.push(Entity::Arc(arc));
            }
            "CIRCLE" => {
                let circle = parse_circle(entity_pairs)?;
                entities.ordered.push(Entity::Circle(circle));
            }
            "ELLIPSE" => {
                let ellipse = parse_ellipse(entity_pairs)?;
                if ellipse.major_axis.x.hypot(ellipse.major_axis.y) <= 0.0 {
                    return Err(DxfImportError::new(
                        "ELLIPSE major axis must have positive length",
                    ));
                }
                if ellipse.axis_ratio <= 0.0 {
                    return Err(DxfImportError::new(
                        "ELLIPSE axis ratio must be a positive finite number",
                    ));
                }
                let sweep = ellipse_sweep(&ellipse);
                if !sweep.is_finite() || sweep <= 0.0 || sweep > std::f64::consts::TAU {
                    return Err(DxfImportError::new(
                        "ELLIPSE parameter sweep must be within one positive turn",
                    ));
                }
                entities.ordered.push(Entity::Ellipse(ellipse));
            }
            _ => unreachable!(),
        }
    }
    Ok(entities)
}

fn is_paper_space(pairs: &[(i32, &str)]) -> Result<bool, DxfImportError> {
    match numeric(pairs, 67)? {
        None | Some(0.0) => Ok(false),
        Some(1.0) => Ok(true),
        Some(_) => Err(DxfImportError::new(
            "DXF group code 67 must be the model-space or paper-space indicator",
        )),
    }
}

fn numeric(pairs: &[(i32, &str)], code: i32) -> Result<Option<f64>, DxfImportError> {
    let Some((_, raw)) = pairs.iter().find(|(candidate, _)| *candidate == code) else {
        return Ok(None);
    };
    let value = raw
        .parse::<f64>()
        .map_err(|_| DxfImportError::new(format!("DXF group code {code} is not numeric")))?;
    if !value.is_finite() {
        return Err(DxfImportError::new(format!(
            "DXF group code {code} must be finite"
        )));
    }
    Ok(Some(value))
}

fn required_numeric(pairs: &[(i32, &str)], code: i32) -> Result<f64, DxfImportError> {
    numeric(pairs, code)?.ok_or_else(|| {
        DxfImportError::new(format!("DXF entity is missing required group code {code}"))
    })
}

fn parse_line(pairs: &[(i32, &str)]) -> Result<Line, DxfImportError> {
    Ok(Line {
        start: Point {
            x: required_numeric(pairs, 10)?,
            y: required_numeric(pairs, 20)?,
        },
        end: Point {
            x: required_numeric(pairs, 11)?,
            y: required_numeric(pairs, 21)?,
        },
    })
}

fn parse_arc(pairs: &[(i32, &str)]) -> Result<Arc, DxfImportError> {
    let radius = required_numeric(pairs, 40)?;
    if radius <= 0.0 {
        return Err(DxfImportError::new("ARC radius must be positive"));
    }
    let start_angle = required_numeric(pairs, 50)?;
    let raw_end_angle = required_numeric(pairs, 51)?;
    let raw_sweep = raw_end_angle - start_angle;
    let sweep = if raw_sweep <= 0.0 {
        raw_sweep + 360.0
    } else {
        raw_sweep
    };
    if !(sweep > 0.0 && sweep <= 360.0) {
        return Err(DxfImportError::new(
            "ARC angle sweep must be within one positive turn",
        ));
    }
    Ok(Arc {
        center: Point {
            x: required_numeric(pairs, 10)?,
            y: required_numeric(pairs, 20)?,
        },
        radius,
        start_angle,
        end_angle: start_angle + sweep,
    })
}

fn parse_circle(pairs: &[(i32, &str)]) -> Result<Circle, DxfImportError> {
    let radius = required_numeric(pairs, 40)?;
    if radius <= 0.0 {
        return Err(DxfImportError::new("CIRCLE radius must be positive"));
    }
    Ok(Circle {
        center: Point {
            x: required_numeric(pairs, 10)?,
            y: required_numeric(pairs, 20)?,
        },
        radius,
    })
}

fn parse_ellipse(pairs: &[(i32, &str)]) -> Result<Ellipse, DxfImportError> {
    Ok(Ellipse {
        center: Point {
            x: required_numeric(pairs, 10)?,
            y: required_numeric(pairs, 20)?,
        },
        major_axis: Point {
            x: required_numeric(pairs, 11)?,
            y: required_numeric(pairs, 21)?,
        },
        axis_ratio: required_numeric(pairs, 40)?,
        start_param: numeric(pairs, 41)?.unwrap_or(0.0),
        end_param: numeric(pairs, 42)?.unwrap_or(std::f64::consts::TAU),
    })
}

fn compute_bounds(entities: &Entities) -> Option<Bounds> {
    let mut points = Vec::new();
    for entity in &entities.ordered {
        match entity {
            Entity::Line(line) => points.extend([line.start, line.end]),
            Entity::Arc(arc) => {
                let span = (arc.end_angle - arc.start_angle).rem_euclid(360.0);
                let sweep = if span == 0.0 { 360.0 } else { span };
                let mut angles = vec![arc.start_angle, arc.start_angle + sweep];
                let mut angle = (arc.start_angle / 90.0).ceil() * 90.0;
                while angle <= arc.start_angle + sweep {
                    angles.push(angle);
                    angle += 90.0;
                }
                points.extend(angles.into_iter().map(|angle| point_on_arc(arc, angle)));
            }
            Entity::Circle(circle) => points.extend([
                Point {
                    x: circle.center.x - circle.radius,
                    y: circle.center.y - circle.radius,
                },
                Point {
                    x: circle.center.x + circle.radius,
                    y: circle.center.y + circle.radius,
                },
            ]),
            Entity::Ellipse(ellipse) => points.extend(ellipse_extreme_points(ellipse)),
        }
    }
    let first = *points.first()?;
    Some(points.into_iter().skip(1).fold(
        Bounds {
            min_x: first.x,
            min_y: first.y,
            max_x: first.x,
            max_y: first.y,
        },
        |bounds, point| Bounds {
            min_x: bounds.min_x.min(point.x),
            min_y: bounds.min_y.min(point.y),
            max_x: bounds.max_x.max(point.x),
            max_y: bounds.max_y.max(point.y),
        },
    ))
}

fn source_segments(
    entities: &Entities,
    bounds: Bounds,
) -> Result<Vec<SourceGeometrySegment>, DxfImportError> {
    let normalize = |point: Point| Point {
        x: point.x - bounds.min_x,
        y: point.y - bounds.min_y,
    };
    let mut segments = Vec::new();
    let mut ellipse_index = 0;

    for entity in &entities.ordered {
        match entity {
            Entity::Line(line) => {
                let start = normalize(line.start);
                let end = normalize(line.end);
                segments.push(SourceGeometrySegment::Line(SourceLineSegment {
                    x1: start.x,
                    y1: start.y,
                    x2: end.x,
                    y2: end.y,
                    bulge: None,
                    source_curve: None,
                }));
            }
            Entity::Arc(arc) => {
                let sampler_span = if arc.end_angle <= arc.start_angle {
                    arc.end_angle + 360.0 - arc.start_angle
                } else {
                    arc.end_angle - arc.start_angle
                };
                if sampler_span <= 0.0 || sampler_span > 360.0 {
                    let points = sample_arc(arc, CURVE_FALLBACK_SEGMENTS);
                    push_line_chain(&mut segments, &points, normalize, None);
                    continue;
                }
                let start_angle = arc.start_angle.rem_euclid(360.0);
                let end_angle = start_angle + sampler_span;
                let center = normalize(arc.center);
                let start = normalize(point_on_arc(arc, start_angle));
                let end = normalize(point_on_arc(arc, end_angle));
                segments.push(SourceGeometrySegment::Arc(SourceArcSegment {
                    x1: start.x,
                    y1: start.y,
                    x2: end.x,
                    y2: end.y,
                    cx: center.x,
                    cy: center.y,
                    radius: arc.radius,
                    start_angle,
                    end_angle,
                }));
            }
            Entity::Circle(circle) => {
                let center = normalize(circle.center);
                for (start_angle, end_angle) in [(0.0_f64, 180.0_f64), (180.0_f64, 360.0_f64)] {
                    let start_radians = start_angle.to_radians();
                    let end_radians = end_angle.to_radians();
                    segments.push(SourceGeometrySegment::Arc(SourceArcSegment {
                        x1: center.x + circle.radius * start_radians.cos(),
                        y1: center.y + circle.radius * start_radians.sin(),
                        x2: center.x + circle.radius * end_radians.cos(),
                        y2: center.y + circle.radius * end_radians.sin(),
                        cx: center.x,
                        cy: center.y,
                        radius: circle.radius,
                        start_angle,
                        end_angle,
                    }));
                }
            }
            Entity::Ellipse(ellipse) => {
                let major_length = ellipse.major_axis.x.hypot(ellipse.major_axis.y);
                let points = sample_ellipse(
                    ellipse,
                    ellipse_segment_count(major_length, ellipse.axis_ratio),
                );
                let center = normalize(ellipse.center);
                let curve = EllipseSource {
                    kind: EllipseSourceKind::Ellipse,
                    source_id: format!("ellipse:{ellipse_index}"),
                    cx: center.x,
                    cy: center.y,
                    major_axis_x: ellipse.major_axis.x,
                    major_axis_y: ellipse.major_axis.y,
                    axis_ratio: ellipse.axis_ratio,
                    start_angle: ellipse.start_param,
                    end_angle: ellipse.end_param,
                };
                ellipse_index += 1;
                push_line_chain(&mut segments, &points, normalize, Some(curve));
            }
        }
    }

    if segments.iter().any(|segment| match segment {
        SourceGeometrySegment::Line(line) => ![line.x1, line.y1, line.x2, line.y2]
            .into_iter()
            .all(f64::is_finite),
        SourceGeometrySegment::Arc(arc) => ![
            arc.x1,
            arc.y1,
            arc.x2,
            arc.y2,
            arc.cx,
            arc.cy,
            arc.radius,
            arc.start_angle,
            arc.end_angle,
        ]
        .into_iter()
        .all(f64::is_finite),
    }) {
        return Err(DxfImportError::new(
            "DXF conversion produced a non-finite geometry coordinate",
        ));
    }
    Ok(segments)
}

fn push_line_chain(
    segments: &mut Vec<SourceGeometrySegment>,
    points: &[Point],
    normalize: impl Fn(Point) -> Point,
    source_curve: Option<EllipseSource>,
) {
    for pair in points.windows(2) {
        let start = normalize(pair[0]);
        let end = normalize(pair[1]);
        segments.push(SourceGeometrySegment::Line(SourceLineSegment {
            x1: start.x,
            y1: start.y,
            x2: end.x,
            y2: end.y,
            bulge: None,
            source_curve: source_curve.clone(),
        }));
    }
}

fn point_on_arc(arc: &Arc, angle_degrees: f64) -> Point {
    let angle = angle_degrees.to_radians();
    Point {
        x: arc.center.x + arc.radius * angle.cos(),
        y: arc.center.y + arc.radius * angle.sin(),
    }
}

fn sample_arc(arc: &Arc, segments: usize) -> Vec<Point> {
    let start = arc.start_angle.to_radians();
    let end = arc.end_angle.to_radians();
    let span = if end <= start {
        end + std::f64::consts::TAU - start
    } else {
        end - start
    };
    (0..=segments)
        .map(|index| {
            let angle = start + span * index as f64 / segments as f64;
            Point {
                x: arc.center.x + arc.radius * angle.cos(),
                y: arc.center.y + arc.radius * angle.sin(),
            }
        })
        .collect()
}

fn sample_ellipse(ellipse: &Ellipse, segments: usize) -> Vec<Point> {
    let span = ellipse_sweep(ellipse);
    let minor_axis = Point {
        x: -ellipse.major_axis.y * ellipse.axis_ratio,
        y: ellipse.major_axis.x * ellipse.axis_ratio,
    };
    (0..=segments)
        .map(|index| {
            let parameter = ellipse.start_param + span * index as f64 / segments as f64;
            Point {
                x: ellipse.center.x
                    + ellipse.major_axis.x * parameter.cos()
                    + minor_axis.x * parameter.sin(),
                y: ellipse.center.y
                    + ellipse.major_axis.y * parameter.cos()
                    + minor_axis.y * parameter.sin(),
            }
        })
        .collect()
}

fn ellipse_sweep(ellipse: &Ellipse) -> f64 {
    if ellipse.end_param <= ellipse.start_param {
        ellipse.end_param + std::f64::consts::TAU - ellipse.start_param
    } else {
        ellipse.end_param - ellipse.start_param
    }
}

fn ellipse_extreme_points(ellipse: &Ellipse) -> Vec<Point> {
    let sweep = ellipse_sweep(ellipse);
    let end = ellipse.start_param + sweep;
    let minor_axis = Point {
        x: -ellipse.major_axis.y * ellipse.axis_ratio,
        y: ellipse.major_axis.x * ellipse.axis_ratio,
    };
    let mut parameters = vec![ellipse.start_param, end];
    for base in [
        minor_axis.x.atan2(ellipse.major_axis.x),
        minor_axis.x.atan2(ellipse.major_axis.x) + std::f64::consts::PI,
        minor_axis.y.atan2(ellipse.major_axis.y),
        minor_axis.y.atan2(ellipse.major_axis.y) + std::f64::consts::PI,
    ] {
        let cycle = ((ellipse.start_param - base) / std::f64::consts::TAU).ceil();
        let parameter = base + cycle * std::f64::consts::TAU;
        if parameter <= end {
            parameters.push(parameter);
        }
    }
    parameters
        .into_iter()
        .map(|parameter| Point {
            x: ellipse.center.x
                + ellipse.major_axis.x * parameter.cos()
                + minor_axis.x * parameter.sin(),
            y: ellipse.center.y
                + ellipse.major_axis.y * parameter.cos()
                + minor_axis.y * parameter.sin(),
        })
        .collect()
}

fn ellipse_segment_count(major_length: f64, axis_ratio: f64) -> usize {
    let long_axis = major_length * axis_ratio.max(1.0);
    let short_axis = major_length * axis_ratio.min(1.0);
    if !(long_axis > 0.0 && short_axis > 0.0) {
        return MIN_ELLIPSE_SEGMENTS;
    }
    let worst_radius = long_axis * long_axis / short_axis;
    if worst_radius <= ELLIPSE_SAG_TOLERANCE_MM {
        return MIN_ELLIPSE_SEGMENTS;
    }
    let max_chord_angle = 2.0 * (1.0 - ELLIPSE_SAG_TOLERANCE_MM / worst_radius).acos();
    ((std::f64::consts::TAU / max_chord_angle).ceil() as usize)
        .clamp(MIN_ELLIPSE_SEGMENTS, MAX_ELLIPSE_SEGMENTS)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use polygon_nesting_protocol::{EngineProfile, SourceGeometrySegment};

    use super::{compute_bounds, import_directory, parse_entities, ImportOptions};

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

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("polygon-dxf-{label}-{unique}"));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        directory
    }

    #[test]
    fn imports_supported_entities_and_emits_a_valid_request() {
        let directory = temporary_directory("entities");
        fs::write(
            directory.join("part.dxf"),
            "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n20\n21\n0\n0\nARC\n10\n20\n20\n20\n40\n20\n50\n270\n51\n90\n0\nCIRCLE\n10\n60\n20\n20\n40\n10\n0\nELLIPSE\n10\n100\n20\n20\n11\n20\n21\n0\n40\n0.5\n41\n0\n42\n6.283185307179586\n0\nENDSEC\n0\nEOF\n",
        )
        .expect("fixture should be written");

        let request = import_directory(&directory, &options()).expect("DXF should import");

        assert_eq!(request.pieces.len(), 1);
        assert_eq!(request.source_pieces.len(), 1);
        let segments = &request.source_pieces[0].geometry.segments;
        assert!(segments
            .iter()
            .any(|segment| matches!(segment, SourceGeometrySegment::Arc(_))));
        assert!(segments.iter().any(|segment| matches!(
            segment,
            SourceGeometrySegment::Line(line) if line.source_curve.is_some()
        )));
        request
            .validate()
            .expect("generated request should validate");
        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn directory_order_cannot_change_request_bytes() {
        let first = temporary_directory("order-first");
        let second = temporary_directory("order-second");
        let rectangle = |width: u32| {
            format!(
                "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n{width}\n21\n0\n0\nLINE\n10\n{width}\n20\n0\n11\n{width}\n21\n10\n0\nLINE\n10\n{width}\n20\n10\n11\n0\n21\n10\n0\nLINE\n10\n0\n20\n10\n11\n0\n21\n0\n0\nENDSEC\n0\nEOF\n"
            )
        };
        fs::write(first.join("b.dxf"), rectangle(20)).expect("fixture should be written");
        fs::write(first.join("a.dxf"), rectangle(10)).expect("fixture should be written");
        fs::write(second.join("a.dxf"), rectangle(10)).expect("fixture should be written");
        fs::write(second.join("b.dxf"), rectangle(20)).expect("fixture should be written");

        let first_request = import_directory(&first, &options()).expect("first import should work");
        let second_request =
            import_directory(&second, &options()).expect("second import should work");

        assert_eq!(
            serde_json::to_vec(&first_request).expect("request should encode"),
            serde_json::to_vec(&second_request).expect("request should encode")
        );
        fs::remove_dir_all(first).expect("temporary directory should be removed");
        fs::remove_dir_all(second).expect("temporary directory should be removed");
    }

    #[test]
    fn preserves_model_space_entity_order_and_ignores_block_definitions() {
        let directory = temporary_directory("sections-and-order");
        fs::write(
            directory.join("part.dxf"),
            "0\nSECTION\n2\nBLOCKS\n0\nLINE\n10\n0\n20\n0\n11\n1000\n21\n1000\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nARC\n10\n0\n20\n0\n40\n10\n50\n0\n51\n90\n0\nLINE\n10\n0\n20\n10\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n",
        )
        .expect("fixture should be written");

        let request = import_directory(&directory, &options()).expect("DXF should import");
        let source = &request.source_pieces[0];
        assert_eq!(source.real_bounds.width, 10.0);
        assert_eq!(source.real_bounds.height, 10.0);
        assert!(matches!(
            source.geometry.segments.as_slice(),
            [
                SourceGeometrySegment::Arc(_),
                SourceGeometrySegment::Line(_)
            ]
        ));

        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }

    #[test]
    fn ignores_supported_paper_space_entities() {
        let entities = parse_entities(
            "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n67\n1\n10\n500\n20\n500\n40\n100\n0\nLINE\n67\n0\n10\n0\n20\n0\n11\n10\n21\n10\n0\nENDSEC\n0\nEOF\n",
        )
        .expect("model-space geometry should parse");
        let bounds = compute_bounds(&entities).expect("model-space line should have bounds");

        assert_eq!(bounds.width(), 10.0);
        assert_eq!(bounds.height(), 10.0);
        assert!(matches!(
            entities.ordered.as_slice(),
            [super::Entity::Line(_)]
        ));
    }

    #[test]
    fn rejects_missing_required_entity_fields_and_nonpositive_radii() {
        for (fixture, expected) in [
            (
                "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n10\n0\nENDSEC\n0\nEOF\n",
                "required group code 21",
            ),
            (
                "0\nSECTION\n2\nENTITIES\n0\nARC\n10\n0\n20\n0\n40\n0\n50\n0\n51\n90\n0\nENDSEC\n0\nEOF\n",
                "ARC radius must be positive",
            ),
            (
                "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n0\n20\n0\n40\n-1\n0\nENDSEC\n0\nEOF\n",
                "CIRCLE radius must be positive",
            ),
            (
                "0\nSECTION\n2\nENTITIES\n0\nELLIPSE\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n",
                "required group code 40",
            ),
        ] {
            let error = parse_entities(fixture).expect_err("malformed entity should be rejected");
            assert!(
                error.to_string().contains(expected),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn rejects_arc_sweeps_larger_than_one_turn() {
        let error = parse_entities(
            "0\nSECTION\n2\nENTITIES\n0\nARC\n10\n0\n20\n0\n40\n10\n50\n0\n51\n450\n0\nENDSEC\n0\nEOF\n",
        )
        .expect_err("overlong arc should be rejected");

        assert_eq!(
            error.to_string(),
            "ARC angle sweep must be within one positive turn"
        );
    }

    #[test]
    fn ellipse_bounds_include_exact_rotated_analytic_extrema() {
        let entities = parse_entities(
            "0\nSECTION\n2\nENTITIES\n0\nELLIPSE\n10\n0\n20\n0\n11\n3\n21\n4\n40\n0.5\n41\n0\n42\n6.283185307179586\n0\nENDSEC\n0\nEOF\n",
        )
        .expect("ellipse should parse");
        let bounds = compute_bounds(&entities).expect("ellipse should have bounds");

        assert!((bounds.width() - 2.0 * 13.0_f64.sqrt()).abs() < 1e-12);
        assert!((bounds.height() - 2.0 * 18.25_f64.sqrt()).abs() < 1e-12);
    }
}
