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

use super::contact::{convex_cell_gap, triangle_minkowski_signed_distance};
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
