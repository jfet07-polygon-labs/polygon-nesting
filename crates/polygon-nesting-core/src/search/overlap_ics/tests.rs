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
fn guided_weights_change_the_guided_total_and_never_the_raw_one() {
    let fixture = Fixture::squares(6, 20.0);
    let (_, _, mut state) = state_of(&fixture, 300.0);
    let before = fold(&state);
    assert!(before.raw > 0.0, "the fixture must actually overlap");
    super::energy::guided_update(&mut state);
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

#[test]
fn the_ladder_runs_from_the_derived_top_to_a_quarter_micrometre() {
    let fixture = Fixture::squares(4, 20.0);
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let contract = Contract::from_settings(test_settings());
    let config = DescentConfig::derive(&contract, &sources, 0);
    let ladder = config.ladder();
    assert!((ladder[0] - 1.25).abs() < 1e-12, "top rung {}", ladder[0]);
    assert_eq!(*ladder.last().expect("a bottom rung"), 0.00025);
    assert!(ladder.len() > 8);
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
// The three vectors of the torque pivot.
//
// `gate0-rerun/README.md` §2.2 named a defect and §2.3 refused to repair it in
// the round that found it: `incident_gradient` takes its torque about the
// piece's transformed **centroid**, while the proposal composition rotated
// about the pose **origin** `(tx, ty)`. The two coincide only when the source
// ring's centroid sits at the source origin, and on this campaign's two
// fixtures it never does - the offset is 1.00 to 1.35 circumradii on every
// piece of both. §2.3 also said exactly what would settle it:
//
//     "What would settle it is a unit vector - 'a step of `s` along the SE(2)
//      direction lowers the incident guided energy for small `s`' - and that
//      vector cannot be committed in this round, because on this code it would
//      be RED."
//
// These are that vector and its two companions. The first two were run against
// the un-fixed tree first and are red there;
// `gate0-pivot-rerun/evidence/pivot-red.log` is the transcript.

/// One piece whose source ring is given explicitly, so a test can put the ring
/// far from its own pose origin - which is where both campaign fixtures put
/// theirs.
fn one_piece(ring: &[[f64; 2]]) -> Fixture {
    Fixture {
        polygons: vec![polygon(ring)],
        ids: vec!["piece-00".to_owned()],
    }
}

/// The same state builder as [`state_of`], at an explicit pose.
fn state_at(fixture: &Fixture, pose: Pose, target: f64) -> (Vec<PieceSource>, Contract, IcsState) {
    let settings = test_settings();
    let contract = Contract::from_settings(settings);
    let pieces = fixture.pieces();
    let sources = super::state::piece_sources(&pieces).expect("sources");
    let poses = sources.iter().map(|_| pose).collect::<Vec<_>>();
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

/// **The pivot vector.** A step along the SE(2) direction the gradient produced
/// must lower the incident guided energy that gradient was taken from.
///
/// The fixture is this campaign's geometry in miniature and every number in it
/// is asserted rather than assumed: one 20 mm square whose source ring sits
/// 141.421 mm from its pose origin (10 circumradii, against the fixtures'
/// 1.00-1.35), placed with exactly one active row - the bottom, at 2.000 mm.
///
/// At that state the gradient is `force = (0, 4)` and `torque = -40` **about
/// the transformed centroid**, so the SE(2)-normalized direction is
/// `(0, 0.816497, -0.0408248 rad/mm)`: a 0.577 rotational share against a
/// 0.816 translational one. The arithmetic of the two pivots is not close.
///
/// * about the **centroid**, the bottom-most material rises at 0.408 mm per mm
///   of step: 0.816 of lift from the translation, less 0.408 at the corner the
///   10 mm arm carries downward — which is the corner that *becomes* the lowest
///   one, so the min is taken there. The row closes and the step is accepted on
///   the first rung;
/// * about the **origin**, the 141.421 mm arm drags the same material *down* at
///   3.674 mm per mm - nine times faster, in the opposite direction - so the row
///   opens on every rung of the ladder, from 1.25 mm to 0.25 µm.
///
/// The second bullet is the C175 census signature exactly: `Δ(incident guided)`
/// positive and **linear in the step** all the way to the bottom rung, on a
/// direction that a correct steepest descent gives first-order coefficient
/// `−|∇|`. The failure message prints the rungs, so a red run of this test is
/// itself the measurement.
#[test]
fn a_ladder_step_descends_the_energy_its_own_gradient_was_taken_from() {
    let fixture = one_piece(&square(90.0, 90.0, 20.0));
    let (sources, contract, mut state) = state_at(
        &fixture,
        Pose { tx_mm: 0.0, ty_mm: -87.0, theta_deg: 0.0, mirrored: false },
        300.0,
    );

    // The fixture: a centroid ten circumradii from the pose origin, and one
    // active row.
    let offset = libm::hypot(sources[0].centroid[0], sources[0].centroid[1]);
    let radius = sources[0].max_radius_mm;
    assert!((offset - 141.42135623730951).abs() < 1e-9, "centroid offset {offset}");
    assert!((radius - 14.142135623730951).abs() < 1e-9, "circumradius {radius}");
    let census = super::energy::census(&state);
    assert_eq!(census.active_pairs, 0, "one piece has no pair rows");
    assert_eq!(
        census.active_edges_by_side, [0, 0, 1, 0],
        "exactly one active row, and it is the bottom one"
    );
    let violation = state.edge_rows[0][super::state::EDGE_BOTTOM].violation_mm;
    assert!((violation - 2.0).abs() < 1e-12, "bottom violation {violation}");

    // The gradient, before anything is armed or stepped.
    let gradient = super::energy::incident_gradient(&state, 0);
    assert!(gradient[0].abs() < 1e-12, "force_x {}", gradient[0]);
    assert!((gradient[1] - 4.0).abs() < 1e-12, "force_y {}", gradient[1]);
    assert!((gradient[2] + 40.0).abs() < 1e-12, "torque {}", gradient[2]);

    let config = DescentConfig::derive(&contract, &sources, 0);
    let mut descent = super::descent::Descent::new(config, vec![true]);
    let mut work = WorkVector::default();
    // Arm the rejection census through its own shipped path, so a refusal
    // records every rung it refused on. One guided update fires with it and
    // that is harmless here: it multiplies the single active row's weight by
    // exactly 2, and the SE(2) direction is invariant under a rescaling of the
    // whole gradient.
    descent.on_stalled_sweep(&mut state, &sources, &contract, &mut work);
    let before = super::energy::incident_guided(&state, 0);
    let accepted = descent.propose(&mut state, &sources, &contract, 0, &mut work);
    let after = super::energy::incident_guided(&state, 0);
    let rungs: Vec<(f64, f64)> = descent
        .rejection_census()
        .records
        .first()
        .map(|record| {
            record
                .rungs
                .iter()
                .map(|rung| (rung.step_mm, rung.delta_incident_guided))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        accepted,
        "the ladder refused every rung of the direction its own gradient \
         produced; (step_mm, delta_incident_guided) = {rungs:?}"
    );
    assert!(
        after < before,
        "incident guided energy {before} -> {after}; rungs {rungs:?}"
    );
}

/// **The pivot, measured kinematically.** A proposal's translational component
/// is the whole of what it moves the piece's centroid by; a rotation about the
/// centroid moves the centroid not at all.
///
/// This one uses a fixture the *un-fixed* composition still accepts, so what is
/// red is not "the step was refused" but the displacement itself: a 2x20 mm
/// bar, source centroid 78.102 mm from its pose origin, one active bottom row
/// at 2.000 mm, whose gradient direction is 0.995 translation and 0.099
/// rotation. On the accepted first rung of 1.25 mm the proposal's own
/// translational component is 1.2438575 mm - and rotating about the pose origin
/// lands the centroid **0.9618590 mm** away from there, 77.3 % of the entire
/// modelled translation, as an unmodelled rigid drift the gradient never
/// accounted for.
///
/// The tolerance is derived and not fitted: 1 nm is 1/250 of the ladder's
/// bottom rung and 1/1000 of the publication band's canonical grid, so it is
/// the largest displacement this engine has no vocabulary for. The residual it
/// is compared against is round-off: the composition is exact, so the identity
/// `c_after = c_before + dt` holds to all orders and not merely to first.
#[test]
fn a_proposal_moves_the_transformed_centroid_by_its_translation_alone() {
    // 1 nm: below the 0.25 µm bottom rung and below the 1 µm canonical grid.
    const ROUND_OFF_MM: f64 = 1e-6;

    let fixture = one_piece(&[[49.0, 50.0], [51.0, 50.0], [51.0, 70.0], [49.0, 70.0]]);
    let (sources, contract, mut state) = state_at(
        &fixture,
        Pose { tx_mm: 0.0, ty_mm: -47.0, theta_deg: 0.0, mirrored: false },
        300.0,
    );
    let offset = libm::hypot(sources[0].centroid[0], sources[0].centroid[1]);
    assert!((offset - 78.10249675906654).abs() < 1e-9, "centroid offset {offset}");
    assert_eq!(
        super::energy::census(&state).active_edges_by_side,
        [0, 0, 1, 0],
        "exactly one active row, and it is the bottom one"
    );

    // The direction the ladder will walk, recomputed here exactly as
    // `Descent::propose` derives it, so the test knows what the step claimed.
    let gradient = super::energy::incident_gradient(&state, 0);
    let radius = sources[0].max_radius_mm;
    let angular = gradient[2] / (radius * radius);
    let norm = libm::hypot(libm::hypot(gradient[0], gradient[1]), radius * angular);
    let direction = [gradient[0] / norm, gradient[1] / norm, angular / norm];
    // In closed form: the witness arm about the centroid is `(-1, -10)` against
    // an inward normal of `(0, 1)`, so `|f| = 2wv` and `tau = -2wv`, and with
    // `R^2 = 101` the translational share is
    // `|f| / hypot(|f|, tau/R) = sqrt(101/102)`.
    assert!(
        (libm::hypot(direction[0], direction[1]) - (101.0f64 / 102.0).sqrt()).abs() < 1e-9,
        "translational share {direction:?}"
    );

    let config = DescentConfig::derive(&contract, &sources, 0);
    let step = config.ladder()[0];
    assert!((step - 1.25).abs() < 1e-12, "top rung {step}");
    let centroid_before = state.geometry.centroids[0];
    let theta_before = state.poses[0].theta_deg;

    let mut descent = super::descent::Descent::new(config, vec![true]);
    let mut work = WorkVector::default();
    let accepted = descent.propose(&mut state, &sources, &contract, 0, &mut work);
    assert!(accepted, "this fixture is chosen so that both pivots accept");
    let turned_deg = state.poses[0].theta_deg - theta_before;
    assert!(
        (turned_deg - (step * direction[2]).to_degrees()).abs() < 1e-12,
        "the accepted rung is the ladder top: turned {turned_deg} deg"
    );

    let centroid_after = state.geometry.centroids[0];
    let translation = [step * direction[0], step * direction[1]];
    let drift = libm::hypot(
        centroid_after[0] - centroid_before[0] - translation[0],
        centroid_after[1] - centroid_before[1] - translation[1],
    );
    assert!(
        drift <= ROUND_OFF_MM,
        "the proposal translated by {translation:?} and the transformed \
         centroid moved from {centroid_before:?} to {centroid_after:?}: \
         {drift} mm of rigid drift the gradient never modelled, against a \
         modelled translation of {} mm",
        libm::hypot(translation[0], translation[1])
    );
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
