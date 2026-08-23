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
/// not it improved anything - and the stall hook runs no second one.
#[test]
fn every_sweep_runs_exactly_one_weight_pass_and_the_stall_hook_runs_none() {
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

    let before = state.pair_rows[0].weight;
    let jump = descent.on_stalled_sweep(&mut state, &sources, &contract, &mut work);
    assert!(!jump.attempted, "there is no topology jump any more");
    assert_eq!(work.weight_updates, 2, "and no second weight dialect");
    assert_eq!(state.pair_rows[0].weight.to_bits(), before.to_bits());
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
