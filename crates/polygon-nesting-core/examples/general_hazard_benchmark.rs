#![recursion_limit = "512"]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

use polygon_nesting_core::domain::ImportedPiece;
use polygon_nesting_core::geometry::general_polygon::PolygonSet;
use polygon_nesting_core::geometry::general_source::polygon_set_from_imported_piece;
use polygon_nesting_core::search::general_fast::{GeneralFastPiece, GeneralFastSettings};
use polygon_nesting_core::search::general_hazard::{
    hazard_collision_expansion_mm, hazard_sheet_inset_mm, GeneralHazardPose, GeneralHazardQuery,
    JaguaHazardIndex,
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

const UPDATE_INTERVAL: usize = 4_096;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    sheet: Sheet,
    pieces: Vec<RequestPiece>,
    source_pieces: Vec<ImportedPiece>,
}

#[derive(Deserialize)]
struct Sheet {
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestPiece {
    id: String,
    source_piece_id: String,
    allow_rotation: bool,
    #[serde(default = "default_true")]
    allow_mirror: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Baseline {
    used_long_axis_depth_mm: f64,
    pair_clearance_mm: f64,
    sheet_edge_clearance_mm: f64,
    clearance_safety_margin_mm: f64,
    flattening_sag_tolerance_mm: f64,
    placements: Vec<BaselinePlacement>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselinePlacement {
    piece_id: String,
    rotation_deg: f64,
    mirrored: bool,
    translate_short_axis: f64,
    translate_long_axis: f64,
}

struct OwnedPiece {
    id: String,
    polygon: PolygonSet,
    allow_rotation: bool,
    allow_mirror: bool,
}

#[derive(Clone)]
struct RecordedQuery {
    moving_piece_id: usize,
    pose: GeneralHazardPose,
    result: GeneralHazardQuery,
    committed_after: bool,
}

#[derive(Default)]
struct Agreement {
    queries: usize,
    pair_set_mismatches: usize,
    false_positive_pairs: usize,
    false_negative_pairs: usize,
    false_negative_pairs_in_bounds: usize,
    false_positive_pairs_in_bounds: usize,
    boundary_mismatches: usize,
    max_false_negative_area_mm2: f64,
    max_false_negative_area_in_bounds_mm2: f64,
    total_false_negative_area_mm2: f64,
    max_false_positive_gap_mm: f64,
    total_false_positive_gap_mm: f64,
    false_positive_pairs_outside_numeric_band: usize,
    max_false_positive_band_ratio: f64,
    max_boundary_mismatch_depth_mm: f64,
    mismatch_samples: Vec<serde_json::Value>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let request_path = arguments.next().ok_or(
        "usage: general_hazard_benchmark REQUEST.json BASELINE.json [queries] [agreement-queries] [seed]",
    )?;
    let baseline_path = arguments.next().ok_or(
        "usage: general_hazard_benchmark REQUEST.json BASELINE.json [queries] [agreement-queries] [seed]",
    )?;
    let query_count = parse_optional(&mut arguments, 100_000)?;
    let agreement_query_count = parse_optional(&mut arguments, query_count.min(10_000))?;
    let seed = arguments
        .next()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(0);
    if query_count == 0 || agreement_query_count > query_count || arguments.next().is_some() {
        return Err("query counts must be valid and no extra arguments are accepted".into());
    }

    let request: Request = serde_json::from_slice(&fs::read(Path::new(&request_path))?)?;
    let baseline: Baseline = serde_json::from_slice(&fs::read(Path::new(&baseline_path))?)?;
    let owned = load_pieces(&request, baseline.flattening_sag_tolerance_mm)?;
    let pieces = owned
        .iter()
        .map(|piece| GeneralFastPiece {
            id: &piece.id,
            polygon: &piece.polygon,
            allow_rotation: piece.allow_rotation,
            allow_mirror: piece.allow_mirror,
        })
        .collect::<Vec<_>>();
    let mut settings = GeneralFastSettings::deterministic_test(
        request.sheet.width.min(request.sheet.height),
        request.sheet.width.max(request.sheet.height),
    );
    settings.total_padding_mm = baseline.pair_clearance_mm;
    settings.sheet_edge_clearance_mm = Some(baseline.sheet_edge_clearance_mm);
    settings.clearance_safety_margin_mm = baseline.clearance_safety_margin_mm;
    settings.flattening_sag_tolerance_mm = baseline.flattening_sag_tolerance_mm;
    let initial_poses = ordered_poses(&owned, &baseline.placements)?;

    let preprocessing_started = Instant::now();
    let mut index = JaguaHazardIndex::new(
        &pieces,
        settings,
        baseline.used_long_axis_depth_mm,
        &initial_poses,
    )?;
    let preprocessing_ms = preprocessing_started.elapsed().as_secs_f64() * 1_000.0;
    let mut poses = initial_poses.clone();
    let mut rng = SplitMix64::new(seed);
    let mut agreement_records = Vec::with_capacity(agreement_query_count);
    let mut fingerprint = Sha256::new();
    let query_started = Instant::now();
    for query_index in 0..query_count {
        let moving_piece_id = (rng.next_u64() as usize) % pieces.len();
        let candidate = sample_pose(
            &mut rng,
            query_index,
            poses[moving_piece_id],
            pieces[moving_piece_id],
            settings.sheet_short_axis_mm,
            baseline.used_long_axis_depth_mm,
        );
        let result = index.query(moving_piece_id, candidate, None)?;
        let committed_after = (query_index + 1) % UPDATE_INTERVAL == 0;
        hash_query(
            &mut fingerprint,
            moving_piece_id,
            candidate,
            &result,
            committed_after,
        );
        if query_index < agreement_query_count {
            agreement_records.push(RecordedQuery {
                moving_piece_id,
                pose: candidate,
                result: result.clone(),
                committed_after,
            });
        }
        if committed_after {
            index.commit(moving_piece_id, candidate)?;
            poses[moving_piece_id] = candidate;
        }
    }
    let query_elapsed_ms = query_started.elapsed().as_secs_f64() * 1_000.0;
    let fingerprint = format!("{:x}", fingerprint.finalize());
    let counters = index.counters();

    let oracle_started = Instant::now();
    let agreement = compare_with_f64_oracle(
        &owned,
        settings,
        baseline.used_long_axis_depth_mm,
        &initial_poses,
        &agreement_records,
    )?;
    let oracle_elapsed_ms = oracle_started.elapsed().as_secs_f64() * 1_000.0;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "request": request_path,
            "baseline": baseline_path,
            "pieces": pieces.len(),
            "queryCount": query_count,
            "agreementQueryCount": agreement.queries,
            "updateInterval": UPDATE_INTERVAL,
            "seed": seed,
            "preprocessingMs": preprocessing_ms,
            "queryElapsedMs": query_elapsed_ms,
            "completeQueriesPerSecond": query_count as f64 / (query_elapsed_ms / 1_000.0),
            "oracleElapsedMsUntimed": oracle_elapsed_ms,
            "fingerprint": fingerprint,
            "counters": {
                "failFastQueries": counters.fail_fast_queries,
                "prunedQueries": counters.pruned_queries,
                "completeQueries": counters.complete_queries,
                "collectedPieceIds": counters.collected_piece_ids,
                "hazardUpdates": counters.hazard_updates,
                "indexRebuilds": counters.index_rebuilds,
            },
            "agreement": {
                "pairSetMismatches": agreement.pair_set_mismatches,
                "falsePositivePairs": agreement.false_positive_pairs,
                "falsePositivePairsInBounds": agreement.false_positive_pairs_in_bounds,
                "falseNegativePairs": agreement.false_negative_pairs,
                "falseNegativePairsInBounds": agreement.false_negative_pairs_in_bounds,
                "boundaryMismatches": agreement.boundary_mismatches,
                "maxFalseNegativeAreaMm2": agreement.max_false_negative_area_mm2,
                "maxFalseNegativeAreaInBoundsMm2": agreement.max_false_negative_area_in_bounds_mm2,
                "totalFalseNegativeAreaMm2": agreement.total_false_negative_area_mm2,
                "maxFalsePositiveGapMm": agreement.max_false_positive_gap_mm,
                "totalFalsePositiveGapMm": agreement.total_false_positive_gap_mm,
                "falsePositivePairsOutsideNumericBand": agreement.false_positive_pairs_outside_numeric_band,
                "maxFalsePositiveBandRatio": agreement.max_false_positive_band_ratio,
                "maxBoundaryMismatchDepthMm": agreement.max_boundary_mismatch_depth_mm,
                "mismatchSamples": agreement.mismatch_samples,
            }
        }))?
    );
    Ok(())
}

fn load_pieces(
    request: &Request,
    flattening_sag_tolerance_mm: f64,
) -> Result<Vec<OwnedPiece>, Box<dyn std::error::Error>> {
    let mut sources = BTreeMap::new();
    for source in &request.source_pieces {
        if sources.insert(source.id.as_str(), source).is_some() {
            return Err(format!("duplicate source piece ID: {}", source.id).into());
        }
    }
    let normalize_axes = request.sheet.width >= request.sheet.height;
    request
        .pieces
        .iter()
        .map(|piece| {
            let source = sources
                .get(piece.source_piece_id.as_str())
                .ok_or_else(|| format!("missing source piece {}", piece.source_piece_id))?;
            let polygon = normalize_polygon_axes(
                polygon_set_from_imported_piece(source, flattening_sag_tolerance_mm)?,
                normalize_axes,
            )?;
            Ok(OwnedPiece {
                id: piece.id.clone(),
                polygon,
                allow_rotation: piece.allow_rotation,
                allow_mirror: piece.allow_mirror,
            })
        })
        .collect()
}

fn ordered_poses(
    pieces: &[OwnedPiece],
    placements: &[BaselinePlacement],
) -> Result<Vec<GeneralHazardPose>, Box<dyn std::error::Error>> {
    let by_id = placements
        .iter()
        .map(|placement| (placement.piece_id.as_str(), placement))
        .collect::<BTreeMap<_, _>>();
    pieces
        .iter()
        .map(|piece| {
            let placement = by_id
                .get(piece.id.as_str())
                .ok_or_else(|| format!("baseline is missing piece {}", piece.id))?;
            Ok(GeneralHazardPose {
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_short_axis: placement.translate_short_axis,
                translate_long_axis: placement.translate_long_axis,
            })
        })
        .collect()
}

fn sample_pose(
    rng: &mut SplitMix64,
    query_index: usize,
    current: GeneralHazardPose,
    piece: GeneralFastPiece<'_>,
    sheet_short_axis_mm: f64,
    strip_depth_mm: f64,
) -> GeneralHazardPose {
    let rotation_deg = if piece.allow_rotation {
        let angle_ordinal = query_index % 257;
        let cycle_phase = (query_index / 257 % 17) as f64 * 0.000_001;
        ((angle_ordinal as f64 + 0.5) * 360.0 / 257.0 + cycle_phase).rem_euclid(360.0)
    } else {
        current.rotation_deg
    };
    let mirrored = if piece.allow_mirror {
        rng.next_u64() & 1 == 1
    } else {
        current.mirrored
    };
    GeneralHazardPose {
        rotation_deg,
        mirrored,
        translate_short_axis: (current.translate_short_axis + rng.range(-80.0, 80.0))
            .clamp(-40.0, sheet_short_axis_mm + 40.0),
        translate_long_axis: (current.translate_long_axis + rng.range(-80.0, 80.0))
            .clamp(-40.0, strip_depth_mm + 40.0),
    }
}

fn compare_with_f64_oracle(
    pieces: &[OwnedPiece],
    settings: GeneralFastSettings,
    strip_depth_mm: f64,
    initial_poses: &[GeneralHazardPose],
    records: &[RecordedQuery],
) -> Result<Agreement, Box<dyn std::error::Error>> {
    let expansion_mm = hazard_collision_expansion_mm(settings);
    let inset_mm = hazard_sheet_inset_mm(settings);
    let mut accepted = pieces
        .iter()
        .zip(initial_poses)
        .map(|(piece, pose)| collision_polygon(piece, *pose, expansion_mm))
        .collect::<Result<Vec<_>, _>>()?;
    let mut agreement = Agreement::default();
    for record in records {
        let moving = collision_polygon(&pieces[record.moving_piece_id], record.pose, expansion_mm)?;
        let bounds = moving.bounds().ok_or("oracle produced empty geometry")?;
        let oracle_boundary = bounds.min_x < inset_mm
            || bounds.min_y < inset_mm
            || bounds.max_x > settings.sheet_short_axis_mm - inset_mm
            || bounds.max_y > strip_depth_mm - inset_mm;
        let mut oracle_pairs = BTreeMap::new();
        for (fixed_id, fixed) in accepted.iter().enumerate() {
            if fixed_id != record.moving_piece_id {
                let area_mm2 = moving.intersection_area_mm2(fixed)?;
                if area_mm2 > 0.0 {
                    oracle_pairs.insert(fixed_id, area_mm2);
                }
            }
        }
        let GeneralHazardQuery::Complete {
            boundary,
            colliding_piece_ids,
        } = &record.result
        else {
            return Err("agreement stream must contain only complete queries".into());
        };
        let backend_pairs = colliding_piece_ids.iter().copied().collect::<BTreeSet<_>>();
        let oracle_pair_ids = oracle_pairs.keys().copied().collect::<BTreeSet<_>>();
        if backend_pairs != oracle_pair_ids {
            agreement.pair_set_mismatches += 1;
            let false_positive_count = backend_pairs.difference(&oracle_pair_ids).count();
            agreement.false_positive_pairs += false_positive_count;
            if !oracle_boundary {
                agreement.false_positive_pairs_in_bounds += false_positive_count;
            }
            for fixed_id in backend_pairs.difference(&oracle_pair_ids).copied() {
                let gap_mm = polygon_distance_mm(&moving, &accepted[fixed_id]);
                let numeric_band_mm = transform_ambiguity_band_mm(&moving, &accepted[fixed_id]);
                agreement.total_false_positive_gap_mm += gap_mm;
                agreement.max_false_positive_gap_mm =
                    agreement.max_false_positive_gap_mm.max(gap_mm);
                if gap_mm > numeric_band_mm {
                    agreement.false_positive_pairs_outside_numeric_band += 1;
                }
                if numeric_band_mm > 0.0 {
                    agreement.max_false_positive_band_ratio = agreement
                        .max_false_positive_band_ratio
                        .max(gap_mm / numeric_band_mm);
                }
                if agreement.mismatch_samples.len() < 12 {
                    agreement.mismatch_samples.push(json!({
                        "kind": "falsePositivePair",
                        "movingPieceId": record.moving_piece_id,
                        "fixedPieceId": fixed_id,
                        "rotationDeg": record.pose.rotation_deg,
                        "mirrored": record.pose.mirrored,
                        "translateShortAxis": record.pose.translate_short_axis,
                        "translateLongAxis": record.pose.translate_long_axis,
                        "oracleBoundary": oracle_boundary,
                        "gapMm": gap_mm,
                        "numericBandMm": numeric_band_mm,
                    }));
                }
            }
            for fixed_id in oracle_pair_ids.difference(&backend_pairs).copied() {
                let area_mm2 = oracle_pairs[&fixed_id];
                agreement.false_negative_pairs += 1;
                agreement.total_false_negative_area_mm2 += area_mm2;
                agreement.max_false_negative_area_mm2 =
                    agreement.max_false_negative_area_mm2.max(area_mm2);
                if !oracle_boundary {
                    agreement.false_negative_pairs_in_bounds += 1;
                    agreement.max_false_negative_area_in_bounds_mm2 = agreement
                        .max_false_negative_area_in_bounds_mm2
                        .max(area_mm2);
                }
                if agreement.mismatch_samples.len() < 12 {
                    agreement.mismatch_samples.push(json!({
                        "kind": "falseNegativePair",
                        "movingPieceId": record.moving_piece_id,
                        "fixedPieceId": fixed_id,
                        "rotationDeg": record.pose.rotation_deg,
                        "mirrored": record.pose.mirrored,
                        "translateShortAxis": record.pose.translate_short_axis,
                        "translateLongAxis": record.pose.translate_long_axis,
                        "intersectionAreaMm2": area_mm2,
                    }));
                }
            }
        }
        if *boundary != oracle_boundary {
            agreement.boundary_mismatches += 1;
            let mismatch_depth_mm = (inset_mm - bounds.min_x)
                .max(inset_mm - bounds.min_y)
                .max(bounds.max_x - (settings.sheet_short_axis_mm - inset_mm))
                .max(bounds.max_y - (strip_depth_mm - inset_mm))
                .max(0.0);
            agreement.max_boundary_mismatch_depth_mm = agreement
                .max_boundary_mismatch_depth_mm
                .max(mismatch_depth_mm);
            if agreement.mismatch_samples.len() < 12 {
                agreement.mismatch_samples.push(json!({
                    "kind": "boundaryMismatch",
                    "movingPieceId": record.moving_piece_id,
                    "rotationDeg": record.pose.rotation_deg,
                    "mirrored": record.pose.mirrored,
                    "translateShortAxis": record.pose.translate_short_axis,
                    "translateLongAxis": record.pose.translate_long_axis,
                    "oracleBoundary": oracle_boundary,
                    "backendBoundary": boundary,
                    "violationDepthMm": mismatch_depth_mm,
                }));
            }
        }
        agreement.queries += 1;
        if record.committed_after {
            accepted[record.moving_piece_id] = moving;
        }
    }
    Ok(agreement)
}

fn collision_polygon(
    piece: &OwnedPiece,
    pose: GeneralHazardPose,
    expansion_mm: f64,
) -> Result<PolygonSet, Box<dyn std::error::Error>> {
    Ok(piece
        .polygon
        .transformed(
            pose.rotation_deg,
            pose.mirrored,
            pose.translate_short_axis,
            pose.translate_long_axis,
        )?
        .offset(expansion_mm)?)
}

fn polygon_distance_mm(first: &PolygonSet, second: &PolygonSet) -> f64 {
    let mut minimum = f64::INFINITY;
    for first_region in first.regions() {
        for second_region in second.regions() {
            minimum = minimum.min(ring_distance_mm(
                first_region.outer.points(),
                second_region.outer.points(),
            ));
        }
    }
    minimum
}

fn transform_ambiguity_band_mm(first: &PolygonSet, second: &PolygonSet) -> f64 {
    let maximum_coordinate = first
        .bounds()
        .into_iter()
        .chain(second.bounds())
        .flat_map(|bounds| {
            [
                bounds.min_x.abs(),
                bounds.min_y.abs(),
                bounds.max_x.abs(),
                bounds.max_y.abs(),
            ]
        })
        .fold(0.0_f64, f64::max);
    8.0 * f32_ulp_mm(maximum_coordinate)
}

fn f32_ulp_mm(value: f64) -> f64 {
    let converted = value as f32;
    if converted == 0.0 {
        return f32::from_bits(1) as f64;
    }
    let bits = converted.abs().to_bits();
    (f32::from_bits(bits + 1) - converted.abs()) as f64
}

fn ring_distance_mm(
    first: &[polygon_nesting_core::domain::IrregularPoint],
    second: &[polygon_nesting_core::domain::IrregularPoint],
) -> f64 {
    let mut minimum = f64::INFINITY;
    for first_index in 0..first.len() {
        let first_start = first[first_index];
        let first_end = first[(first_index + 1) % first.len()];
        for second_index in 0..second.len() {
            let second_start = second[second_index];
            let second_end = second[(second_index + 1) % second.len()];
            if segments_intersect(first_start, first_end, second_start, second_end) {
                return 0.0;
            }
            minimum = minimum
                .min(point_segment_distance(
                    first_start,
                    second_start,
                    second_end,
                ))
                .min(point_segment_distance(first_end, second_start, second_end))
                .min(point_segment_distance(second_start, first_start, first_end))
                .min(point_segment_distance(second_end, first_start, first_end));
        }
    }
    minimum
}

fn segments_intersect(
    first_start: polygon_nesting_core::domain::IrregularPoint,
    first_end: polygon_nesting_core::domain::IrregularPoint,
    second_start: polygon_nesting_core::domain::IrregularPoint,
    second_end: polygon_nesting_core::domain::IrregularPoint,
) -> bool {
    let cross = |a: polygon_nesting_core::domain::IrregularPoint,
                 b: polygon_nesting_core::domain::IrregularPoint,
                 c: polygon_nesting_core::domain::IrregularPoint| {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    };
    let first_a = cross(first_start, first_end, second_start);
    let first_b = cross(first_start, first_end, second_end);
    let second_a = cross(second_start, second_end, first_start);
    let second_b = cross(second_start, second_end, first_end);
    first_a.signum() != first_b.signum() && second_a.signum() != second_b.signum()
}

fn point_segment_distance(
    point: polygon_nesting_core::domain::IrregularPoint,
    start: polygon_nesting_core::domain::IrregularPoint,
    end: polygon_nesting_core::domain::IrregularPoint,
) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return (point.x - start.x).hypot(point.y - start.y);
    }
    let parameter =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    let closest_x = start.x + parameter * dx;
    let closest_y = start.y + parameter * dy;
    (point.x - closest_x).hypot(point.y - closest_y)
}

fn hash_query(
    hasher: &mut Sha256,
    moving_piece_id: usize,
    pose: GeneralHazardPose,
    result: &GeneralHazardQuery,
    committed_after: bool,
) {
    hasher.update((moving_piece_id as u64).to_le_bytes());
    hasher.update(pose.rotation_deg.to_bits().to_le_bytes());
    hasher.update([pose.mirrored as u8]);
    hasher.update(pose.translate_short_axis.to_bits().to_le_bytes());
    hasher.update(pose.translate_long_axis.to_bits().to_le_bytes());
    hasher.update([committed_after as u8]);
    match result {
        GeneralHazardQuery::Pruned { lower_bound } => {
            hasher.update([0]);
            hasher.update((*lower_bound as u64).to_le_bytes());
        }
        GeneralHazardQuery::Complete {
            boundary,
            colliding_piece_ids,
        } => {
            hasher.update([1, *boundary as u8]);
            for piece_id in colliding_piece_ids {
                hasher.update((*piece_id as u64).to_le_bytes());
            }
        }
    }
}

fn normalize_polygon_axes(
    polygon: PolygonSet,
    rotate_physical_to_normalized: bool,
) -> Result<PolygonSet, Box<dyn std::error::Error>> {
    if !rotate_physical_to_normalized {
        return Ok(polygon);
    }
    let rotated = polygon.transformed(270.0, false, 0.0, 0.0)?;
    let bounds = rotated
        .bounds()
        .ok_or("cannot normalize empty source geometry")?;
    Ok(rotated.translated(-bounds.min_x, -bounds.min_y)?)
}

fn parse_optional(
    arguments: &mut impl Iterator<Item = String>,
    default: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(default))
}

fn default_true() -> bool {
    true
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn range(&mut self, minimum: f64, maximum: f64) -> f64 {
        let unit = (self.next_u64() >> 11) as f64 / ((1_u64 << 53) as f64);
        minimum + (maximum - minimum) * unit
    }
}
