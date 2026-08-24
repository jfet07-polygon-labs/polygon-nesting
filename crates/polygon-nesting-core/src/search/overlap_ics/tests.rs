//! The module's unit vectors — the FAST tier's second stage.
//!
//! Sol review 14 Round 2 §3 lists eleven required vectors and this file carries
//! all of them, plus the two independent oracles the convergence retained:
//! the crate's existing SAT for the overlapping convex case, and the nine-point
//! triangle Minkowski hull for the triangle case. Both oracles live here and
//! nowhere else; `contact.rs` never calls either.

use crate::domain::IrregularPoint;
use crate::geometry::general_polygon::PolygonSet;
use crate::search::general_fast::{GeneralFastPiece, GeneralFastSettings};
use crate::validation::sat::measure_convex_sat_penetration;

use super::contact::convex_cell_gap;
use super::decomposition::{decompose, ear_clip, is_convex, signed_area, source_ring};
use super::descent::{counter_hash, rotated_halton, DescentConfig};
use super::diagnostics::WorkVector;
use super::energy::{fold, rebuild_all, rebuild_piece_rows};
use super::publish::{placement_fingerprint, publication_settings, PublicationLimits};
use super::state::{
    build_geometry, pair_count, transform_piece, Contract, EdgeRow, IcsState, PairRow, PieceSource,
    Pose,
};
use super::{Engine, IcsConfig};
use crate::search::overlap_ics_meter::strike_meter::{
    Patience, StrikeConfig, COMPRESS_WORK_QUANTUM, EXPLORE_WORK_QUANTUM,
};

fn polygon(points: &[[f64; 2]]) -> PolygonSet {
    PolygonSet::from_outer(
        points
            .iter()
            .map(|point| IrregularPoint::new(point[0], point[1]))
            .collect(),
    )
    .expect("a test polygon")
}

fn square(x: f64, y: f64, size: f64) -> Vec<[f64; 2]> {
    vec![
        [x, y],
        [x + size, y],
        [x + size, y + size],
        [x, y + size],
    ]
}

fn triangle(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> [[f64; 2]; 3] {
    [a, b, c]
}

/// Oracle two: the independent nine-point Minkowski hull for the triangle
/// path.
///
/// `C = conv{a_u - b_v}` over the nine vertex differences; the signed distance
/// is `dist(0, C)` outside and `-dist(0, ∂C)` inside. This is Sol review 14
/// Round 1's construction, retained by the convergence as the small-cell test
/// oracle and never as the hot path. It is `pub` so the module's own tests and
/// the corpus driver can differential it against [`convex_cell_gap`]; nothing
/// in the descent calls it.
fn triangle_minkowski_signed_distance(a: &[[f64; 2]; 3], b: &[[f64; 2]; 3]) -> f64 {
    let mut difference = [[0.0f64; 2]; 9];
    let mut count = 0;
    for u in a.iter() {
        for v in b.iter() {
            difference[count] = [u[0] - v[0], u[1] - v[1]];
            count += 1;
        }
    }
    let built = convex_hull_9(&difference);
    let hull = &built.0[..built.1];
    if hull.len() < 3 {
        // Degenerate difference body: the two cells are collinear or a point.
        let mut best = f64::INFINITY;
        for index in 0..hull.len() {
            let first = hull[index];
            let second = hull[(index + 1) % hull.len().max(1)];
            best = best.min(origin_to_segment(first, second));
        }
        return if best.is_finite() { best } else { 0.0 };
    }
    let mut inside = true;
    for index in 0..hull.len() {
        let first = hull[index];
        let second = hull[(index + 1) % hull.len()];
        let side = (second[0] - first[0]) * (0.0 - first[1])
            - (second[1] - first[1]) * (0.0 - first[0]);
        if side < 0.0 {
            inside = false;
            break;
        }
    }
    let mut boundary = f64::INFINITY;
    for index in 0..hull.len() {
        let first = hull[index];
        let second = hull[(index + 1) % hull.len()];
        boundary = boundary.min(origin_to_segment(first, second));
    }
    if inside {
        -boundary
    } else {
        boundary
    }
}

/// A fixed-capacity monotone-chain hull of the nine point differences.
fn convex_hull_9(points: &[[f64; 2]; 9]) -> ([[f64; 2]; 18], usize) {
    let mut sorted = *points;
    sorted.sort_by(|left, right| {
        left[0]
            .partial_cmp(&right[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                left[1]
                    .partial_cmp(&right[1])
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    let count = sorted.len();
    let mut hull = [[0.0f64; 2]; 18];
    let mut length = 0usize;
    let push = |hull: &mut [[f64; 2]; 18], length: &mut usize, floor: usize, point: [f64; 2]| {
        while *length >= floor + 2 {
            let a = hull[*length - 2];
            let b = hull[*length - 1];
            let turn = (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0]);
            if turn > 0.0 {
                break;
            }
            *length -= 1;
        }
        hull[*length] = point;
        *length += 1;
    };
    for index in 0..count {
        push(&mut hull, &mut length, 0, sorted[index]);
    }
    let floor = length;
    for index in (0..count.saturating_sub(1)).rev() {
        push(&mut hull, &mut length, floor - 1, sorted[index]);
    }
    if length > 1 {
        length -= 1;
    }
    (hull, length)
}

/// The distance from the origin to a segment. The oracle's own, deliberately:
/// `contact.rs` has a segment-segment routine and borrowing it would make the
/// two implementations one.
fn origin_to_segment(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared > 0.0 {
        ((-a[0] * dx - a[1] * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    libm::hypot(a[0] + t * dx, a[1] + t * dy)
}


// ---------------------------------------------------------------- contact ---

#[test]
fn separated_squares_report_the_axis_gap_and_opposite_normals() {
    let a = square(0.0, 0.0, 10.0);
    let b = square(13.0, 0.0, 10.0);
    let forward = convex_cell_gap(&a, &b);
    let backward = convex_cell_gap(&b, &a);
    assert!((forward.signed_gap_mm - 3.0).abs() < 1e-12, "{forward:?}");
    assert!((backward.signed_gap_mm - 3.0).abs() < 1e-12);
    assert!((forward.normal[0] + backward.normal[0]).abs() < 1e-12);
    assert!((forward.normal[1] + backward.normal[1]).abs() < 1e-12);
    assert!(forward.normal[0] < 0.0, "a must move left to separate");
}

#[test]
fn touching_squares_report_exactly_zero() {
    let a = square(0.0, 0.0, 10.0);
    let b = square(10.0, 0.0, 10.0);
    let gap = convex_cell_gap(&a, &b);
    assert_eq!(gap.signed_gap_mm, 0.0, "{gap:?}");
    // Sol review 15 §B.6: exact material contact must still name a direction.
    // The gap is zero, but the row's *violation* is the whole pair clearance,
    // and a positive violation with a zero normal is weight in Phi with no
    // force in the gradient - a piece charged for an overlap it is given no way
    // to leave. The SAT's own separating axis is that direction.
    assert_eq!(gap.normal, [-1.0, 0.0], "a must move left to separate: {gap:?}");
    let reversed = convex_cell_gap(&b, &a);
    assert_eq!(reversed.signed_gap_mm, 0.0, "{reversed:?}");
    assert_eq!(reversed.normal, [1.0, 0.0], "{reversed:?}");
}

#[test]
fn overlapping_squares_report_negative_penetration() {
    let a = square(0.0, 0.0, 10.0);
    let b = square(7.0, 0.0, 10.0);
    let gap = convex_cell_gap(&a, &b);
    assert!((gap.signed_gap_mm + 3.0).abs() < 1e-12, "{gap:?}");
    assert!(gap.normal[0] < 0.0);
}

#[test]
fn containment_is_negative_and_never_false_feasible() {
    // The named hole in every pure segment-distance measure: a small square
    // wholly inside a large one has *positive* boundary distance and is
    // catastrophically feasible under it.
    let outer = square(0.0, 0.0, 100.0);
    let inner = square(40.0, 40.0, 5.0);
    let gap = convex_cell_gap(&outer, &inner);
    assert!(gap.signed_gap_mm < 0.0, "containment must be negative: {gap:?}");
    assert!(convex_cell_gap(&inner, &outer).signed_gap_mm < 0.0);
}

#[test]
fn signed_gap_is_symmetric_under_swap() {
    let cases = [
        (square(0.0, 0.0, 10.0), square(20.0, 3.0, 4.0)),
        (square(0.0, 0.0, 10.0), square(5.0, 5.0, 10.0)),
        (square(0.0, 0.0, 10.0), square(2.0, 2.0, 3.0)),
    ];
    for (a, b) in cases {
        let forward = convex_cell_gap(&a, &b);
        let backward = convex_cell_gap(&b, &a);
        assert!(
            (forward.signed_gap_mm - backward.signed_gap_mm).abs() < 1e-9,
            "{forward:?} vs {backward:?}"
        );
    }
}

#[test]
fn hot_path_matches_the_existing_sat_oracle_on_overlapping_convex_cells() {
    // Oracle one: the crate's own `measure_convex_sat_penetration`. It returns
    // `None` for separation and exact contact and allocates per call, which is
    // why it is a differential oracle and not the hot path - but where it
    // answers, the two must agree to the ULP scale its own module documents.
    let mut compared = 0usize;
    for dx in 0..9 {
        for dy in 0..9 {
            let a = square(0.0, 0.0, 10.0);
            let b = square(dx as f64, dy as f64, 10.0);
            let oracle = measure_convex_sat_penetration(
                &a.iter().map(|p| IrregularPoint::new(p[0], p[1])).collect::<Vec<_>>(),
                &b.iter().map(|p| IrregularPoint::new(p[0], p[1])).collect::<Vec<_>>(),
            );
            let ours = convex_cell_gap(&a, &b);
            match oracle {
                Some(penetration) => {
                    compared += 1;
                    assert!(
                        (penetration.depth + ours.signed_gap_mm).abs() < 1e-9,
                        "dx={dx} dy={dy} oracle={penetration:?} ours={ours:?}"
                    );
                }
                None => assert!(
                    ours.signed_gap_mm >= 0.0,
                    "dx={dx} dy={dy}: the oracle refused but we report penetration {ours:?}"
                ),
            }
        }
    }
    assert!(compared > 20, "the differential must actually overlap");
}

#[test]
fn triangle_hot_path_matches_the_nine_point_minkowski_oracle() {
    // Oracle two: Sol review 14 Round 1's construction. On two triangles the
    // nine-point Minkowski difference and the streamed SAT are the same query,
    // and this is where that claim is checked rather than asserted.
    let base = triangle([0.0, 0.0], [10.0, 0.0], [4.0, 8.0]);
    let mut compared = 0usize;
    for dx in -3..14 {
        for dy in -3..12 {
            let other = triangle(
                [dx as f64, dy as f64],
                [dx as f64 + 9.0, dy as f64 + 1.0],
                [dx as f64 + 3.0, dy as f64 + 7.0],
            );
            let ours = convex_cell_gap(&base, &other);
            let oracle = triangle_minkowski_signed_distance(&base, &other);
            assert!(
                (ours.signed_gap_mm - oracle).abs() < 1e-9,
                "dx={dx} dy={dy}: hot={} oracle={oracle}",
                ours.signed_gap_mm
            );
            compared += 1;
        }
    }
    assert!(compared > 100);
}

#[test]
fn nonfinite_input_fails_closed_in_the_decomposition() {
    let error = PolygonSet::from_outer(vec![
        IrregularPoint::new(0.0, 0.0),
        IrregularPoint::new(f64::NAN, 0.0),
        IrregularPoint::new(0.0, 1.0),
    ]);
    assert!(error.is_err(), "a non-finite ring must not build a polygon set");
}

// ----------------------------------------------------------- decomposition ---

#[test]
fn a_convex_piece_is_one_cell() {
    let set = polygon(&square(0.0, 0.0, 10.0));
    let decomposed = decompose(&set).expect("a convex decomposition");
    assert!(decomposed.convex);
    assert_eq!(decomposed.cells.len(), 1);
    assert_eq!(decomposed.cells[0].len, 4);
}

#[test]
fn ear_clip_preserves_area_and_winding() {
    // An L, deliberately nonconvex, with its reflex vertex in the middle of the
    // index order so the scan has to walk past it.
    let ring = vec![
        [0.0, 0.0],
        [10.0, 0.0],
        [10.0, 4.0],
        [4.0, 4.0],
        [4.0, 10.0],
        [0.0, 10.0],
    ];
    assert!(!is_convex(&ring));
    let triangles = ear_clip(&ring).expect("an ear clip");
    assert_eq!(triangles.len(), 4);
    let total: f64 = triangles
        .iter()
        .map(|t| signed_area(&[ring[t[0]], ring[t[1]], ring[t[2]]]))
        .sum();
    assert!(
        (total - signed_area(&ring)).abs() < 1e-9,
        "triangulated area {total} vs ring {}",
        signed_area(&ring)
    );
    for t in &triangles {
        assert!(
            signed_area(&[ring[t[0]], ring[t[1]], ring[t[2]]]) > 0.0,
            "every cell must stay counter-clockwise"
        );
    }
}

#[test]
fn holes_are_an_explicit_error_and_are_never_filled() {
    use crate::geometry::general_polygon::PolygonRegion;
    let outer = square(0.0, 0.0, 100.0)
        .iter()
        .map(|p| IrregularPoint::new(p[0], p[1]))
        .collect::<Vec<_>>();
    let hole = vec![
        IrregularPoint::new(40.0, 40.0),
        IrregularPoint::new(40.0, 60.0),
        IrregularPoint::new(60.0, 60.0),
        IrregularPoint::new(60.0, 40.0),
    ];
    let region = PolygonRegion::new(outer, vec![hole]).expect("a region with a hole");
    let set = PolygonSet::new(vec![region]).expect("a set with a hole");
    let error = source_ring(&set).expect_err("holes must be refused");
    assert!(
        error.contains("holes"),
        "the refusal must name holes: {error}"
    );
}

// ------------------------------------------------------------------ energy ---

struct Fixture {
    polygons: Vec<PolygonSet>,
    ids: Vec<String>,
}

impl Fixture {
    /// `count` identical squares in their own source frame at the origin. The
    /// layout comes from the poses, never from the source geometry - which is
    /// what the engine assumes everywhere else too.
    fn squares(count: usize, size: f64) -> Self {
        let mut polygons = Vec::new();
        let mut ids = Vec::new();
        for index in 0..count {
            polygons.push(polygon(&square(0.0, 0.0, size)));
            ids.push(format!("piece-{index:02}"));
        }
        Self { polygons, ids }
    }

    fn pieces(&self) -> Vec<GeneralFastPiece<'_>> {
        self.ids
            .iter()
            .zip(&self.polygons)
            .map(|(id, polygon)| GeneralFastPiece {
                id,
                polygon,
                allow_rotation: true,
                allow_mirror: false,
            })
            .collect()
    }
}

fn test_settings() -> GeneralFastSettings {
    let mut settings = GeneralFastSettings::deterministic_test(200.0, 400.0);
    settings.total_padding_mm = 5.0;
    settings.sheet_edge_clearance_mm = Some(5.0);
    settings.search_offset_allowance_mm = 0.0;
    settings
}

fn state_of(fixture: &Fixture, target: f64) -> (Vec<PieceSource>, Contract, IcsState) {
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = sources
        .iter()
        .map(|_| Pose {
            tx_mm: 10.0,
            ty_mm: 10.0,
            theta_deg: 0.0,
            mirrored: false,
        })
        .collect::<Vec<_>>();
    let geometry = build_geometry(&sources, &poses);
    let count = poses.len();
    let mut state = IcsState {
        poses,
        geometry,
        pair_rows: vec![PairRow::default(); pair_count(count)],
        edge_rows: vec![[EdgeRow::default(); 4]; count],
        target_depth_mm: target,
    };
    let mut work = WorkVector::default();
    rebuild_all(&mut state, &contract, &mut work);
    (sources, contract, state)
}

#[test]
fn incremental_rows_equal_a_cold_rebuild_bit_for_bit() {
    let fixture = Fixture::squares(8, 20.0);
    let (sources, contract, mut state) = state_of(&fixture, 300.0);
    // Move a piece, then update only its rows.
    state.poses[3].tx_mm += 7.25;
    state.poses[3].theta_deg += 11.5;
    transform_piece(&sources, &mut state.geometry, &state.poses, 3);
    let mut work = WorkVector::default();
    rebuild_piece_rows(&mut state, &contract, 3, &mut work);
    let incremental = fold(&state);
    let mut cold = state.clone();
    rebuild_all(&mut cold, &contract, &mut work);
    let cold_totals = fold(&cold);
    assert_eq!(
        incremental.raw.to_bits(),
        cold_totals.raw.to_bits(),
        "incremental {} vs cold {}",
        incremental.raw,
        cold_totals.raw
    );
    assert_eq!(incremental.guided.to_bits(), cold_totals.guided.to_bits());
    assert_eq!(
        incremental.max_violation_mm.to_bits(),
        cold_totals.max_violation_mm.to_bits()
    );
    for (left, right) in state.pair_rows.iter().zip(&cold.pair_rows) {
        assert_eq!(left.violation_mm.to_bits(), right.violation_mm.to_bits());
    }
}

#[test]
fn the_fixed_order_fold_equals_a_cold_phi() {
    let fixture = Fixture::squares(12, 20.0);
    let (_, contract, mut state) = state_of(&fixture, 300.0);
    let folded = fold(&state);
    let mut work = WorkVector::default();
    rebuild_all(&mut state, &contract, &mut work);
    let cold = fold(&state);
    assert_eq!(folded.raw.to_bits(), cold.raw.to_bits());
    assert_eq!(folded.guided.to_bits(), cold.guided.to_bits());
}

#[test]
fn the_gls_pass_changes_the_guided_total_and_never_the_raw_one() {
    let fixture = Fixture::squares(6, 20.0);
    let (_, _, mut state) = state_of(&fixture, 300.0);
    let before = fold(&state);
    assert!(before.raw > 0.0, "the fixture must actually overlap");
    let active = super::energy::gls_update(&mut state);
    assert!(active > 0, "the fixture has active rows");
    let after = fold(&state);
    assert_eq!(before.raw.to_bits(), after.raw.to_bits());
    assert!(after.guided > before.guided);
}

// ------------------------------------------------------------- publication ---

fn engine_fixture(target: f64, budget: u64) -> (Fixture, GeneralFastSettings, IcsConfig) {
    let fixture = Fixture::squares(6, 20.0);
    let settings = test_settings();
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let contract = Contract::from_settings(settings);
    let config = IcsConfig {
        target_depth_mm: target,
        proposal_budget: budget,
        relocate_eval_budget: u64::MAX,
        checkpoint_every_sweeps: 1,
        descent: DescentConfig::derive(&contract, &sources, 0),
        limits: PublicationLimits::default(),
    };
    (fixture, settings, config)
}

#[test]
fn a_zero_phi_layout_is_accepted_by_both_exact_authorities() {
    // Φ = 0 must imply round + contract acceptance outside the 4 µm canonical
    // band. The layout is a wide grid whose clearances are far above `c_pair`.
    let (fixture, settings, config) = engine_fixture(360.0, 0);
    let pieces = fixture.pieces();
    let contract = Contract::from_settings(settings);
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = (0..pieces.len())
        .map(|index| Pose {
            tx_mm: 20.0 + (index % 3) as f64 * 60.0,
            ty_mm: 20.0 + (index / 3) as f64 * 60.0,
            theta_deg: 0.0,
            mirrored: false,
        })
        .collect::<Vec<_>>();
    let incumbent = super::state::ExactIncumbent {
        placements: Vec::new(),
        raw_source_depth_mm: f64::INFINITY,
        from_constructor: true,
        placement_fingerprint: String::new(),
    };
    let mut engine = Engine::from_poses(
        &pieces, settings, sources, contract, poses, incumbent, config,
    );
    let totals = engine.totals();
    assert_eq!(totals.raw, 0.0, "the fixture must be Φ-feasible: {totals:?}");
    assert!(engine.checkpoint(), "Φ = 0 must publish");
    let checkpoint = engine.trace.checkpoints.last().expect("a checkpoint row");
    assert!(checkpoint.kernel_exclusive_valid, "{checkpoint:?}");
    assert!(checkpoint.contract_valid, "{checkpoint:?}");
    assert_eq!(checkpoint.repair_rows, 0, "a clear layout needs no repair");
    assert_eq!(checkpoint.repair_depth_giveback_mm, 0.0);
    assert!(!engine.incumbent.from_constructor);
}

#[test]
fn a_four_micrometre_deficit_is_repaired_inside_the_same_strip() {
    // Two squares whose *material* gap is 5.0 mm minus 3 µm: inside the
    // canonical band, so the round kernel refuses and the bounded repair is
    // exactly what should recover it - without moving the strip.
    let ids = vec!["a".to_owned(), "b".to_owned()];
    let polygons = vec![polygon(&square(0.0, 0.0, 20.0)), polygon(&square(0.0, 0.0, 20.0))];
    let fixture = Fixture { polygons, ids };
    let pieces = fixture.pieces();
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = vec![
        Pose { tx_mm: 20.0, ty_mm: 20.0, theta_deg: 0.0, mirrored: false },
        Pose { tx_mm: 45.0 - 0.003, ty_mm: 20.0, theta_deg: 0.0, mirrored: false },
    ];
    let config = IcsConfig {
        target_depth_mm: 200.0,
        proposal_budget: 0,
        relocate_eval_budget: u64::MAX,
        checkpoint_every_sweeps: 1,
        descent: DescentConfig::derive(&contract, &sources, 0),
        limits: PublicationLimits::default(),
    };
    let incumbent = super::state::ExactIncumbent {
        placements: Vec::new(),
        raw_source_depth_mm: f64::INFINITY,
        from_constructor: true,
        placement_fingerprint: String::new(),
    };
    let mut engine = Engine::from_poses(
        &pieces, settings, sources, contract, poses, incumbent, config,
    );
    let totals = engine.totals();
    assert!(
        totals.max_violation_mm > 0.0 && totals.max_violation_mm <= 0.004,
        "the deficit must sit inside the band: {totals:?}"
    );
    let published = engine.checkpoint();
    let checkpoint = engine.trace.checkpoints.last().expect("a checkpoint row").clone();
    assert!(published, "a banded deficit must be repairable: {checkpoint:?}");
    assert!(checkpoint.repair_rows >= 1, "{checkpoint:?}");
    assert!(
        checkpoint.repair_max_displacement_mm <= 0.016 + 1e-12,
        "per-piece repair must stay under 16 µm: {checkpoint:?}"
    );
    assert!(
        checkpoint.published_raw_depth_mm.expect("a depth") <= 200.0,
        "repair may not enlarge the locked strip: {checkpoint:?}"
    );
}

#[test]
fn a_half_millimetre_deficit_is_discarded_rather_than_legalized() {
    // The inflation test. A 0.5 mm shortfall is far outside the band; the
    // checkpoint must be thrown away and `best_exact` must not move.
    let ids = vec!["a".to_owned(), "b".to_owned()];
    let polygons = vec![polygon(&square(0.0, 0.0, 20.0)), polygon(&square(0.0, 0.0, 20.0))];
    let fixture = Fixture { polygons, ids };
    let pieces = fixture.pieces();
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = vec![
        Pose { tx_mm: 20.0, ty_mm: 20.0, theta_deg: 0.0, mirrored: false },
        Pose { tx_mm: 44.5, ty_mm: 20.0, theta_deg: 0.0, mirrored: false },
    ];
    let mut limits = PublicationLimits::default();
    // Widen only the *attempt* band, so the attempt happens and the repair caps
    // are what refuse it. The shipped band would not even try.
    limits.band_mm = 1.0;
    let config = IcsConfig {
        target_depth_mm: 200.0,
        proposal_budget: 0,
        relocate_eval_budget: u64::MAX,
        checkpoint_every_sweeps: 1,
        descent: DescentConfig::derive(&contract, &sources, 0),
        limits,
    };
    let incumbent = super::state::ExactIncumbent {
        placements: Vec::new(),
        raw_source_depth_mm: f64::INFINITY,
        from_constructor: true,
        placement_fingerprint: String::new(),
    };
    let mut engine = Engine::from_poses(
        &pieces, settings, sources, contract, poses, incumbent, config,
    );
    assert!(!engine.checkpoint(), "a 0.5 mm deficit must never publish");
    let checkpoint = engine.trace.checkpoints.last().expect("a checkpoint row");
    assert!(checkpoint.published_raw_depth_mm.is_none());
    assert!(checkpoint.repair_max_displacement_mm <= 0.016 + 1e-12);
    assert!(engine.incumbent.from_constructor, "best_exact must not move");
}

#[test]
fn the_search_allowance_is_forced_to_zero_at_publication() {
    let mut settings = test_settings();
    settings.search_offset_allowance_mm = 0.002;
    assert_eq!(publication_settings(settings).search_offset_allowance_mm, 0.0);
}

#[test]
fn the_fingerprint_separates_two_layouts_that_differ_by_one_micrometre() {
    use crate::search::general_fast::GeneralFastPlacement;
    let base = vec![GeneralFastPlacement {
        piece_id: "a".to_owned(),
        rotation_deg: 30.0,
        mirrored: false,
        translate_short_axis: 10.0,
        translate_long_axis: 20.0,
    }];
    let mut moved = base.clone();
    moved[0].translate_long_axis = 20.001;
    assert_ne!(placement_fingerprint(&base), placement_fingerprint(&moved));
    assert_eq!(placement_fingerprint(&base), placement_fingerprint(&base.clone()));
}

// ------------------------------------------------------------------ solver ---

#[test]
fn the_descent_lowers_raw_phi_on_a_compressed_grid() {
    let (fixture, settings, config) = engine_fixture(220.0, 600);
    let pieces = fixture.pieces();
    let contract = Contract::from_settings(settings);
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = (0..pieces.len())
        .map(|index| Pose {
            tx_mm: 20.0 + (index % 3) as f64 * 22.0,
            ty_mm: 20.0 + (index / 3) as f64 * 22.0,
            theta_deg: 0.0,
            mirrored: false,
        })
        .collect::<Vec<_>>();
    let incumbent = super::state::ExactIncumbent {
        placements: Vec::new(),
        raw_source_depth_mm: f64::INFINITY,
        from_constructor: true,
        placement_fingerprint: String::new(),
    };
    let mut engine = Engine::from_poses(
        &pieces, settings, sources, contract, poses, incumbent, config,
    );
    let before = engine.totals().raw;
    assert!(before > 0.0);
    let outcome = engine.run();
    assert!(
        outcome.final_raw_phi < before,
        "raw Φ went {before} -> {}",
        outcome.final_raw_phi
    );
    assert!(outcome.trace.work.piece_proposals > 0);
}

#[test]
fn two_runs_of_the_same_trajectory_are_bit_identical() {
    let run = || {
        let (fixture, settings, config) = engine_fixture(220.0, 400);
        let pieces = fixture.pieces();
        let contract = Contract::from_settings(settings);
        let sources = super::state::piece_sources(&pieces).expect("sources");
        let poses = (0..pieces.len())
            .map(|index| Pose {
                tx_mm: 20.0 + (index % 3) as f64 * 22.0,
                ty_mm: 20.0 + (index / 3) as f64 * 22.0,
                theta_deg: 0.0,
                mirrored: false,
            })
            .collect::<Vec<_>>();
        let incumbent = super::state::ExactIncumbent {
            placements: Vec::new(),
            raw_source_depth_mm: f64::INFINITY,
            from_constructor: true,
            placement_fingerprint: String::new(),
        };
        let mut engine = Engine::from_poses(
            &pieces, settings, sources, contract, poses, incumbent, config,
        );
        let outcome = engine.run();
        (
            outcome
                .final_poses
                .iter()
                .map(|pose| {
                    (
                        pose.tx_mm.to_bits(),
                        pose.ty_mm.to_bits(),
                        pose.theta_deg.to_bits(),
                    )
                })
                .collect::<Vec<_>>(),
            outcome.final_raw_phi.to_bits(),
            outcome.trace.work,
        )
    };
    assert_eq!(run(), run());
}

#[test]
fn the_counter_source_is_a_function_of_its_key_alone() {
    assert_eq!(counter_hash(&[1, 2, 3]), counter_hash(&[1, 2, 3]));
    assert_ne!(counter_hash(&[1, 2, 3]), counter_hash(&[1, 2, 4]));
    for index in 1..64u64 {
        let value = rotated_halton(2, index, 12345);
        assert!((0.0..1.0).contains(&value), "{value}");
    }
}

/// Sol review 15 §B.7, verbatim: *"Test sag-specifico: top virtuale
/// soddisfatto a `max_y = T - edge`, mentre left/right/bottom continuano a
/// richiedere `edge + sag`."*
///
/// The four rows of one box, on a contract with `sag = 0.25`, placed so that
/// each side is *exactly* on its own threshold. If the strip top were charged
/// `edge + sag` like a sheet edge, the first assertion would read `0.25`
/// instead of `0`, and every triangle-20 trajectory would be descending toward
/// a target a quarter of a millimetre stricter than the gate it publishes
/// through.
#[test]
fn the_strip_top_is_sag_less_while_the_sheet_edges_are_not() {
    let mut settings = GeneralFastSettings::deterministic_test(2000.0, 2700.0);
    settings.total_padding_mm = 5.0;
    settings.sheet_edge_clearance_mm = Some(5.0);
    settings.flattening_sag_tolerance_mm = 0.25;
    let contract = Contract::from_settings(settings);
    assert_eq!(contract.physical_edge_clearance_mm(), 5.25);
    assert_eq!(contract.depth_top_inset_mm(), 5.0);

    let target = 70.742;
    // A box whose top is exactly at `T - depth_top_inset` and whose other three
    // sides are exactly on the physical `edge + sag` thresholds.
    let satisfied = [
        contract.physical_edge_clearance_mm(),
        contract.physical_edge_clearance_mm(),
        settings.sheet_short_axis_mm - contract.physical_edge_clearance_mm(),
        target - contract.depth_top_inset_mm(),
    ];
    let rows = super::broad_phase::boundary_residuals(satisfied, &contract, target);
    assert_eq!(rows, [0.0, 0.0, 0.0, 0.0], "every side is exactly satisfied");

    // One micrometre past the sag-less top is a violation, of exactly that.
    let over = [satisfied[0], satisfied[1], satisfied[2], satisfied[3] + 0.001];
    let rows = super::broad_phase::boundary_residuals(over, &contract, target);
    assert!((rows[3] - 0.001).abs() < 1e-12, "top row {rows:?}");

    // The three physical sides keep charging the sag: a box sitting on the
    // sag-*less* inset on the left is violating by exactly one sag tolerance.
    let shy = [
        contract.depth_top_inset_mm(),
        satisfied[1],
        satisfied[2],
        satisfied[3],
    ];
    let rows = super::broad_phase::boundary_residuals(shy, &contract, target);
    assert!((rows[0] - 0.25).abs() < 1e-12, "left row {rows:?}");

    // And the physical sheet top is still a boundary, even though no Gate-0
    // cell reaches it: a locked target beyond the sheet cannot buy depth.
    let deep = [satisfied[0], satisfied[1], satisfied[2], 2700.0 - 5.0];
    let rows = super::broad_phase::boundary_residuals(deep, &contract, 3000.0);
    assert!((rows[3] - 0.25).abs() < 1e-12, "sheet top row {rows:?}");
}

/// The same split, one level up: `lower_scale_mm` is sag-aware and asymmetric.
///
/// Sol review 15 §A.1 computes triangle-20's correct floor as
/// `60 + 5.25 + 5.0 = 70.25`, against the 70.0 the symmetric `2 * edge`
/// produced. The bound is a *lower* bound on a depth in the sag-less
/// publication convention, so it carries one physical bottom edge and one
/// sag-less top inset - not two of either.
#[test]
fn the_lower_scale_carries_one_physical_edge_and_one_depth_inset() {
    let fixture = Fixture::squares(1, 60.0);
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");

    let mut settings = GeneralFastSettings::deterministic_test(2000.0, 2700.0);
    settings.total_padding_mm = 5.0;
    settings.sheet_edge_clearance_mm = Some(5.0);
    settings.flattening_sag_tolerance_mm = 0.25;
    let sagged = super::homotopy::lower_scale_mm(&sources, &Contract::from_settings(settings));

    settings.flattening_sag_tolerance_mm = 0.0;
    let exact = super::homotopy::lower_scale_mm(&sources, &Contract::from_settings(settings));

    // 60 mm of minimum width, plus 5.25 below and 5.0 above.
    assert!((sagged - 70.25).abs() < 1e-9, "sagged {sagged}");
    // With `sag = 0` the two clearances coincide and the bound is unchanged,
    // which is why mixed-61's `L` - and therefore C175's target - does not move.
    assert!((exact - 70.0).abs() < 1e-9, "exact {exact}");
}

// ------------------------------------------------------------------- pivot ---
//
// The rotation pivot, which outlived the operator that found it.
//
// `gate0-rerun/README.md` §2.2 named the defect - torque taken about the
// transformed **centroid**, step composed about the pose **origin** - and §2.3
// declined to repair it in the round that found it. Two of the three vectors
// that settled it were written against the gradient ladder and died with it.
// The invariant did not: the coordinate descent's wiggle axis turns the piece
// about its transformed centroid through the same `compose_proposal`, and a
// wiggle that slid the piece sideways while claiming to test an angle would be
// the identical defect in a new operator. The composition vector below is the
// one the ladder vectors were consequences of, and
// `relocate_wiggle_turns_about_the_transformed_centroid` is its use-site.

/// One piece whose source ring is given explicitly, so a test can put the ring
/// far from its own pose origin - which is where both campaign fixtures put
/// theirs.
fn one_piece(ring: &[[f64; 2]]) -> Fixture {
    Fixture {
        polygons: vec![polygon(ring)],
        ids: vec!["piece-00".to_owned()],
    }
}

/// The invariant the previous two tests are two consequences of: a proposal
/// with `dt = 0` leaves the transformed centroid exactly where it was.
///
/// The composition is a rigid rotation *about* that centroid, so this is not a
/// first-order statement and the tolerance is not a linearization error - it is
/// round-off, and the assertion holds at 180° as firmly as at 0.25 µm worth of
/// turn. 1 nm is the derived floor: 1/250 of the ladder's bottom rung and
/// 1/1000 of the publication band's 1 µm canonical grid.
///
/// It is checked across the whole ladder's worth of angles, past a full turn,
/// at both mirror flags and at a non-zero starting rotation - because the
/// composition has to be mirror-agnostic (a rigid post-composition does not see
/// the mirror) and has to work from a pose that is already turned, which every
/// pose in the S0 import is.
#[test]
fn a_pure_rotation_proposal_leaves_the_transformed_centroid_where_it_was() {
    const ROUND_OFF_MM: f64 = 1e-6;

    let fixture = one_piece(&square(90.0, 90.0, 20.0));
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let mut worst = 0.0f64;
    for mirrored in [false, true] {
        for theta_deg in [0.0, 17.5, -123.75, 359.0] {
            let pose = Pose { tx_mm: 31.5, ty_mm: -87.25, theta_deg, mirrored };
            let mut geometry = build_geometry(&sources, &[pose]);
            let pivot = geometry.centroids[0];
            for dtheta_deg in [
                1e-6, 0.001, 0.5, 2.0, 30.0, 90.0, 180.0, 359.9, -45.0, -270.0,
            ] {
                let turned = super::state::compose_proposal(pose, pivot, 0.0, 0.0, dtheta_deg);
                assert_eq!(
                    turned.theta_deg.to_bits(),
                    (theta_deg + dtheta_deg).to_bits(),
                    "the angle coordinate is the plain sum, in degrees"
                );
                assert_eq!(turned.mirrored, mirrored, "a rigid step never mirrors");
                let poses = [turned];
                transform_piece(&sources, &mut geometry, &poses, 0);
                let moved = libm::hypot(
                    geometry.centroids[0][0] - pivot[0],
                    geometry.centroids[0][1] - pivot[1],
                );
                worst = worst.max(moved);
                assert!(
                    moved <= ROUND_OFF_MM,
                    "a pure rotation of {dtheta_deg} deg about {pivot:?} \
                     (mirrored {mirrored}, theta {theta_deg}) moved the \
                     transformed centroid by {moved} mm"
                );
                // Restore the geometry for the next angle: every iteration
                // composes from the same pose, never from the previous one.
                let poses = [pose];
                transform_piece(&sources, &mut geometry, &poses, 0);
            }
        }
    }
    // The invariance is exact-to-round-off, not merely inside the band. If this
    // ever reads micrometres the composition has stopped being rigid.
    assert!(worst < 1e-12, "worst centroid excursion {worst} mm");
}

// ==================================================== the CutCloseRelocate ===
//
// The member's own vectors. Grok review 12 Round 2 §6.9 and the wave-1 task
// list them: sample uniqueness, accept-equal, a container commit beyond the old
// `ladder_top` on a distant-vacancy fixture, the coordinate descent's wiggle
// pivot, wiggle only when the piece may rotate, the Algorithm-8 schedule, the
// interior witness on both campaign fixtures, and the swap-with-followers map.

use super::disrupt::{
    carry, closest_feasible_angle, disrupt, is_distinct_enough, large_pieces, point_in_ring,
    transformed_witness,
};
use super::relocate::{
    angle_gap_deg, cd_accepts, colliding_permutation, coord_descent, eval_cmp, relocate,
    transformed_centroid, wiggle_pose, BestSamples, Candidate, RelocateConfig, RelocateKey,
    SampleEval, SampleOrigin,
};

/// A fixture whose pieces carry an explicit rotation permission, so the two
/// wiggle vectors can be the same geometry with the flag flipped.
fn pieces_of(fixture: &Fixture, allow_rotation: bool) -> Vec<GeneralFastPiece<'_>> {
    fixture
        .ids
        .iter()
        .zip(&fixture.polygons)
        .map(|(id, polygon)| GeneralFastPiece {
            id,
            polygon,
            allow_rotation,
            allow_mirror: false,
        })
        .collect()
}

/// A state built from an explicit pose per piece.
fn state_of_poses(
    fixture: &Fixture,
    poses: Vec<Pose>,
    target: f64,
) -> (Vec<PieceSource>, Contract, IcsState) {
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let geometry = build_geometry(&sources, &poses);
    let count = poses.len();
    let mut state = IcsState {
        poses,
        geometry,
        pair_rows: vec![PairRow::default(); pair_count(count)],
        edge_rows: vec![[EdgeRow::default(); 4]; count],
        target_depth_mm: target,
    };
    let mut work = WorkVector::default();
    rebuild_all(&mut state, &contract, &mut work);
    (sources, contract, state)
}

fn pose_at(tx: f64, ty: f64) -> Pose {
    Pose {
        tx_mm: tx,
        ty_mm: ty,
        theta_deg: 0.0,
        mirrored: false,
    }
}

fn collision(weighted: f64) -> SampleEval {
    SampleEval {
        raw: weighted,
        weighted,
    }
}

const CLEAR: SampleEval = SampleEval {
    raw: 0.0,
    weighted: 0.0,
};

// ------------------------------------------------------------ the ordering ---

/// The lexicographic sample order, and the accept-equal rule it feeds.
///
/// `Clear` beats every collision however small, two clears compare **equal**,
/// and below that the order is the weighted incident Φ. The equality is the
/// load-bearing half: Sparrow's `SampleEval::Clear` carries no payload, and
/// `coord_descent.rs::tell` adopts anything `!worse`. `if after < before` -
/// the line Grok review 12 Round 2 §6.3 deletes by name - is precisely the
/// negation of `cd_accepts` on an equal evaluation.
#[test]
fn the_sample_order_is_lexicographic_and_the_descent_accepts_an_equal() {
    assert_eq!(eval_cmp(CLEAR, collision(1e-12)), std::cmp::Ordering::Less);
    assert_eq!(eval_cmp(CLEAR, CLEAR), std::cmp::Ordering::Equal);
    assert_eq!(
        eval_cmp(collision(1.0), collision(2.0)),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        eval_cmp(collision(1.0), SampleEval::INVALID),
        std::cmp::Ordering::Less
    );

    // The rule itself: not-worse is accepted.
    assert!(cd_accepts(collision(2.0), collision(1.0)), "better");
    assert!(cd_accepts(collision(2.0), collision(2.0)), "EQUAL");
    assert!(cd_accepts(CLEAR, CLEAR), "two clears are equal, and accepted");
    assert!(!cd_accepts(collision(1.0), collision(2.0)), "worse");
    assert!(!cd_accepts(CLEAR, collision(1e-12)), "clear is never left");
}

/// **Accept-equal, walked rather than asserted.**
///
/// One piece alone in the middle of a strip: every pose the coarse coordinate
/// descent can reach is collision-free, so every candidate compares *equal* to
/// the current one and a strict-decrease rule would refuse all of them and
/// return the start pose untouched. Accept-equal crosses the plateau instead.
///
/// The walk still terminates, and quickly: equal is not `better`, so every step
/// halves its axis and re-draws.
#[test]
fn the_coordinate_descent_crosses_a_plateau_of_equal_evaluations() {
    let fixture = Fixture::squares(1, 20.0);
    let start = pose_at(90.0, 90.0);
    let (sources, contract, mut state) = state_of_poses(&fixture, vec![start], 300.0);
    assert_eq!(fold(&state).raw, 0.0, "the fixture must be clear");

    let config = RelocateConfig::default();
    let mut work = WorkVector::default();
    let (pose, eval) = coord_descent(
        &mut state,
        &sources,
        &contract,
        0,
        start,
        CLEAR,
        config.coarse,
        &config,
        true,
        99,
        &mut work,
    );
    assert!(eval.is_clear(), "the walk never left the clear plateau: {eval:?}");
    assert!(work.sample_evaluations >= 2, "the walk evaluated candidates");
    let travelled = libm::hypot(pose.tx_mm - start.tx_mm, pose.ty_mm - start.ty_mm);
    assert!(
        travelled > 0.0 || pose.theta_deg != start.theta_deg,
        "a strict-decrease rule returns the start pose; accept-equal must move: \
         {start:?} -> {pose:?}"
    );
}

// ------------------------------------------------------------- uniqueness ---

/// **The three finalists are three *different* poses.**
///
/// `sample/best_samples.rs`'s rule, on our poses: a sample similar to one
/// already held is accepted only if it beats **all** the samples it is similar
/// to, and then it evicts them. Without it, 75 draws that happen to cluster
/// would spend all three coordinate descents in one basin, and the container
/// half of the pool would be paid for and thrown away.
#[test]
fn the_finalist_pool_holds_three_poses_no_two_of_which_are_the_same_sample() {
    let threshold = 1.0;
    let mut pool = BestSamples::new(3, threshold, 1.0);
    // Four well-separated poses, improving: the fourth evicts the worst.
    for (index, weighted) in [(0usize, 9.0), (1, 7.0), (2, 5.0), (3, 3.0)] {
        assert!(
            pool.report(Candidate {
                pose: pose_at(index as f64 * 10.0, 0.0),
                eval: collision(weighted),
                origin: SampleOrigin::Container,
            }),
            "a distinct improving sample is accepted"
        );
    }
    assert_eq!(pool.samples.len(), 3, "the pool is bounded");
    for (index, left) in pool.samples.iter().enumerate() {
        for right in &pool.samples[index + 1..] {
            assert!(
                !pool.similar(left.pose, right.pose),
                "two finalists are the same sample: {:?} vs {:?}",
                left.pose,
                right.pose
            );
        }
    }
    // A *worse* near-duplicate of the current best is refused outright, even
    // though its score would otherwise have earned a slot.
    let best = pool.best().expect("a best sample");
    assert!(
        !pool.report(Candidate {
            pose: pose_at(best.pose.tx_mm + threshold / 2.0, best.pose.ty_mm),
            eval: collision(best.eval.weighted + 1.0),
            origin: SampleOrigin::Container,
        }),
        "a worse near-duplicate must not take a coordinate descent's slot"
    );
    // A *better* near-duplicate replaces it, and does not sit beside it.
    let held = pool.samples.len();
    assert!(pool.report(Candidate {
        pose: pose_at(best.pose.tx_mm + threshold / 2.0, best.pose.ty_mm),
        eval: collision(best.eval.weighted - 1.0),
        origin: SampleOrigin::Container,
    }));
    assert_eq!(pool.samples.len(), held, "the similar sample was evicted");

    // The angular half of the rule, on the accumulated degree coordinate.
    assert!((angle_gap_deg(359.5, 0.5) - 1.0).abs() < 1e-12, "the wrap is closed");
    assert!((angle_gap_deg(-0.5, 720.5) - 1.0).abs() < 1e-12, "and so is a full turn");
    let mut turned = pose_at(0.0, 0.0);
    turned.theta_deg = 0.5;
    assert!(pool.similar(pose_at(0.0, 0.0), turned), "half a degree is one sample");
    turned.theta_deg = 1.5;
    assert!(
        !pool.similar(pose_at(0.0, 0.0), turned),
        "a degree and a half is two"
    );
}

// ---------------------------------------------- the neutered-relocate wire ---

/// **The pre-named defect's tripwire.**
///
/// Both consultants named the same most-likely implementation failure: the 50
/// container-wide samples are drawn and evaluated, and then a leftover strict
/// filter or step cap rejects every one of them, so only local refinement ever
/// commits. Grok review 12 §6.3.1 writes the vector as counters plus a
/// distance, and this is it verbatim: `containerSamples >= 50`,
/// `focusedSamples >= 25`, `containerCommits >= 1`, and a committed
/// displacement **greater than the old `ladder_top`** - the exact radius the
/// retired backtracking ladder could not leave.
///
/// The fixture is the minimum that can distinguish the two behaviours: two
/// identical squares in the *same* pose, in a strip with room for a hundred of
/// them. Every focused sample is inside the piece's own overlapping AABB and
/// cannot be collision-free; a container sample almost anywhere is. A neutered
/// relocate reports the samples and stays put.
#[test]
fn a_relocate_commits_a_container_pose_far_beyond_the_old_ladder_top() {
    let fixture = Fixture::squares(2, 20.0);
    let stacked = pose_at(10.0, 10.0);
    let (sources, contract, mut state) = state_of_poses(&fixture, vec![stacked, stacked], 300.0);
    let entry = fold(&state);
    assert!(entry.raw > 0.0, "the two pieces must overlap: {entry:?}");

    let ladder_top_mm = DescentConfig::derive(&contract, &sources, 0).ladder_top_mm;
    assert!(
        (ladder_top_mm - 1.25).abs() < 1e-12,
        "the retired ladder's top rung {ladder_top_mm}"
    );

    let config = RelocateConfig::default();
    let mut work = WorkVector::default();
    let outcome = relocate(
        &mut state,
        &sources,
        &contract,
        &[true, true],
        1,
        &config,
        RelocateKey::default(),
        &mut work,
    );

    assert!(outcome.ran, "a colliding piece is in the colliding set");
    assert!(
        work.focused_samples >= 25,
        "focusedSamples {}",
        work.focused_samples
    );
    assert!(
        work.container_samples >= 50,
        "containerSamples {}",
        work.container_samples
    );
    assert_eq!(
        outcome.origin,
        SampleOrigin::Container,
        "the vacancy is container-wide and nothing focused can reach it"
    );
    assert!(
        work.container_commits >= 1,
        "containerCommits {} - the container half of the pool was evaluated and \
         then refused; look at the commit filter, not the sampler",
        work.container_commits
    );
    assert!(
        outcome.displacement_mm > ladder_top_mm,
        "committed displacement {} mm is inside the retired ladder's {} mm \
         neighbourhood: this is PGS in a sampling costume",
        outcome.displacement_mm,
        ladder_top_mm
    );
    assert!(
        outcome.after.is_clear(),
        "the strip has a vacancy and the relocate must find it: {:?}",
        outcome.after
    );
    // The whole pool was actually paid for: 1 current pose + 25 + 50, plus the
    // four coordinate-descent walks.
    assert!(
        outcome.sample_evaluations > 76,
        "sampleEvaluations {} - the coordinate descents did not run",
        outcome.sample_evaluations
    );
    assert!(
        work.sample_evaluations_per_relocate() > 76.0,
        "sampleEvaluationsPerRelocate {}",
        work.sample_evaluations_per_relocate()
    );
    println!(
        "neutered-relocate tripwire: focusedSamples={} containerSamples={} \
         containerCommits={} sampleEvaluations={} displacement={:.3} mm against \
         ladderTop={:.3} mm",
        work.focused_samples,
        work.container_samples,
        work.container_commits,
        outcome.sample_evaluations,
        outcome.displacement_mm,
        ladder_top_mm
    );
}

/// A relocate never runs on a piece that is not in the colliding set, and never
/// touches it. Their `ct.get_loss(pk) > 0.0` filter.
#[test]
fn a_clear_piece_is_not_in_the_colliding_set() {
    let fixture = Fixture::squares(2, 20.0);
    let apart = vec![pose_at(20.0, 20.0), pose_at(120.0, 120.0)];
    let (sources, contract, mut state) = state_of_poses(&fixture, apart.clone(), 300.0);
    assert_eq!(fold(&state).raw, 0.0);

    let mut order = Vec::new();
    colliding_permutation(&state, RelocateKey::default(), &mut order);
    assert!(order.is_empty(), "nothing collides: {order:?}");

    let mut work = WorkVector::default();
    let outcome = relocate(
        &mut state,
        &sources,
        &contract,
        &[true, true],
        0,
        &RelocateConfig::default(),
        RelocateKey::default(),
        &mut work,
    );
    assert!(!outcome.ran);
    assert_eq!(work.sample_evaluations, 0, "nothing was sampled");
    assert_eq!(work.relocates, 0);
    assert_eq!(state.poses[0], apart[0], "and the pose is untouched");
}

// ----------------------------------------------------------- the CD pivot ---

/// **The wiggle turns the piece about its transformed centroid.**
///
/// The same invariant the retired ladder's pivot vectors were consequences of,
/// now at the coordinate descent's own use site. A wiggle that composed about
/// the pose origin would slide the piece by `|c − t| · dtheta` while claiming to
/// be testing an angle - which on both campaign fixtures is at least as large as
/// the rotation it is modelling, and is the defect `gate0-rerun/README.md` §2.2
/// named.
///
/// The ring here sits 141 mm from its own pose origin, ten circumradii, so an
/// origin pivot would be off by tens of millimetres rather than by round-off.
#[test]
fn the_relocate_wiggle_turns_about_the_transformed_centroid() {
    let fixture = one_piece(&square(90.0, 90.0, 20.0));
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let offset = libm::hypot(sources[0].centroid[0], sources[0].centroid[1]);
    assert!(
        (offset - 141.42135623730951).abs() < 1e-9,
        "centroid offset {offset}"
    );

    let mut worst = 0.0f64;
    for mirrored in [false, true] {
        for theta_deg in [0.0, 17.5, -123.75, 359.0] {
            let pose = Pose {
                tx_mm: 31.5,
                ty_mm: -87.25,
                theta_deg,
                mirrored,
            };
            let before = transformed_centroid(&sources[0], pose);
            for dtheta_deg in [0.05, 0.5, 5.0, -5.0, 90.0, -270.0] {
                let turned = wiggle_pose(&sources[0], pose, dtheta_deg);
                assert_eq!(
                    turned.theta_deg.to_bits(),
                    (theta_deg + dtheta_deg).to_bits(),
                    "the angle coordinate is the plain sum, in degrees"
                );
                assert_eq!(turned.mirrored, mirrored, "a wiggle never mirrors");
                let after = transformed_centroid(&sources[0], turned);
                let moved = libm::hypot(after[0] - before[0], after[1] - before[1]);
                worst = worst.max(moved);
            }
        }
    }
    assert!(
        worst < 1e-9,
        "the wiggle moved the transformed centroid by {worst} mm; it is turning \
         about the pose origin, not the centroid"
    );
}

/// **The wiggle axis exists only when the piece may rotate**, and so do the 16
/// sampled orientations.
///
/// Two assertions on the same geometry with the flag flipped: a frozen piece's
/// angle is bit-identical after a coordinate descent *and* after a whole
/// relocate, and a rotatable one's is not. `sample/search.rs::prerefine_cd_config`
/// enables the wiggle only for `RotationRange::Continuous`;
/// `uniform_sampler.rs` gives a `RotationRange::None` item the single angle 0,
/// and our analogue of "the piece's allowed set" for a frozen piece is the angle
/// it already has.
#[test]
fn a_frozen_piece_keeps_its_angle_through_the_whole_member() {
    let fixture = Fixture::squares(2, 20.0);
    let stacked = pose_at(10.0, 10.0);

    for allow_rotation in [false, true] {
        let (sources, contract, mut state) =
            state_of_poses(&fixture, vec![stacked, stacked], 300.0);
        let mut work = WorkVector::default();
        let outcome = relocate(
            &mut state,
            &sources,
            &contract,
            &[allow_rotation, allow_rotation],
            1,
            &RelocateConfig::default(),
            RelocateKey::default(),
            &mut work,
        );
        assert!(outcome.ran);
        if allow_rotation {
            assert!(
                state.poses[1].theta_deg != stacked.theta_deg,
                "a rotatable piece must be able to turn: the 16 orientations and \
                 the wiggle axis are the rotation half of the operator"
            );
        } else {
            assert_eq!(
                state.poses[1].theta_deg.to_bits(),
                stacked.theta_deg.to_bits(),
                "a frozen piece turned by {} deg",
                outcome.rotation_deg
            );
        }
    }

    // And directly on the walk, where the axis is drawn.
    let pieces = pieces_of(&fixture, false);
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let (_, contract, mut state) = state_of_poses(&fixture, vec![stacked, stacked], 300.0);
    let config = RelocateConfig::default();
    let mut work = WorkVector::default();
    let (pose, _) = coord_descent(
        &mut state,
        &sources,
        &contract,
        1,
        stacked,
        collision(1.0),
        config.coarse,
        &config,
        false,
        7,
        &mut work,
    );
    assert_eq!(
        pose.theta_deg.to_bits(),
        stacked.theta_deg.to_bits(),
        "the wiggle axis must not be drawable for a frozen piece"
    );
}

// -------------------------------------------------------------------- GLS ---

/// **Algorithm 8's schedule, one dialect, all rows.**
///
/// Four claims in one fixture, because they are one rule: the inactive decay
/// has a floor at 1, the active multiplier is `1.2 + 0.8 v/v_max` so the worst
/// row gets exactly 2 and a half-pressure row exactly 1.6, the guided fold is
/// `Σ w v²` with no second term anywhere, and the cap holds.
#[test]
fn the_gls_schedule_is_the_published_one_and_the_only_one() {
    let fixture = Fixture::squares(3, 20.0);
    let (_, _, mut state) = state_of_poses(
        &fixture,
        vec![pose_at(10.0, 10.0), pose_at(60.0, 60.0), pose_at(110.0, 110.0)],
        300.0,
    );
    // A hand-built row set: one row at the worst violation, one at half of it,
    // one clear and already carrying weight, one clear at the floor.
    for row in &mut state.pair_rows {
        row.violation_mm = 0.0;
        row.weight = 1.0;
    }
    for rows in &mut state.edge_rows {
        for row in rows {
            row.violation_mm = 0.0;
            row.weight = 1.0;
        }
    }
    state.pair_rows[0].violation_mm = 4.0;
    state.pair_rows[1].violation_mm = 2.0;
    state.pair_rows[2].weight = 2.0;
    state.edge_rows[0][0].weight = 1.0;

    let active = super::energy::gls_update(&mut state);
    assert_eq!(active, 2, "exactly two rows carry a violation");
    assert!(
        (state.pair_rows[0].weight - 2.0).abs() < 1e-12,
        "the worst row takes the maximum ratio: {}",
        state.pair_rows[0].weight
    );
    assert!(
        (state.pair_rows[1].weight - 1.6).abs() < 1e-12,
        "a half-pressure row takes 1.2 + 0.8/2: {}",
        state.pair_rows[1].weight
    );
    assert!(
        (state.pair_rows[2].weight - 1.9).abs() < 1e-12,
        "an inactive row decays by 0.95: {}",
        state.pair_rows[2].weight
    );
    assert!(
        (state.edge_rows[0][0].weight - 1.0).abs() < 1e-12,
        "and never below the floor: {}",
        state.edge_rows[0][0].weight
    );

    // One dialect: the guided fold is `Σ w v²` and nothing else. There is no
    // integer penalty left to add a second term.
    let totals = fold(&state);
    let mut expected_raw = 0.0;
    let mut expected_guided = 0.0;
    for row in &state.pair_rows {
        if row.violation_mm > 0.0 {
            expected_raw += row.violation_mm * row.violation_mm;
            expected_guided += row.weight * row.violation_mm * row.violation_mm;
        }
    }
    for rows in &state.edge_rows {
        for row in rows {
            if row.violation_mm > 0.0 {
                expected_raw += row.violation_mm * row.violation_mm;
                expected_guided += row.weight * row.violation_mm * row.violation_mm;
            }
        }
    }
    assert_eq!(totals.raw.to_bits(), expected_raw.to_bits());
    assert_eq!(totals.guided.to_bits(), expected_guided.to_bits());

    // The cap, and the reset.
    for _ in 0..200 {
        super::energy::gls_update(&mut state);
    }
    assert!(
        state.pair_rows[0].weight <= super::energy::GLS_WEIGHT_CAP,
        "weight {} above the 2^20 cap",
        state.pair_rows[0].weight
    );
    assert!(
        (state.pair_rows[0].weight - super::energy::GLS_WEIGHT_CAP).abs() < 1e-6,
        "200 maximum-ratio passes must reach the cap: {}",
        state.pair_rows[0].weight
    );
    super::energy::reset_weights(&mut state);
    for row in &state.pair_rows {
        assert_eq!(row.weight, 1.0);
    }
    for rows in &state.edge_rows {
        for row in rows {
            assert_eq!(row.weight, 1.0);
        }
    }
}

/// A sweep runs the Algorithm-8 pass exactly once, over every row, whether or
/// not it improved anything - and **a worker sweep runs none at all.**
///
/// The second clause is the Algorithm-10 half. Eight workers clone one master
/// and each runs `worker_sweep`; only the master then runs `gls_update`. A
/// worker that updated its own weights would be descending a landscape none of
/// its rivals could see, which is a different algorithm and not the one either
/// consultant signed. The old stall hook - which this clause replaces - is gone
/// with the jump seam it belonged to.
#[test]
fn every_sweep_runs_exactly_one_weight_pass_and_a_worker_sweep_runs_none() {
    let fixture = Fixture::squares(3, 20.0);
    let stacked = pose_at(10.0, 10.0);
    let (sources, contract, mut state) =
        state_of_poses(&fixture, vec![stacked, stacked, stacked], 300.0);
    let config = DescentConfig::derive(&contract, &sources, 0);
    let mut descent = super::descent::Descent::new(config, vec![true, true, true]);
    let mut work = WorkVector::default();

    descent.sweep(&mut state, &sources, &contract, &mut work);
    assert_eq!(work.weight_updates, 1);
    descent.sweep(&mut state, &sources, &contract, &mut work);
    assert_eq!(work.weight_updates, 2);

    let before: Vec<u64> = state.pair_rows.iter().map(|row| row.weight.to_bits()).collect();
    let outcome = descent.worker_sweep(
        &mut state,
        &sources,
        &contract,
        #[cfg(feature = "minimum-conflict-binary-close")]
        None,
        &mut work,
    );
    assert_eq!(
        work.weight_updates, 2,
        "a worker sweep charges no weight pass"
    );
    assert_eq!(outcome.active_rows, 0, "and reports none");
    let after: Vec<u64> = state.pair_rows.iter().map(|row| row.weight.to_bits()).collect();
    assert_eq!(before, after, "every weight is untouched, to the bit");
}

#[cfg(feature = "minimum-conflict-binary-close")]
#[test]
fn consumed_worker_order_digest_is_stable_and_order_sensitive() {
    let mut first = super::ConsumedOrderTrace::default();
    first.observe(22, 0, &[4, 1, 7]);
    first.observe(22, 1, &[1, 4]);
    let mut replay = super::ConsumedOrderTrace::default();
    replay.observe(22, 0, &[4, 1, 7]);
    replay.observe(22, 1, &[1, 4]);
    let mut reordered = super::ConsumedOrderTrace::default();
    reordered.observe(22, 0, &[1, 4, 7]);
    reordered.observe(22, 1, &[1, 4]);

    assert_eq!(first.digest_hex(), replay.digest_hex());
    assert_ne!(first.digest_hex(), reordered.digest_hex());
    assert_eq!((first.sweeps, first.slots), (2, 5));
}

/// A sweep advances the work quota by one per piece however small the colliding
/// set is, so a converged trajectory reaches its budget instead of spinning.
#[test]
fn a_sweep_advances_the_quota_by_one_per_piece() {
    let fixture = Fixture::squares(4, 20.0);
    let apart = vec![
        pose_at(20.0, 20.0),
        pose_at(120.0, 20.0),
        pose_at(20.0, 120.0),
        pose_at(120.0, 120.0),
    ];
    let (sources, contract, mut state) = state_of_poses(&fixture, apart, 300.0);
    assert_eq!(fold(&state).raw, 0.0, "a converged state");
    let config = DescentConfig::derive(&contract, &sources, 0);
    let mut descent = super::descent::Descent::new(config, vec![true; 4]);
    let mut work = WorkVector::default();
    let outcome = descent.sweep(&mut state, &sources, &contract, &mut work);
    assert_eq!(outcome.relocated, 0, "nothing was in the colliding set");
    assert_eq!(descent.proposals, 4, "and the quota still advanced");
    assert_eq!(
        work.piece_proposals, descent.proposals,
        "the counter the evidence prints beside proposalBudget tracks the ordinal"
    );
    assert_eq!(work.sample_evaluations, 0);
}

// -------------------------------------------------------- interior witness ---

/// **Every piece of both campaign fixtures has a witness inside its own
/// material.**
///
/// Arbitration 1's whole content, measured on the two request populations the
/// campaign runs: mixed-61's 61 source pieces and triangle-20's 20. The
/// assertion is the even-odd containment test the disruption itself uses, so a
/// witness that passed here and failed there would be impossible.
///
/// The comparison arm is the point of the arbitration: for each piece the area
/// centroid is measured with the same predicate, and the count of centroids
/// that fall *outside* their own material is reported. On a population where
/// that count is zero the arbitration costs nothing; where it is not, this test
/// is the evidence for it.
#[test]
fn every_fixture_piece_has_an_interior_witness_inside_its_own_material() {
    for (label, path) in [
        (
            "mixed-61",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/mixed-61/mixed61-request-exact-clearance.json"
            ),
        ),
        (
            "triangle-20",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/triangle-20/2000x2700-compact/request.json"
            ),
        ),
    ] {
        let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{label}: {error}"));
        let document: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
        let sag = document
            .pointer("/options/irregularSettings/geometry/flatteningSagToleranceMm")
            .or_else(|| document.pointer("/settings/geometry/flatteningSagToleranceMm"))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let imported: Vec<crate::domain::ImportedPiece> = serde_json::from_value(
            document
                .get("sourcePieces")
                .cloned()
                .unwrap_or_else(|| panic!("{label}: no sourcePieces")),
        )
        .unwrap_or_else(|error| panic!("{label}: {error}"));
        assert!(!imported.is_empty(), "{label}: no pieces");

        let mut centroids_outside = 0usize;
        for piece in &imported {
            let polygon =
                crate::geometry::general_source::polygon_set_from_imported_piece(piece, sag)
                    .unwrap_or_else(|error| panic!("{label} {}: {error:?}", piece.id.0));
            let source = PieceSource::of(&piece.id.0, &polygon)
                .unwrap_or_else(|error| panic!("{label} {}: {error}", piece.id.0));
            let ring = &source.decomposition.ring;
            assert!(
                point_in_ring(source.interior_witness, ring),
                "{label} {}: the interior witness {:?} is outside its own ring",
                piece.id.0,
                source.interior_witness
            );
            if !point_in_ring(source.centroid, ring) {
                centroids_outside += 1;
            }
            assert!(
                source.convex_hull_area_mm2 >= source.area_mm2 - 1e-6,
                "{label} {}: hull area {} below ring area {}",
                piece.id.0,
                source.convex_hull_area_mm2,
                source.area_mm2
            );
            assert!(
                source.diameter_mm > 0.0 && source.min_bbox_dim_mm > 0.0,
                "{label} {}: degenerate scale",
                piece.id
            );
        }
        // Reported, not asserted: the arbitration is about what an area
        // centroid *can* do, not about what it happens to do here. On both of
        // these populations the count is **zero** - every piece is convex or
        // mildly nonconvex enough that its area centroid is interior too - so
        // the arbitration buys nothing measurable on mixed-61 or triangle-20
        // and is taken for the guarantee, not for the fixture.
        println!("{label}: {centroids_outside} area centroids outside their own material");
    }
}

// ------------------------------------------------------------- disruption ---

/// **The swap and the rigid follow.**
///
/// Three pieces: two large ones far apart, and a small one sitting where the
/// big square is about to *land*. After the disruption the two large pieces have
/// exchanged poses, and the small one - which the arriving square would have
/// engulfed - has been sent into the space that square vacated, by exactly the
/// rigid map that takes the square's new frame back to its old one.
///
/// The direction is the source's and it is checked here because it is easy to
/// get backwards: `optimizer/explore.rs::disrupt_solution` asks the containment
/// question *after* both swaps and maps `T_old . T_new^-1`, so the followers are
/// the occupants of the arriving piece's destination, not of its origin. Its
/// comment says why - the huge item creates a large empty space and the items
/// that surrounded the smaller one are the ones sent into it.
#[test]
fn a_disruption_swaps_two_large_pieces_and_carries_their_interior_followers() {
    // A big square, a tall bar of clearly different area and diameter, and a
    // small square parked where the big square is going.
    let fixture = Fixture {
        polygons: vec![
            polygon(&square(0.0, 0.0, 80.0)),
            polygon(&[[0.0, 0.0], [40.0, 0.0], [40.0, 100.0], [0.0, 100.0]]),
            polygon(&square(0.0, 0.0, 6.0)),
        ],
        ids: vec![
            "big-square".to_owned(),
            "tall-bar".to_owned(),
            "passenger".to_owned(),
        ],
    };
    let poses = vec![
        pose_at(20.0, 20.0),
        pose_at(100.0, 200.0),
        // Inside the big square's *destination* footprint, (100,200)-(180,280),
        // and outside the bar's, (20,20)-(60,120).
        pose_at(110.0, 210.0),
    ];
    let (sources, contract, mut state) = state_of_poses(&fixture, poses.clone(), 380.0);

    // The large set and the distinctness rule, before anything moves.
    let large = large_pieces(&sources);
    assert!(large.contains(&0) && large.contains(&1), "large set {large:?}");
    assert!(
        !large.contains(&2),
        "a 36 mm² piece is not in the top 75 % of hull area: {large:?}"
    );
    assert!(
        is_distinct_enough(&sources[0], &sources[1]),
        "6400 mm² / 113.1 mm diameter against 4000 mm² / 107.7 mm"
    );
    assert!(
        !is_distinct_enough(&sources[0], &sources[0]),
        "a piece is never distinct from itself"
    );

    let mut work = WorkVector::default();
    let outcome = disrupt(
        &mut state,
        &sources,
        &contract,
        &[true, true, true],
        0,
        0,
        0,
        &mut work,
    );
    assert!(outcome.fired);
    let (first, second) = outcome.swapped.expect("a swapped pair");
    assert!(outcome.distinct, "both large pieces pass the AND filter");
    assert_eq!(
        [first.min(second), first.max(second)],
        [0, 1],
        "the two large pieces are the ones that swapped"
    );
    assert_eq!(work.disruptions, 1);

    // The swap itself.
    assert_eq!(state.poses[first].tx_mm, poses[second].tx_mm);
    assert_eq!(state.poses[first].ty_mm, poses[second].ty_mm);
    assert_eq!(state.poses[second].tx_mm, poses[first].tx_mm);
    assert_eq!(state.poses[second].ty_mm, poses[first].ty_mm);

    // The follow: the passenger moved, and by the square's own map, new frame
    // back to old.
    assert_eq!(outcome.followers, vec![2], "followers {:?}", outcome.followers);
    let expected = carry(poses[2], state.poses[0], poses[0]);
    assert!(
        (state.poses[2].tx_mm - expected.tx_mm).abs() < 1e-9
            && (state.poses[2].ty_mm - expected.ty_mm).abs() < 1e-9
            && (state.poses[2].theta_deg - expected.theta_deg).abs() < 1e-9,
        "the passenger went to {:?}, not to {expected:?}",
        state.poses[2]
    );
    // It landed in the vacancy the square left, not beside the square.
    assert!(
        (state.poses[2].tx_mm - 30.0).abs() < 1e-9
            && (state.poses[2].ty_mm - 30.0).abs() < 1e-9,
        "the passenger should have taken the square's old corner: {:?}",
        state.poses[2]
    );
    // The map is rigid: the offset the passenger had from the *arriving* square
    // is the offset it keeps from that square's origin. The host is piece 0
    // whichever of the two the draw called `first`, because the passenger sits
    // in piece 0's destination.
    let host_new = poses[1];
    let host_old = poses[0];
    let before = libm::hypot(
        poses[2].tx_mm - host_new.tx_mm,
        poses[2].ty_mm - host_new.ty_mm,
    );
    let after = libm::hypot(
        state.poses[2].tx_mm - host_old.tx_mm,
        state.poses[2].ty_mm - host_old.ty_mm,
    );
    assert!(
        (after - before).abs() < 1e-9,
        "the map is rigid: {before} mm from the destination before, {after} mm \
         from the origin after"
    );

    // Every cache is consistent on return, so the caller can separate at once.
    let incremental = fold(&state);
    let mut cold = state.clone();
    rebuild_all(&mut cold, &contract, &mut work);
    assert_eq!(incremental.raw.to_bits(), fold(&cold).raw.to_bits());
}

/// The rigid map itself, and the angle it maps through the receiving piece's
/// allowed set.
#[test]
fn the_disruption_map_is_rigid_and_respects_a_frozen_angle() {
    let from = Pose {
        tx_mm: 10.0,
        ty_mm: 20.0,
        theta_deg: 0.0,
        mirrored: false,
    };
    let to = Pose {
        tx_mm: 200.0,
        ty_mm: -30.0,
        theta_deg: 90.0,
        mirrored: false,
    };
    let follower = Pose {
        tx_mm: 40.0,
        ty_mm: 20.0,
        theta_deg: 15.0,
        mirrored: true,
    };
    let carried = carry(follower, from, to);
    // A 30 mm arm along +x, turned by 90°, is a 30 mm arm along +y.
    assert!((carried.tx_mm - 200.0).abs() < 1e-9, "{carried:?}");
    assert!((carried.ty_mm - 0.0).abs() < 1e-9, "{carried:?}");
    assert!((carried.theta_deg - 105.0).abs() < 1e-12, "{carried:?}");
    assert!(carried.mirrored, "a rigid follow never mirrors");
    // Distance to the host is preserved exactly.
    let before = libm::hypot(follower.tx_mm - from.tx_mm, follower.ty_mm - from.ty_mm);
    let after = libm::hypot(carried.tx_mm - to.tx_mm, carried.ty_mm - to.ty_mm);
    assert!((after - before).abs() < 1e-9);

    assert_eq!(closest_feasible_angle(true, 33.0, 7.0), 33.0);
    assert_eq!(closest_feasible_angle(false, 33.0, 7.0), 7.0);
}

/// The follower cap: a disruption can move at most the layout.
#[test]
fn a_disruption_never_moves_more_pieces_than_the_layout_has() {
    let fixture = Fixture::squares(5, 20.0);
    let poses = (0..5)
        .map(|index| pose_at(30.0 + index as f64 * 25.0, 30.0))
        .collect::<Vec<_>>();
    let (sources, contract, mut state) = state_of_poses(&fixture, poses, 300.0);
    let mut work = WorkVector::default();
    let outcome = disrupt(
        &mut state,
        &sources,
        &contract,
        &[true; 5],
        3,
        1,
        2,
        &mut work,
    );
    assert!(outcome.fired);
    assert!(
        work.disruption_moves <= state.poses.len() as u64,
        "moved {} of {} pieces",
        work.disruption_moves,
        state.poses.len()
    );
}

// -------------------------------------------------------- the sample stream ---

/// The permutation and the sample stream are functions of the key alone, and
/// two workers on the same master state get two different orders.
#[test]
fn the_sweep_permutation_is_a_function_of_its_key() {
    let fixture = Fixture::squares(6, 20.0);
    let stacked = (0..6).map(|_| pose_at(10.0, 10.0)).collect::<Vec<_>>();
    let (_, _, state) = state_of_poses(&fixture, stacked, 300.0);

    let mut first = Vec::new();
    let mut again = Vec::new();
    let mut other_worker = Vec::new();
    let key = RelocateKey {
        seed: 4,
        bite: 2,
        iteration: 9,
        worker: 0,
    };
    colliding_permutation(&state, key, &mut first);
    colliding_permutation(&state, key, &mut again);
    colliding_permutation(
        &state,
        RelocateKey { worker: 1, ..key },
        &mut other_worker,
    );
    assert_eq!(first, again, "the same key is the same order");
    assert_eq!(first.len(), 6, "every piece collides");
    assert_ne!(
        first, other_worker,
        "two Algorithm-10 workers must sweep in different orders"
    );
    let mut sorted = other_worker.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..6).collect::<Vec<_>>(), "it is a permutation");
}

// ============================================== the schedule's unit vectors ===
//
// The five vectors docs/cutclose-relocate-spec.md names for the schedule wave,
// plus the two counter-derived draws the regime depends on:
//
//   * cut-close bits: far-side `t_y += delta` to the bit, near side untouched
//     to the bit, `t_x` and `theta` frozen;
//   * a refused publication does not advance `W` - including the Phi = 0 case,
//     which is a *failed separation* and not a converged one;
//   * the exact parent: a forced-nonzero-repair publication is installed whole,
//     every cache is rebuilt from it, and the next bite's `D` is the published
//     raw depth;
//   * the eight-worker tournament is a function of its key - same winner
//     ordinals, same master fingerprints, whatever the operating system did
//     with the threads;
//   * the TimeBased step against a fake elapsed.

use super::homotopy::{
    compress_bite, compress_width_mm, explore_bite, normal_biased_rank, split_and_close,
    time_based_step, uniform_cut_mm, COMPRESS_SHRINK_RANGE, EXPLORE_SHRINK_STEP,
    EXPLORE_TIME_RATIO,
};
use super::{
    observe_raw, state_fingerprint, Budget, Phase, RawObservation, ScheduleConfig, ScheduleOutcome,
    SeparateLimits,
};

/// The four poses the cut-close vector splits: two below the cut and two above
/// it, all four with an angle and a translation nothing may touch.
fn cut_close_poses() -> Vec<Pose> {
    vec![
        Pose { tx_mm: 30.5, ty_mm: 12.25, theta_deg: 17.0, mirrored: false },
        Pose { tx_mm: 60.125, ty_mm: 30.0, theta_deg: -5.5, mirrored: false },
        Pose { tx_mm: 30.75, ty_mm: 120.0, theta_deg: 91.25, mirrored: false },
        Pose { tx_mm: 90.0, ty_mm: 150.5, theta_deg: 0.0, mirrored: false },
    ]
}

/// **Split-and-close moves exactly one side, in exactly one coordinate.**
///
/// Grok review 12 Round 2 §6.3's FAST vector, verbatim: "cut-close bits:
/// far-side `t_y += delta`, near-side `t_y` unchanged, `t_x` and `theta`
/// frozen". Every comparison here is on `to_bits`, because the failure this
/// guards against is not a large error - it is an affine squeeze, or a stray
/// `x` nudge, that would look correct at four decimal places and would make the
/// homotopy a different one.
#[test]
fn the_cut_close_moves_only_the_far_side_and_only_in_y() {
    let fixture = Fixture::squares(4, 20.0);
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let before = cut_close_poses();
    let split_y = 100.0;
    let delta = -0.18297600000000001;
    let mut after = before.clone();
    let moved = split_and_close(&sources, &mut after, delta, split_y);

    let mut far = 0usize;
    for (index, (entry, exit)) in before.iter().zip(&after).enumerate() {
        assert_eq!(
            exit.tx_mm.to_bits(),
            entry.tx_mm.to_bits(),
            "piece {index}: t_x is frozen to the bit"
        );
        assert_eq!(
            exit.theta_deg.to_bits(),
            entry.theta_deg.to_bits(),
            "piece {index}: theta is frozen to the bit"
        );
        assert_eq!(exit.mirrored, entry.mirrored, "piece {index}: the mirror is frozen");
        let centre_y = transformed_centroid(&sources[index], *entry)[1];
        if centre_y > split_y {
            far += 1;
            assert_eq!(
                exit.ty_mm.to_bits(),
                (entry.ty_mm + delta).to_bits(),
                "piece {index} is on the far side: t_y += delta, to the bit"
            );
        } else {
            assert_eq!(
                exit.ty_mm.to_bits(),
                entry.ty_mm.to_bits(),
                "piece {index} is on the near side and must not move at all"
            );
        }
    }
    assert_eq!(far, 2, "the fixture must actually straddle the cut");
    assert_eq!(moved, far, "the returned count is the far side's size");
}

/// The explore bite's three numbers: 0.1 %, a centre cut, and `delta = T - D`.
#[test]
fn the_explore_bite_is_a_tenth_of_a_percent_at_mid_depth() {
    let fixture = Fixture::squares(4, 20.0);
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let mut poses = cut_close_poses();
    let width = 182.976;
    let bite = explore_bite(&sources, &mut poses, width);
    assert_eq!(EXPLORE_SHRINK_STEP, 0.001, "Sparrow's frozen shrink_step");
    assert_eq!(bite.width_before_mm.to_bits(), width.to_bits());
    assert_eq!(
        bite.width_after_mm.to_bits(),
        (width * (1.0 - 0.001)).to_bits(),
        "W <- W (1 - 0.001)"
    );
    assert_eq!(
        bite.delta_mm.to_bits(),
        (bite.width_after_mm - width).to_bits(),
        "delta is T - D"
    );
    assert!(bite.delta_mm < 0.0, "and it is negative");
    assert_eq!(
        bite.split_y_mm.to_bits(),
        (width / 2.0).to_bits(),
        "explore cuts at mid-depth: their split_position = None"
    );
    assert_eq!(EXPLORE_TIME_RATIO, 0.8, "8 s of a 10 s budget explore");
}

/// **The TimeBased step, against a fake elapsed.**
///
/// The whole point of the signature is that the clock is somebody else's: a
/// wall phase hands it seconds and a fixed-work replay hands it a bite ordinal,
/// and the decay is the same either way. So the vector is arithmetic, and it
/// pins the two ends, both saturations, and monotonicity.
#[test]
fn the_time_based_step_interpolates_against_a_fake_elapsed() {
    let (start, end) = COMPRESS_SHRINK_RANGE;
    assert_eq!(start, 0.0005, "0.05 %");
    assert_eq!(end, 0.00001, "0.001 %");
    assert_eq!(
        time_based_step(0.0, 2.0).to_bits(),
        start.to_bits(),
        "at the start of the phase the step is the start of the range, exactly"
    );
    assert_eq!(
        time_based_step(1.0, 2.0).to_bits(),
        (start + (end - start) * 0.5).to_bits(),
        "linear in elapsed/limit"
    );
    // `start + (end - start) * 1.0` is `end` in real arithmetic and one ulp
    // away from it in this one; the interpolation is left as written rather
    // than special-cased, so the vector asks for the arithmetic it does.
    let at_end = time_based_step(2.0, 2.0);
    assert!(
        (at_end - end).abs() <= 1e-18,
        "at the phase limit the step is the end of the range: {at_end}"
    );
    assert_eq!(
        time_based_step(9.0, 2.0).to_bits(),
        at_end.to_bits(),
        "past the phase limit the step saturates rather than inverting"
    );
    assert_eq!(
        time_based_step(-1.0, 2.0).to_bits(),
        start.to_bits(),
        "before the phase began it is the start of the range"
    );
    assert_eq!(
        time_based_step(1.0, 0.0).to_bits(),
        start.to_bits(),
        "an unmeasured phase has not decayed"
    );
    let mut previous = f64::INFINITY;
    for tick in 0..=10 {
        let step = time_based_step(tick as f64, 10.0);
        assert!(step < previous, "the decay is strictly monotone at tick {tick}");
        assert!(
            step >= end - 1e-18 && step <= start,
            "and stays inside the range"
        );
        previous = step;
    }
    assert_eq!(
        compress_width_mm(100.0, 0.0005).to_bits(),
        (100.0f64 * (1.0 - 0.0005)).to_bits()
    );
}

/// The compression cut and the pool rank are functions of their keys alone.
#[test]
fn the_compress_cut_and_the_pool_rank_are_functions_of_their_keys() {
    let fixture = Fixture::squares(2, 20.0);
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let edge = contract.physical_edge_clearance_mm();

    let first = uniform_cut_mm(&contract, 182.976, 7, 3);
    assert_eq!(
        first.to_bits(),
        uniform_cut_mm(&contract, 182.976, 7, 3).to_bits(),
        "the same key is the same cut"
    );
    assert_ne!(
        first.to_bits(),
        uniform_cut_mm(&contract, 182.976, 7, 4).to_bits(),
        "the next bite draws a different cut"
    );
    for bite in 0..64 {
        let cut = uniform_cut_mm(&contract, 182.976, 7, bite);
        assert!(cut >= edge && cut <= 182.976, "uniform in (edge, W): {cut}");
    }

    // The bite carries the cut it drew, and closes the far side of it.
    let mut poses = vec![
        Pose { tx_mm: 20.0, ty_mm: 20.0, theta_deg: 0.0, mirrored: false },
        Pose { tx_mm: 20.0, ty_mm: 120.0, theta_deg: 0.0, mirrored: false },
    ];
    let bite = compress_bite(&sources, &mut poses, &contract, 182.976, 0.0005, 7, 3);
    assert_eq!(bite.split_y_mm.to_bits(), first.to_bits());
    assert_eq!(bite.step.to_bits(), 0.0005f64.to_bits());
    assert_eq!(
        bite.width_after_mm.to_bits(),
        (182.976f64 * (1.0 - 0.0005)).to_bits()
    );

    // The Normal(0, 0.25) bias: in range, keyed, and skewed toward the best
    // entries of a loss-sorted pool.
    assert_eq!(normal_biased_rank(0, 1, 2, 3), 0, "an empty pool has no rank");
    assert_eq!(normal_biased_rank(1, 1, 2, 3), 0);
    let mut best_half = 0usize;
    for attempt in 0..512u64 {
        let rank = normal_biased_rank(40, 11, 5, attempt);
        assert!(rank < 40, "the rank indexes the pool");
        if rank < 20 {
            best_half += 1;
        }
    }
    assert_eq!(
        normal_biased_rank(40, 11, 5, 17),
        normal_biased_rank(40, 11, 5, 17),
        "the same key is the same rank"
    );
    assert!(
        best_half > 400,
        "|N(0, 0.25)| * len puts the draw in the better half almost always: {best_half}/512"
    );
}

// ------------------------------------------------------ the loop's fixtures ---

/// A two-square layout whose *material* gap is 5.0 mm minus 3 µm: inside the
/// canonical band, so the round kernel refuses and the bounded repair moves a
/// pose. That is what makes it the forced-nonzero-repair fixture.
fn banded_deficit_engine<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    incumbent_depth_mm: f64,
) -> Engine<'a> {
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let sources = super::state::piece_sources(pieces).expect("sources");
    let poses = vec![
        Pose { tx_mm: 20.0, ty_mm: 25.0, theta_deg: 0.0, mirrored: false },
        Pose { tx_mm: 45.0 - 0.003, ty_mm: 25.0, theta_deg: 0.0, mirrored: false },
    ];
    let config = IcsConfig {
        target_depth_mm: incumbent_depth_mm,
        proposal_budget: 0,
        relocate_eval_budget: u64::MAX,
        checkpoint_every_sweeps: u64::MAX,
        descent: DescentConfig::derive(&contract, &sources, 0),
        limits: PublicationLimits::default(),
    };
    let incumbent = super::state::ExactIncumbent {
        placements: Vec::new(),
        raw_source_depth_mm: incumbent_depth_mm,
        from_constructor: true,
        placement_fingerprint: "the-constructor".to_owned(),
    };
    Engine::from_poses(pieces, settings, sources, contract, poses, incumbent, config)
}

fn two_squares() -> Fixture {
    Fixture {
        polygons: vec![polygon(&square(0.0, 0.0, 20.0)), polygon(&square(0.0, 0.0, 20.0))],
        ids: vec!["a".to_owned(), "b".to_owned()],
    }
}

/// Two explore bites, one attempt each: enough to publish and bite again, and
/// small enough to run in a debug build.
const TWO_BITES: Budget = Budget::FixedWork {
    explore_bites: 2,
    compress_bites: 0,
    attempts_per_bite: 1,
    iterations_per_separation: 2,
};

/// **The exact parent: a repaired publication becomes the continuous state, and
/// the next `D` is the published raw depth.**
///
/// Sol review 17 Round 2's mandatory addition 1, all six clauses: a bite reaches
/// the 4 µm band; the repair moves at least one pose; the publication succeeds;
/// the engine's poses equal `Publication.poses`; the geometry and every row
/// equal a cold rebuild *from those poses*; and the next bite derives `D` from
/// the published raw depth rather than from the target or the pre-repair proxy
/// depth.
///
/// The cold-rebuild clause is built from `build_geometry`, not from
/// `rebuild_all` on the engine's own geometry - `rebuild_all` measures the
/// cached transforms, so it would happily agree with a *stale* geometry, and a
/// stale geometry after a pose install is exactly the failure being excluded.
#[test]
fn a_repaired_publication_becomes_the_next_bites_exact_parent() {
    let fixture = two_squares();
    let pieces = fixture.pieces();

    // --- the install, on its own.
    let mut engine = banded_deficit_engine(&pieces, f64::INFINITY);
    let totals = engine.totals();
    assert!(
        totals.max_violation_mm > 0.0 && totals.max_violation_mm <= 0.004,
        "the deficit must sit inside the 4 µm band: {totals:?}"
    );
    let attempt = engine.attempt_publication();
    let publication = attempt.publication.expect("a banded deficit must publish");
    assert!(
        publication.repair_rows >= 1 && publication.repair_max_displacement_mm > 0.0,
        "the vector needs a repair that actually moved a pose: {publication:?}"
    );
    let pre_repair = engine.state().poses.clone();
    assert!(
        pre_repair
            .iter()
            .zip(&publication.poses)
            .any(|(before, after)| before.tx_mm.to_bits() != after.tx_mm.to_bits()
                || before.ty_mm.to_bits() != after.ty_mm.to_bits()),
        "the repaired poses must differ from the state's, or the vector is vacuous"
    );

    engine.install_publication(&publication);
    for (index, (installed, published)) in engine
        .state()
        .poses
        .iter()
        .zip(&publication.poses)
        .enumerate()
    {
        assert_eq!(
            installed.tx_mm.to_bits(),
            published.tx_mm.to_bits(),
            "piece {index}: the state's poses are the publication's"
        );
        assert_eq!(installed.ty_mm.to_bits(), published.ty_mm.to_bits());
        assert_eq!(installed.theta_deg.to_bits(), published.theta_deg.to_bits());
    }
    assert_eq!(
        engine.state().target_depth_mm.to_bits(),
        publication.raw_source_depth_mm.to_bits(),
        "the width becomes the published raw depth"
    );

    let sources = super::state::piece_sources(&pieces).expect("sources");
    let contract = Contract::from_settings(test_settings());
    let cold_geometry = build_geometry(&sources, &publication.poses);
    for (index, (cached, cold)) in engine
        .geometry()
        .ring_points
        .iter()
        .zip(&cold_geometry.ring_points)
        .enumerate()
    {
        assert_eq!(
            cached[0].to_bits(),
            cold[0].to_bits(),
            "ring point {index} was not re-transformed after the install"
        );
        assert_eq!(cached[1].to_bits(), cold[1].to_bits());
    }
    let count = publication.poses.len();
    let mut cold = IcsState {
        poses: publication.poses.clone(),
        geometry: cold_geometry,
        pair_rows: vec![PairRow::default(); pair_count(count)],
        edge_rows: vec![[EdgeRow::default(); 4]; count],
        target_depth_mm: publication.raw_source_depth_mm,
    };
    let mut work = WorkVector::default();
    rebuild_all(&mut cold, &contract, &mut work);
    for (index, (cached, fresh)) in engine
        .state()
        .pair_rows
        .iter()
        .zip(&cold.pair_rows)
        .enumerate()
    {
        assert_eq!(
            cached.violation_mm.to_bits(),
            fresh.violation_mm.to_bits(),
            "pair row {index} disagrees with a cold rebuild of the installed poses"
        );
    }
    for (piece, (cached, fresh)) in
        engine.state().edge_rows.iter().zip(&cold.edge_rows).enumerate()
    {
        for edge in 0..4 {
            assert_eq!(
                cached[edge].violation_mm.to_bits(),
                fresh[edge].violation_mm.to_bits(),
                "piece {piece} edge {edge} disagrees with a cold rebuild"
            );
        }
    }

    // --- the two bites.
    let mut engine = banded_deficit_engine(&pieces, 60.0);
    let schedule = ScheduleConfig {
        workers: 2,
        ..ScheduleConfig::default()
    };
    let run = engine.run_cutclose(schedule, TWO_BITES);
    assert_eq!(run.start_depth_mm.to_bits(), 60.0f64.to_bits(), "W enters at D*");
    assert!(
        !run.publications.is_empty(),
        "the banded deficit must publish inside the first bite: {:?}",
        run.bites
    );
    let first = &run.publications[0];
    assert_eq!(first.phase, Phase::Explore);
    assert!(first.repair_rows >= 1, "the repair must have fired: {first:?}");
    assert_eq!(
        first.parent_fingerprint, "the-constructor",
        "the bite's parent is the layout it started from"
    );
    for row in &run.publications {
        assert!(
            row.published_raw_depth_mm <= row.target_depth_mm,
            "every publication is inside the strip it was published in: {row:?}"
        );
    }
    assert_eq!(
        run.depth_mm.to_bits(),
        run.publications
            .last()
            .expect("a publication")
            .published_raw_depth_mm
            .to_bits(),
        "D is the PUBLISHED raw depth, not the target and not the proxy depth"
    );
    assert_eq!(run.bites.len(), 2, "the publication licensed a second bite");
    assert_eq!(
        run.bites[1].bite.width_before_mm.to_bits(),
        first.published_raw_depth_mm.to_bits(),
        "the second bite shrinks from the depth the first one published"
    );
    assert_eq!(
        run.bites[1].bite.width_after_mm.to_bits(),
        (first.published_raw_depth_mm * (1.0 - EXPLORE_SHRINK_STEP)).to_bits()
    );
    // The exact-parent chain: every bite after the first one names the layout
    // its predecessor published.
    for pair in run.publications.windows(2) {
        assert_eq!(
            pair[1].parent_fingerprint, pair[0].placement_fingerprint,
            "each publication's parent is the previous publication"
        );
    }
}

/// **A Phi = 0 layout whose publication is refused does not advance `W`.**
///
/// Grok review 12 Round 1 §5.2 names the deception this excludes - the
/// "proxy-legal parent", a shrink taken from a `Phi = 0` state the exact
/// authorities would reject - and Sol review 17 Round 2 §5 names what the loop
/// must do instead: "If proxy Phi reaches zero but exact publication refuses,
/// classify it as a failed separation: otherwise every piece is skipped forever
/// and the loop spins at a false legal state."
///
/// The refusal here is the publication gate's own: a 1 km minimum improvement,
/// so no layout at any depth can ever clear it. The state is genuinely
/// collision-free at the bitten width, so the colliding set is empty and no
/// sweep could do anything - which is precisely the spin this clause prevents.
#[test]
fn a_refused_publication_never_advances_the_width() {
    let fixture = Fixture::squares(6, 20.0);
    let pieces = fixture.pieces();
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = (0..pieces.len())
        .map(|index| Pose {
            tx_mm: 20.0 + (index % 3) as f64 * 60.0,
            ty_mm: 20.0 + (index / 3) as f64 * 60.0,
            theta_deg: 0.0,
            mirrored: false,
        })
        .collect::<Vec<_>>();
    let mut limits = PublicationLimits::default();
    limits.minimum_improvement_mm = 1_000_000.0;
    let config = IcsConfig {
        target_depth_mm: 360.0,
        proposal_budget: 0,
        relocate_eval_budget: u64::MAX,
        checkpoint_every_sweeps: u64::MAX,
        descent: DescentConfig::derive(&contract, &sources, 0),
        limits,
    };
    let incumbent = super::state::ExactIncumbent {
        placements: Vec::new(),
        raw_source_depth_mm: 360.0,
        from_constructor: true,
        placement_fingerprint: "the-constructor".to_owned(),
    };
    let mut engine = Engine::from_poses(
        &pieces, settings, sources, contract, poses, incumbent, config,
    );
    assert_eq!(engine.totals().raw, 0.0, "the fixture must be Phi-feasible");

    let schedule = ScheduleConfig {
        workers: 2,
        ..ScheduleConfig::default()
    };
    let run = engine.run_cutclose(
        schedule,
        Budget::FixedWork {
            explore_bites: 3,
            compress_bites: 0,
            attempts_per_bite: 3,
            iterations_per_separation: 2,
        },
    );

    assert!(
        run.publications.is_empty(),
        "nothing may publish: {:?}",
        run.publications
    );
    assert_eq!(run.explore_bites, 0, "no bite succeeded");
    assert_eq!(
        run.bites.len(),
        1,
        "and no second bite was ever licensed: {:?}",
        run.bites.iter().map(|row| row.bite).collect::<Vec<_>>()
    );
    assert_eq!(
        run.depth_mm.to_bits(),
        run.start_depth_mm.to_bits(),
        "D did not move"
    );
    assert_eq!(
        run.bites[0].bite.width_after_mm.to_bits(),
        (360.0f64 * (1.0 - EXPLORE_SHRINK_STEP)).to_bits(),
        "exactly one 0.1 % bite was taken and it stayed there"
    );
    assert_eq!(
        run.bites[0].attempts, 3,
        "it spent its whole attempt quota failing"
    );
    assert!(
        run.bites[0].proxy_band_reached,
        "the state was inside the band the whole time - that is what makes it a refusal"
    );
    assert!(
        engine.incumbent.from_constructor,
        "best_exact never moved off the constructor"
    );
}

/// The tournament vector's fixture: twelve 20 mm squares in a strip that is too
/// shallow to hold them.
///
/// The strip has to be **infeasible by area**, or the vector measures nothing.
/// Twelve pieces at `c_pair = 5` need `12 * 25 * 25 = 7,500` mm² and the usable
/// width is 190 mm, so a depth of 40 mm - which leaves `40 - 5 - 5 = 30` mm of
/// usable band, or 5,700 mm² - cannot hold them at any arrangement. Phi
/// therefore never reaches zero, no worker ever ties at the floor, and the
/// eight of them really do have to be compared. A roomy strip is not a weaker
/// test of the merge; it is not a test of the merge at all, because eight
/// workers that all clear it tie at zero and the ordinal decides.
fn tournament_run(workers: usize) -> (ScheduleOutcome, usize) {
    let fixture = Fixture::squares(12, 20.0);
    let pieces = fixture.pieces();
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = (0..pieces.len())
        .map(|index| Pose {
            tx_mm: 20.0 + (index % 4) as f64 * 22.0,
            ty_mm: 20.0 + (index / 4) as f64 * 22.0,
            theta_deg: 0.0,
            mirrored: false,
        })
        .collect::<Vec<_>>();
    let config = IcsConfig {
        target_depth_mm: 40.0,
        proposal_budget: 0,
        relocate_eval_budget: u64::MAX,
        checkpoint_every_sweeps: u64::MAX,
        descent: DescentConfig::derive(&contract, &sources, 4),
        limits: PublicationLimits::default(),
    };
    let incumbent = super::state::ExactIncumbent {
        placements: Vec::new(),
        raw_source_depth_mm: 40.0,
        from_constructor: true,
        placement_fingerprint: "the-constructor".to_owned(),
    };
    let mut engine = Engine::from_poses(
        &pieces, settings, sources, contract, poses, incumbent, config,
    );
    let schedule = ScheduleConfig {
        workers,
        record_fingerprints: true,
        ..ScheduleConfig::default()
    };
    let outcome = engine.run_cutclose(
        schedule,
        Budget::FixedWork {
            explore_bites: 1,
            compress_bites: 1,
            attempts_per_bite: 1,
            iterations_per_separation: 2,
        },
    );
    let count = engine.state().poses.len();
    (outcome, count)
}

/// **The eight-worker tournament is a function of its key.**
///
/// Sol review 17 Round 2's mandatory addition 2 asks FAST for two processes
/// agreeing on "each worker seed, each master snapshot, winning worker ordinal,
/// pose and weight fingerprint after every master iteration, exact parent after
/// every bite". This is the in-process half of that - two independent
/// trajectories through the same eight-thread tournament - and it is the half
/// that is sensitive to the thing threads break: the workers are joined in
/// ordinal order and the merge is a serial scan, so the operating system's
/// scheduling cannot reach the answer. The evidence agent owns the two-process
/// cell.
///
/// It also pins the fan-out in the work vector. Eight workers really do sweep
/// the same master state, so `pieceProposals` is exactly `workers * n` per
/// master iteration; a "tournament" that quietly ran one worker would be
/// deterministic too, and this is what tells the two apart.
#[test]
fn the_eight_worker_tournament_is_a_function_of_its_key() {
    let (first, count) = tournament_run(8);
    let (again, _) = tournament_run(8);
    assert!(
        !first.fingerprints.is_empty(),
        "the vector needs master iterations to compare"
    );
    assert_eq!(
        first.fingerprints, again.fingerprints,
        "two runs of the same key must agree on every winner and every master state"
    );
    assert_eq!(first.trace.work, again.trace.work, "and on every counter");
    for (left, right) in first.final_poses.iter().zip(&again.final_poses) {
        assert_eq!(left.tx_mm.to_bits(), right.tx_mm.to_bits());
        assert_eq!(left.ty_mm.to_bits(), right.ty_mm.to_bits());
        assert_eq!(left.theta_deg.to_bits(), right.theta_deg.to_bits());
    }
    assert_eq!(
        first.bites.iter().map(|row| row.bite).collect::<Vec<_>>(),
        again.bites.iter().map(|row| row.bite).collect::<Vec<_>>()
    );

    // Eight sweeps per master iteration, not one.
    let iterations: u64 = first.bites.iter().map(|row| row.master_iterations).sum();
    assert!(iterations > 0, "the fixture must actually separate");
    assert_eq!(
        first.trace.work.piece_proposals,
        8 * count as u64 * iterations,
        "eight workers each sweep every piece slot of every master iteration"
    );
    assert_eq!(
        first.fingerprints.len() as u64,
        iterations,
        "one fingerprint per master iteration"
    );

    // The merge had something to choose. On this fixture Phi can never reach
    // zero, so the eight workers cannot all tie at the floor.
    assert!(
        first.fingerprints.iter().all(|row| row.contested),
        "the eight workers must reach different totals on an over-full strip: {:?}",
        first
            .fingerprints
            .iter()
            .map(|row| (row.winner, row.winner_guided, row.contested))
            .collect::<Vec<_>>()
    );
    assert!(
        first.fingerprints.iter().any(|row| row.winner != 0),
        "and some iteration must be won by a worker other than ordinal 0, or the \
         tournament is decoration: {:?}",
        first.fingerprints.iter().map(|row| row.winner).collect::<Vec<_>>()
    );

    // A single worker is a different trajectory, and its winner is always
    // ordinal 0.
    let (single, _) = tournament_run(1);
    assert!(
        single.fingerprints.iter().all(|row| row.winner == 0 && !row.contested),
        "with one worker the winner is always ordinal 0 and nothing is contested"
    );
    let single_iterations: u64 = single.bites.iter().map(|row| row.master_iterations).sum();
    assert_eq!(
        single.trace.work.piece_proposals,
        count as u64 * single_iterations
    );
    assert!(
        first
            .fingerprints
            .iter()
            .zip(&single.fingerprints)
            .any(|(many, one)| many.state != one.state),
        "the eight-worker trajectory must differ from worker 0's own"
    );
    // The merge rule itself. Both runs start their first master iteration from
    // the same master state, so the eight-worker winner cannot be worse than
    // ordinal 0's own sweep - that is what taking the minimum means. From the
    // second iteration on the two trajectories stand on different states and
    // their totals are no longer comparable, which is why this is pinned on the
    // first one alone.
    assert!(
        first.fingerprints[0].winner_guided <= single.fingerprints[0].winner_guided,
        "eight workers: {} vs ordinal 0 alone: {}",
        first.fingerprints[0].winner_guided,
        single.fingerprints[0].winner_guided
    );
}

/// The master fingerprint is over the weights as well as the poses.
///
/// Two master iterations can install the same poses on two different
/// landscapes, and the merge-determinism vector has to be able to tell those
/// apart - the weights are half of what the next tournament ranks on.
#[test]
fn the_master_fingerprint_sees_the_weights_and_not_only_the_poses() {
    let fixture = Fixture::squares(6, 20.0);
    let (_, _, state) = state_of(&fixture, 300.0);
    let bare = state_fingerprint(&state);
    let mut weighted = state.clone();
    super::energy::gls_update(&mut weighted);
    assert_ne!(
        bare,
        state_fingerprint(&weighted),
        "one Algorithm-8 pass moved every weight and nothing else"
    );
    for (left, right) in state.poses.iter().zip(&weighted.poses) {
        assert_eq!(left.tx_mm.to_bits(), right.tx_mm.to_bits());
        assert_eq!(left.ty_mm.to_bits(), right.ty_mm.to_bits());
    }
    let mut moved = state.clone();
    moved.poses[0].tx_mm += 1.0;
    assert_ne!(bare, state_fingerprint(&moved), "and it sees a moved pose");
    let mut retargeted = state.clone();
    retargeted.target_depth_mm += 0.001;
    assert_ne!(bare, state_fingerprint(&retargeted), "and the width");
}

/// The strike limits and the worker count are the published ones, and nothing
/// fitted them to a wall number.
///
/// **This vector checks literals and nothing else.** It was read for two rounds
/// as covering separator-strike *semantics* - the round 1 provenance table says
/// "identical / none" for that row - and it never touched the transition. Sol
/// review 18 §P0 names it a false green. The semantics are
/// [`the_no_improvement_counter_pauses_on_a_marginal_minimum_and_resets_only_on_two_percent`],
/// immediately below; this one is kept because "nobody retuned 200 to fit a
/// wall number" is still worth asserting, and is now honest about being all it
/// asserts.
#[test]
fn the_strike_caps_are_the_published_two_hundred_three_and_one_hundred_five() {
    assert_eq!(SeparateLimits::EXPLORE.iterations_without_improvement, 200);
    assert_eq!(SeparateLimits::EXPLORE.strikes, 3);
    assert_eq!(SeparateLimits::COMPRESS.iterations_without_improvement, 100);
    assert_eq!(SeparateLimits::COMPRESS.strikes, 5);
    assert_eq!(super::STRIKE_IMPROVEMENT_RATIO, 0.98);
    let default = ScheduleConfig::default();
    assert_eq!(default.workers, 8, "eight workers from the start");
    // Wave 3 moved the two phases' limits inside the arm. The default arm is
    // the control, and the control arm *is* the shipped `SeparateLimits`, so
    // the assertion is the same sentence read through one more accessor.
    assert_eq!(
        default.strikes,
        StrikeConfig::IterationStrikes {
            explore: SeparateLimits::EXPLORE,
            compress: SeparateLimits::COMPRESS,
        },
        "the default arm is the control, on the frozen literals"
    );
    assert_eq!(
        default.strikes.rule(Phase::Explore).patience,
        Patience::Iterations(200)
    );
    assert_eq!(default.strikes.rule(Phase::Explore).strikes, 3);
    assert_eq!(
        default.strikes.rule(Phase::Compress).patience,
        Patience::Iterations(100)
    );
    assert_eq!(default.strikes.rule(Phase::Compress).strikes, 5);
    assert!(
        !default.record_fingerprints,
        "the wall run does not pay for the per-iteration record"
    );
}

/// **The red/green state-machine vector for the inner strike predicate.**
///
/// Sol review 18 §P0 and Grok review 13 flag 3, the same sketch from both: feed
/// the no-improvement counter repeated blocks of **nine non-minima followed by
/// one 0.01 %-better minimum**.
///
/// * **Red.** Round 1's rule - `raw < min_raw => since_improvement = 0` - resets
///   on the tenth observation of every block and can never reach 200, in any
///   number of blocks. The transcript of this vector against that rule is
///   `docs/experiments/overlap-ics/cutclose-rerun/evidence/strike-red.log`,
///   reproducible from `evidence/strike-red.patch`.
/// * **Green.** The repaired rule *pauses* on each 0.01 % minimum and reaches
///   the explore limit after exactly 200 non-minimum observations.
/// * A single **>2 %** improvement resets it, and exactly 2 % does not.
///
/// The vector drives [`observe_raw`] - the same function `Engine::separate`
/// calls, and the only copy of the rule in the tree. It does not restate the
/// predicate, so it cannot pass by agreeing with a duplicate of it.
#[test]
fn the_no_improvement_counter_pauses_on_a_marginal_minimum_and_resets_only_on_two_percent() {
    let limit = SeparateLimits::EXPLORE.iterations_without_improvement;

    // ---- the vector: nine non-minima, then a 0.01 % minimum, repeated -------
    let mut min_raw = 1.0_f64;
    let mut since = 0_u64;
    let mut observations = 0_u64;
    let mut classes: Vec<RawObservation> = Vec::new();
    let mut struck_after = None;
    'blocks: for _ in 0..1_000 {
        for _ in 0..9 {
            // Strictly worse than the incumbent: a non-improvement under any
            // reading of the word.
            classes.push(observe_raw(min_raw * 1.5, &mut min_raw, &mut since));
            observations += 1;
            if since >= limit {
                struck_after = Some(observations);
                break 'blocks;
            }
        }
        // The trickle: a genuine new minimum, 200x too small to be worth 2 %.
        // This is the 1e-15-scale minimum bite 22 produced thousands of times.
        classes.push(observe_raw(min_raw * 0.9999, &mut min_raw, &mut since));
        observations += 1;
        if since >= limit {
            struck_after = Some(observations);
            break 'blocks;
        }
    }

    let nones = classes.iter().filter(|c| **c == RawObservation::None).count();
    let marginals = classes
        .iter()
        .filter(|c| **c == RawObservation::Marginal)
        .count();
    let substantials = classes
        .iter()
        .filter(|c| **c == RawObservation::Substantial)
        .count();

    // Printed, not only asserted: `cargo test` shows a failing test's stdout,
    // so the red transcript of this vector carries its own numbers.
    println!(
        "observations={observations} none={nones} marginal={marginals} \
         substantial={substantials} since={since} limit={limit} \
         struckAfter={struck_after:?} minRaw={min_raw:e}"
    );

    // GREEN: the strike arrives, after 200 non-minima and not one earlier.
    assert_eq!(
        struck_after,
        Some(222),
        "22 whole blocks of ten, then two more non-minima: the counter crosses \
         200 on the 222nd observation of this vector"
    );
    assert_eq!(nones, 200, "only non-minima counted");
    assert_eq!(since, limit, "and it stopped exactly at the explore limit");
    assert_eq!(marginals, 22, "each 0.01 % minimum paused the counter");
    assert_eq!(substantials, 0, "nothing here was worth 2 %");
    assert!(min_raw < 1.0, "the trickle really did lower the incumbent");

    // RED, stated in the classifier's own vocabulary rather than by
    // re-implementing round 1: round 1 reset on ANY new minimum, so its counter
    // was exactly the longest run of consecutive `None` in this same sequence.
    let mut run = 0_u64;
    let mut longest_run = 0_u64;
    for class in &classes {
        if *class == RawObservation::None {
            run += 1;
            longest_run = longest_run.max(run);
        } else {
            run = 0;
        }
    }
    assert_eq!(
        longest_run, 9,
        "round 1's counter never got past nine on this vector"
    );
    assert!(
        longest_run < limit,
        "which is why no separation on the 22nd bite ever struck out, and why \
         Algorithm 12 never ran there"
    );

    // ---- one >2 % improvement forgives the counter --------------------------
    let mut min_raw = 1.0_f64;
    let mut since = 0_u64;
    for _ in 0..150 {
        observe_raw(min_raw * 1.5, &mut min_raw, &mut since);
    }
    assert_eq!(since, 150);
    assert_eq!(
        observe_raw(min_raw * 0.9999, &mut min_raw, &mut since),
        RawObservation::Marginal
    );
    assert_eq!(since, 150, "a marginal minimum neither resets nor increments");
    assert_eq!(
        observe_raw(min_raw * 0.97, &mut min_raw, &mut since),
        RawObservation::Substantial
    );
    assert_eq!(since, 0, "one 3 % improvement forgives it");

    // ---- the boundary is strict, as `separator.rs`'s `<` is -----------------
    let mut min_raw = 1.0_f64;
    let mut since = 7_u64;
    assert_eq!(
        observe_raw(0.98, &mut min_raw, &mut since),
        RawObservation::Marginal,
        "exactly 2 % is not a 2 % improvement"
    );
    assert_eq!(since, 7);
    assert_eq!(min_raw, 0.98, "but it is still the new incumbent");
    let mut min_raw = 1.0_f64;
    let mut since = 7_u64;
    assert_eq!(
        observe_raw(0.979_999, &mut min_raw, &mut since),
        RawObservation::Substantial
    );
    assert_eq!(since, 0);

    // ---- equal is not an improvement ----------------------------------------
    let mut min_raw = 1.0_f64;
    let mut since = 3_u64;
    assert_eq!(
        observe_raw(1.0, &mut min_raw, &mut since),
        RawObservation::None,
        "the comparison is strict on both sides"
    );
    assert_eq!(since, 4);

    // ---- and the snapshot moves on both improving classes --------------------
    assert!(RawObservation::Substantial.is_new_minimum());
    assert!(RawObservation::Marginal.is_new_minimum());
    assert!(!RawObservation::None.is_new_minimum());
}

// ======================= the economics round's integration wave (Wave 3) ======
//
// docs/economics-round-spec.md funds three changes. Two of them are wired into
// `run_cutclose` here and the third - the persistent executor - is not, because
// its pre-committed gate said no (`economics-round/census/README.md`: largest
// prep+dispatch share 5.082 % against a 10.000 % bar).
//
// These vectors prove the *wiring*. The primitives themselves are proved in
// `search::overlap_ics_meter::` against independent references, and the claim
// that the control arm is bit-identical to the pre-Wave-3 trajectory is a
// **cross-binary** measurement that no in-process test can make:
// `economics-round/integration/armgate.py` runs the round's base binary against
// this one on four fixed-work cells.

/// A calibrated plan for the tournament fixture: two phases, small enough that
/// the vector runs in a debug build, keyed to nothing real.
///
/// The rates are chosen, not measured, and the plan says so in `derivation`.
/// That is legitimate here and nowhere else: this is a test of the *pacer's*
/// arithmetic, and a plan that had to be measured first would make the vector a
/// test of the machine.
fn test_plan(explore_units: u64, compress_units: u64) -> super::icscal::WorkPlan {
    use super::icscal::{BinaryKey, CurrencyVersion, Executor, PhasePlan, PlanKey, PlanPhase};
    super::icscal::WorkPlan::new(
        PlanKey {
            request_sha256: "a".repeat(64),
            currency_version: CurrencyVersion::U0Samples,
            binary_key: BinaryKey {
                executable_sha256: "b".repeat(64),
                features: vec!["overlap-ics".to_owned()],
            },
            workers: 8,
            executor: Executor::EphemeralScope,
        },
        vec![
            PhasePlan::from_measurement(
                PlanPhase::Explore,
                explore_units,
                1.0,
                1.0,
                "a vector's chosen rate, not a measurement",
            )
            .expect("explore rate"),
            PhasePlan::from_measurement(
                PlanPhase::Compress,
                compress_units,
                1.0,
                1.0,
                "a vector's chosen rate, not a measurement",
            )
            .expect("compress rate"),
        ],
        "search::overlap_ics::tests",
    )
}

/// One calibrated run of the twelve-square fixture: the strip that is
/// infeasible by area, so `Φ` never reaches zero and the workers really are
/// compared.
fn calibrated_run(
    explore_units: u64,
    compress_units: u64,
    strikes: StrikeConfig,
) -> super::ScheduleOutcome {
    use crate::search::overlap_ics_meter::currency::Currency;
    use crate::search::overlap_ics_meter::pacer::{NoClock, WorkPlanPacer};

    let fixture = Fixture::squares(12, 20.0);
    let pieces = fixture.pieces();
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = (0..pieces.len())
        .map(|index| Pose {
            tx_mm: 20.0 + (index % 4) as f64 * 22.0,
            ty_mm: 20.0 + (index / 4) as f64 * 22.0,
            theta_deg: 0.0,
            mirrored: false,
        })
        .collect::<Vec<_>>();
    let config = IcsConfig {
        target_depth_mm: 40.0,
        proposal_budget: 0,
        relocate_eval_budget: u64::MAX,
        checkpoint_every_sweeps: u64::MAX,
        descent: DescentConfig::derive(&contract, &sources, 4),
        limits: PublicationLimits::default(),
    };
    let incumbent = super::state::ExactIncumbent {
        placements: Vec::new(),
        raw_source_depth_mm: 40.0,
        from_constructor: true,
        placement_fingerprint: "the-constructor".to_owned(),
    };
    let mut engine = Engine::from_poses(
        &pieces, settings, sources, contract, poses, incumbent, config,
    );
    let plan = test_plan(explore_units, compress_units);
    let pacer = WorkPlanPacer::from_plan(&plan, &Currency::U0, 1.0, 0.8, NoClock)
        .expect("the vector's plan must be spendable");
    engine.run_cutclose(
        ScheduleConfig {
            workers: 2,
            strikes,
            record_fingerprints: true,
            ..ScheduleConfig::default()
        },
        super::Budget::CalibratedWork {
            plan: Box::new(pacer),
            attempts_per_bite: 0,
        },
    )
}

/// The work-denominated arm sized for a twelve-square fixture rather than for
/// the 179 shelf. **Not** the shipped treatment: `StrikeConfig::TREATMENT`
/// carries `1_630_000` / `815_000`, a quantum that could never fire here, and
/// that those are the frozen numbers is asserted separately and on every FAST
/// run by `strike_meter::frozen_literals_intact`.
const SMALL_WORK_ARM: StrikeConfig = StrikeConfig::WorkStrikes {
    explore_quantum: 400,
    compress_quantum: 200,
    explore_strikes: 3,
    compress_strikes: 5,
};

/// **Both arms are reachable from `ScheduleConfig`, and the selection reaches
/// the trajectory.**
///
/// The spec's funded change 1 is a paired comparison of two runs of the same
/// executor and pacer differing in strike semantics and nothing else. A field
/// that selected an arm without changing what the trajectory does would make
/// that comparison vacuous, so this drives one cell twice and requires the
/// strike accounting to be denominated differently in the two documents.
#[test]
fn both_strike_arms_are_reachable_and_the_arm_reaches_the_trajectory() {
    let control = calibrated_run(60_000, 15_000, StrikeConfig::CONTROL);
    let treatment = calibrated_run(60_000, 15_000, SMALL_WORK_ARM);
    assert_eq!(control.strike_arm.arm(), "control-iteration-strikes");
    assert_eq!(treatment.strike_arm.arm(), "treatment-work-strikes");

    // The control's patience is counted in batches, so what accumulates at a
    // strike is a batch count: small. The treatment's is counted in sample
    // evaluations: large. On a fixture where the treatment actually strikes,
    // the two documents cannot be confused for one another.
    let treatment_strikes: u32 = treatment.bites.iter().map(|row| row.strikes).sum();
    let treatment_accumulated: u64 = treatment.bites.iter().map(|r| r.strike_accumulated).sum();
    assert!(
        treatment_accumulated >= 400,
        "a 400-evaluation quantum must have run out at least once, and what accumulated \
         is sample evaluations rather than batches: {treatment_accumulated} \
         (strikes {treatment_strikes})"
    );
    // And the control, on the same cell, never reaches 200 no-improvement
    // batches inside its unit allocation - so its strike count is zero and the
    // arm is the only thing that produced the treatment's strikes.
    let control_strikes: u32 = control.bites.iter().map(|row| row.strikes).sum();
    let control_accumulated: u64 = control.bites.iter().map(|r| r.strike_accumulated).sum();
    assert_eq!(
        control_accumulated, 0,
        "the control's 200-batch patience cannot be spent inside this allocation \
         (strikes {control_strikes})"
    );
}

/// **Both arms carry both patience counters**, which is what makes the spec's
/// paired promotion comparison term by term rather than shape by shape.
#[test]
fn both_arms_carry_both_shadow_counters() {
    for (label, outcome) in [
        (
            "control",
            calibrated_run(60_000, 15_000, StrikeConfig::CONTROL),
        ),
        ("treatment", calibrated_run(60_000, 15_000, SMALL_WORK_ARM)),
    ] {
        for row in &outcome.bites {
            let shadow = row.strike_shadow;
            assert_eq!(
                shadow.substantial + shadow.marginal + shadow.none,
                shadow.batches,
                "{label}: the three classes must partition the turns: {shadow:?}"
            );
            assert!(
                shadow.batches >= row.master_iterations,
                "{label}: every tournament is preceded by a classification, and the entry \
                 turn is one more: {shadow:?} vs {} iterations",
                row.master_iterations
            );
            // The meter is charged the batch that produced the reading it is
            // classifying, so the last tournament of a separation is never
            // charged to it. It can therefore never exceed the bite's own
            // sample evaluations.
            assert!(
                shadow.charged_work <= row.profile.sample_evaluations,
                "{label}: the meter cannot be charged work the bite did not do: {} vs {}",
                shadow.charged_work,
                row.profile.sample_evaluations
            );
        }
    }
}

/// **The shipped arms are the ones the spec signed.**
#[test]
fn the_shipped_arms_are_the_ones_the_spec_signed() {
    assert_eq!(
        StrikeConfig::CONTROL,
        StrikeConfig::IterationStrikes {
            explore: SeparateLimits::EXPLORE,
            compress: SeparateLimits::COMPRESS,
        }
    );
    assert_eq!(
        StrikeConfig::TREATMENT,
        StrikeConfig::WorkStrikes {
            explore_quantum: EXPLORE_WORK_QUANTUM,
            compress_quantum: COMPRESS_WORK_QUANTUM,
            explore_strikes: 3,
            compress_strikes: 5,
        }
    );
    assert_eq!(EXPLORE_WORK_QUANTUM, 1_630_000);
    assert_eq!(COMPRESS_WORK_QUANTUM, 815_000);
    assert_eq!(
        StrikeConfig::TREATMENT.rule(Phase::Explore).patience,
        Patience::Work(1_630_000)
    );
    assert_eq!(
        StrikeConfig::TREATMENT.rule(Phase::Compress).patience,
        Patience::Work(815_000)
    );
}

/// **A calibrated trajectory reads no clock.**
///
/// `search_seconds` and `explore_seconds` are `Pacer::elapsed_s()`, and the
/// calibrated arm returns `None` from it for the same reason the fixed-work arm
/// does: there is no `Instant` in the arm to read. Every publication's
/// `wall_seconds` is `None` for the same reason, which is what makes the
/// two-process bit identity of a calibrated cell a proof rather than a
/// coincidence.
#[test]
fn a_calibrated_trajectory_has_no_clock() {
    let outcome = calibrated_run(60_000, 15_000, StrikeConfig::CONTROL);
    assert!(outcome.search_seconds.is_none(), "a plan does not tick");
    assert!(outcome.explore_seconds.is_none(), "nor at a phase boundary");
    for row in &outcome.publications {
        assert!(
            row.wall_seconds.is_none(),
            "a calibrated publication has no second to record: {row:?}"
        );
    }
}

/// **The charge is the delta, never the running total.**
///
/// The spec's ranked defect (1): *"persistent-slot leakage / double-debit
/// ('stable but false' work accounting - the worst class this round has)"*, and
/// Sol review 19 §5's pre-committed red/green: *"batch two's aggregate must
/// equal the sum of the eight batch-two deltas, not cumulative slot totals"*.
///
/// The persistent executor the defect was named for does not exist - its gate
/// said no - but the accounting it would have corrupted is now live, because a
/// calibrated plan is spent out of exactly these numbers. So the identity is
/// asserted against the trajectory's own work vector: what the plan charged,
/// plus the tail after the last barrier, is what the engine counted. A pacer
/// handed a cumulative reading would charge the whole trajectory on every batch
/// and this sum would exceed the work vector by orders of magnitude.
#[test]
fn a_calibrated_plan_charges_deltas_and_they_sum_to_the_trajectory() {
    let outcome = calibrated_run(60_000, 15_000, StrikeConfig::CONTROL);
    let ledger = outcome
        .calibrated
        .as_ref()
        .expect("a calibrated run must carry its ledger");
    assert!(
        ledger.charge_identity_holds,
        "charged + tail must equal the trajectory: {ledger:?}"
    );
    assert!(
        ledger.consumed_units_match_charged,
        "the pacer's units and the currency of what it was handed must agree: {ledger:?}"
    );
    assert_eq!(
        ledger.consumed_units, ledger.charged.sample_evaluations,
        "under U0 a unit IS a sample evaluation, so the two are the same number"
    );
    // The engine's own counters are the third party here: the ledger is built
    // by the pacer and this is read off `Trace`, so agreement is a fact about
    // the wiring rather than about one accumulator.
    assert_eq!(
        ledger.charged.sample_evaluations + ledger.uncharged_tail.sample_evaluations,
        outcome.trace.work.sample_evaluations,
        "the base unit, against the engine's own counter"
    );
    assert_eq!(
        ledger.charged.master_batches + ledger.uncharged_tail.master_batches,
        outcome.trace.sweeps,
        "every tournament is charged to exactly one batch"
    );
    assert_eq!(
        ledger.charged.repair_rows + ledger.uncharged_tail.repair_rows,
        outcome.trace.work.repair_rows
    );
    assert_eq!(
        ledger.charged.actual_publication_attempt_calls
            + ledger.uncharged_tail.actual_publication_attempt_calls,
        outcome.trace.work.exact_checkpoints
    );
    // Every `charge_batch` call is one master batch, so the pacer's own batch
    // counters and the currency's `master_batches` term are the same number
    // reached two ways.
    assert_eq!(
        ledger.explore_batches + ledger.compress_batches,
        ledger.charged.master_batches,
        "the pacer counted a different number of batches than it charged: {ledger:?}"
    );
    assert!(
        ledger.charged.master_batches > 0,
        "the vector must actually have spent something: {ledger:?}"
    );
}

/// **The plan is what stopped it, and it stopped at a boundary.**
///
/// Nothing here asserts an exact stopping point: batch costs vary, which is the
/// whole reason the currency exists. What it does assert is that the 80/20 was
/// spent in units at the plan's own rate, and that explore did not stop before
/// its allocation was gone.
#[test]
fn a_calibrated_phase_spends_its_allocation_and_stops_at_a_barrier() {
    let outcome = calibrated_run(60_000, 15_000, StrikeConfig::CONTROL);
    let ledger = outcome.calibrated.as_ref().expect("ledger");
    // 1.0 s of budget, 80/20: explore gets 0.8 s at 60,000 u/s and compress
    // gets **the remainder**, at 15,000 u/s.
    assert_eq!(ledger.explore_allocation, 48_000);
    // 2,999 and not 3,000, and that is the correct number rather than a
    // rounding wart. Compress takes `budget - explore`, which is what
    // `Pacer::Wall` does - explore ends at `total * ratio` and compress runs to
    // `total` - and `1.0 - 0.8` is `0.19999999999999996` in binary. The
    // alternative, `budget * (1.0 - ratio)`, is not closer to the truth: it is
    // the same error moved one operation earlier, and it would make the two
    // pacers disagree about where a phase ends. `floor` then declines to spend
    // a unit the rate did not promise.
    assert_eq!(ledger.compress_allocation, 2_999);
    assert_eq!(ledger.budget_seconds, 1.0);
    assert_eq!(ledger.explore_ratio, 0.8);
    assert!(
        ledger.explore_consumed >= ledger.explore_allocation,
        "explore ran until its units were spent: {ledger:?}"
    );
    // **Overshoot <= one batch**, the spec's clause, against the batch that
    // actually crossed rather than against an average of them.
    assert!(
        ledger.explore_crossing_batch_units > 0,
        "explore ended by spending its allocation, so a batch crossed it: {ledger:?}"
    );
    assert!(
        ledger.explore_consumed - ledger.explore_allocation
            <= ledger.explore_crossing_batch_units,
        "explore overspent by more than the batch that crossed: {ledger:?}"
    );
    assert!(
        outcome.explore_bites > 0 || !outcome.bites.is_empty(),
        "the trajectory must have taken bites: {ledger:?}"
    );
}

/// **Two calibrated runs of the same plan agree bit for bit.** The whole point
/// of denominating a budget in work: quality is deterministic, wall is a
/// distribution.
#[test]
fn two_calibrated_runs_of_the_same_plan_are_bit_identical() {
    let first = calibrated_run(60_000, 15_000, StrikeConfig::CONTROL);
    let second = calibrated_run(60_000, 15_000, StrikeConfig::CONTROL);
    assert_eq!(
        first.fingerprints.len(),
        second.fingerprints.len(),
        "a calibrated plan must stop at the same batch every time"
    );
    assert!(!first.fingerprints.is_empty(), "the cell must have run");
    for (a, b) in first.fingerprints.iter().zip(&second.fingerprints) {
        assert_eq!(a, b, "the master state diverged");
    }
    assert_eq!(first.depth_mm.to_bits(), second.depth_mm.to_bits());
    assert_eq!(first.final_raw_phi.to_bits(), second.final_raw_phi.to_bits());
    assert_eq!(first.calibrated, second.calibrated, "and the ledger too");
}

/// **The wall and fixed-work arms carry no ledger, and are unchanged.**
///
/// `None` is not "nothing was spent": it is "no plan was spending". A reduction
/// that read a zeroed ledger off a fixed-work cell would be reading a plan that
/// never existed.
#[test]
fn the_other_two_budgets_carry_no_calibrated_ledger() {
    let pieces_owner = two_squares();
    let pieces = pieces_owner.pieces();
    let mut engine = banded_deficit_engine(&pieces, 60.0);
    let run = engine.run_cutclose(
        ScheduleConfig {
            workers: 2,
            ..ScheduleConfig::default()
        },
        TWO_BITES,
    );
    assert!(run.calibrated.is_none());
    assert_eq!(run.strike_arm.arm(), "control-iteration-strikes");
}
