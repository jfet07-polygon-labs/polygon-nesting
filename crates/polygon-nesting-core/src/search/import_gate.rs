//! Gate A: the three verdicts on one imported pose set, and the slacks behind
//! them.
//!
//! This module answers Grok review 6 §2's gate. It takes a *complete pose set*
//! that some other packer produced and reports, on the same poses:
//!
//! * **(a) contract only** — [`validate_placements_against_contract`], the
//!   raw-source `f64` clearance validator at `total_padding + 2 * sag` pair and
//!   `sheet_edge_clearance + sag` boundary. No envelope, no canonical grid;
//! * **(b) composite miter** — [`validate_and_measure_placements`], the
//!   acceptance authority: every placement's canonical collision polygon, a
//!   **miter** offset by [`collision_expansion_mm`], must fit the inset sheet
//!   and be pairwise disjoint on the integer grid, *and then* (a) must hold;
//! * **(c) composite round** — the same construction with the miter join
//!   replaced by a round one at the same radius, which is what
//!   `P (+) disc(radius)` actually is. Shadow only.
//!
//! # What is a shadow and what is not
//!
//! (a) and (b) are the engine's own functions, called unmodified. (c) is built
//! here and reaches nothing: no search path, no scorer, no publication route
//! calls into this module, the whole file is compiled out without
//! `import-gate-shadow`, and the one geometry hook it needs
//! ([`PolygonSet::offset_with_join_shadow`]) is compiled out with it.
//! [`PolygonSet::offset`] is untouched, and [`census`] asserts at run time that
//! the shadow's miter configuration reproduces it byte for byte on every piece
//! it measures before it trusts any round number.
//!
//! # Why a boolean is not the deliverable
//!
//! "The composite rejects" is compatible with two completely different worlds,
//! and Grok's interpretation table turns on which one obtains:
//!
//! * the **join shape** rejects — two convex corners in diagonal opposition
//!   push their miter points together while the material is still clear, which
//!   is the failure the micro-legalization module documents and the one a round
//!   envelope would fix;
//! * the **radius** rejects — the envelope is offset by `total_padding / 2 +
//!   clearance_safety_margin + search_offset_allowance`, which is *strictly
//!   larger* than the contract's `total_padding / 2`, so a pair sitting between
//!   `total_padding` and `2 * expansion` of material clearance is refused by
//!   any join, round included.
//!
//! Those two demand opposite spends. So this module measures, per pair, a
//! quantity that separates them:
//!
//! ## The critical radius `r*`
//!
//! For a fixed pair of poses and a fixed join,
//!
//! ```text
//! r*(i, j) = max { r in integer micrometres : offset(P_i, r) and offset(P_j, r)
//!                  have zero intersection area }
//! ```
//!
//! Offsets are nested and increasing in `r`, so disjointness is monotone in `r`
//! and the maximum is found by bisection. `r*` is exact on the canonical grid -
//! the grid step *is* one micrometre, so there is no interpolation and no
//! tolerance to argue about.
//!
//! Two facts make it the right instrument:
//!
//! * for an exact disc join, `offset(P_i, r)` and `offset(P_j, r)` are disjoint
//!   iff the **material** distance `d(i, j) > 2r`, so `2 * r*` *is* `d(i, j)`,
//!   to the grid;
//! * for any join whose result contains the disc - miter and square both do -
//!   `2 * r* <= d(i, j)`, and the deficit
//!
//!   ```text
//!   join cost(i, j) = d(i, j) - 2 * r*(i, j)
//!   ```
//!
//!   is exactly the material clearance the representation spends on that pair
//!   and cannot give back.
//!
//! The composite accepts the pair iff `expansion <= r*`, so
//! `radius slack = r* - expansion` is the pair's margin in the envelope's own
//! units, and `2 * radius slack` is it in material-clearance millimetres. The
//! boundary test gets the same treatment: `b*` is the largest radius at which a
//! placement's envelope still fits the inset rectangle.
//!
//! # The round join's discretisation, and which way it errs
//!
//! Clipper's round join emits points **on** the circle, so the polygon it
//! builds is *inscribed* - a round envelope is a slight **under**-approximation
//! of `P (+) disc(r)`, and an under-approximating envelope can accept a layout
//! the true disc would refuse. The deviation is Clipper's `arc_tolerance`, and
//! its default is `radius / 500` - **5 µm** at the 2.5 mm collision radius,
//! five times the canonical grid step and six times the margin this gate is
//! measuring. That default would have decided the verdict by itself.
//!
//! So [`EnvelopeSpec::arc_tolerance_grid`] is set explicitly by the caller, in
//! grid units, and [`Census::round_inward_deviation_mm`] reports the bound that
//! results. Below one grid unit the round join and the true disc cannot be
//! distinguished by anything on this grid, which is the regime the evidence
//! runs in. Nothing here is used to publish; a promotion would need Sol review
//! 11's outward-only discretisation with the error inside the margin, and this
//! module deliberately does not pretend to be it.

use std::collections::BTreeMap;

use crate::clipper::offset::JoinType;
use crate::geometry::general_polygon::{GeneralPolygonError, PolygonSet};
use crate::search::general_fast::{
    collision_expansion_mm, collision_sheet_inset_mm, polygons_overlap_exact,
    validate_and_measure_placements, validate_placements_against_contract, GeneralFastPiece,
    GeneralFastPlacement, GeneralFastSettings,
};
use crate::validation::general_polygon::{
    material_pair_distance_mm, material_sheet_clearance_mm, raw_source_long_axis_depth_mm,
    GeneralPlacement,
};

/// One envelope configuration to measure the pose set against.
#[derive(Clone, Debug)]
pub struct EnvelopeSpec {
    /// How the evidence names this row.
    pub label: String,
    /// `Miter` is the production join; `Round` is `P (+) disc(radius)` to
    /// within `arc_tolerance_grid`; `Square` is Grok's "square containing the
    /// disc" alternative.
    pub join: JoinType,
    /// The offset radius in millimetres. The production value is
    /// [`collision_expansion_mm`]; passing `total_padding / 2` is the
    /// zero-allowance envelope, which is the radius Sol review 11 and Grok
    /// review 6 §A.1 both name.
    pub radius_mm: f64,
    /// Clipper's miter limit. The production value is 2.0 and is only read for
    /// `Miter`.
    pub miter_limit: f64,
    /// Clipper's arc tolerance in canonical grid units (1 unit = 0.001 mm),
    /// only read for `Round`. At or below 0.01 Clipper substitutes
    /// `radius / 500`, which is 5 µm here - see the module note.
    pub arc_tolerance_grid: f64,
}

impl EnvelopeSpec {
    /// The production envelope of `settings`: miter, at
    /// [`collision_expansion_mm`], at the production miter limit and arc
    /// tolerance.
    pub fn production(label: impl Into<String>, settings: GeneralFastSettings) -> Self {
        let (miter_limit, arc_tolerance_grid) = PolygonSet::production_offset_join_shadow();
        Self {
            label: label.into(),
            join: JoinType::Miter,
            radius_mm: collision_expansion_mm(settings),
            miter_limit,
            arc_tolerance_grid,
        }
    }

    /// The production envelope with the join swapped and the radius kept.
    pub fn with_join(mut self, label: impl Into<String>, join: JoinType) -> Self {
        self.label = label.into();
        self.join = join;
        self
    }

    /// The same envelope at another radius.
    pub fn at_radius(mut self, label: impl Into<String>, radius_mm: f64) -> Self {
        self.label = label.into();
        self.radius_mm = radius_mm;
        self
    }
}

/// One pair of placements, measured.
#[derive(Clone, Debug)]
pub struct PairRow {
    pub first_index: usize,
    pub second_index: usize,
    pub first_piece_id: String,
    pub second_piece_id: String,
    /// The contract validator's own material clearance for this pair.
    pub material_clearance_mm: f64,
    /// Whether the two envelopes intersect with positive area, by the same
    /// [`polygons_overlap_exact`] the composite asks.
    pub envelope_overlaps: bool,
    /// The intersection area, `0.0` when they do not overlap.
    pub envelope_intersection_area_mm2: f64,
    /// The critical radius, in millimetres, on the integer-micrometre grid.
    /// `None` when the pair was outside the bisection band.
    pub critical_radius_mm: Option<f64>,
    /// Whether the bisection saturated its ceiling, so `critical_radius_mm` is
    /// a floor and not the answer. Every derived number on this row is then a
    /// floor too, and none of them is quotable.
    pub critical_radius_saturated: bool,
    /// `critical_radius - radius`: positive is margin, negative is the amount
    /// the envelope radius would have to shrink for the pair to be legal.
    pub radius_slack_mm: Option<f64>,
    /// `2 * radius_slack`: the same margin in material-clearance millimetres.
    pub clearance_slack_mm: Option<f64>,
    /// `material_clearance - 2 * critical_radius`: the material clearance this
    /// join shape spends on this pair and cannot give back. Zero for a join
    /// that represents the disc exactly.
    pub join_cost_mm: Option<f64>,
}

/// One placement against the four sheet edges, measured.
#[derive(Clone, Debug)]
pub struct BoundaryRow {
    pub index: usize,
    pub piece_id: String,
    /// The contract validator's own `[short low, short high, long low, long
    /// high]` material clearances.
    pub material_clearance_mm: [f64; 4],
    /// Whether the envelope fits the inset rectangle, by the same
    /// `PolygonSet::fits_rect` the composite asks.
    pub envelope_fits: bool,
    /// How far past each inset edge the *snapped* envelope reaches; negative is
    /// inside. Order matches `material_clearance_mm`.
    pub envelope_excursion_mm: [f64; 4],
    /// The largest radius, on the integer-micrometre grid, at which this
    /// placement's envelope still fits the inset rectangle. `None` when the
    /// placement was outside the bisection band.
    pub critical_radius_mm: Option<f64>,
    pub radius_slack_mm: Option<f64>,
    /// Whether the bisection saturated its ceiling. True for any placement
    /// sitting far enough inside the sheet that its envelope fits at four
    /// times the contract radius - a row that carries no boundary information
    /// and whose slack must not be quoted or averaged.
    pub critical_radius_saturated: bool,
}

/// The whole measurement for one [`EnvelopeSpec`].
#[derive(Clone, Debug)]
pub struct Census {
    pub label: String,
    pub join: &'static str,
    pub radius_mm: f64,
    pub miter_limit: f64,
    pub arc_tolerance_grid: f64,
    /// The bound on how far *inside* the true `P (+) disc(radius)` this
    /// envelope's boundary can fall, in millimetres. `0.0` for joins that
    /// contain the disc (miter, square); Clipper's chord deviation for round.
    pub round_inward_deviation_mm: f64,
    /// The rectangle the envelope must fit, `sheet - inset` on both axes.
    pub sheet_inset_mm: f64,
    /// Whether the shadow rebuild's miter configuration reproduced
    /// [`PolygonSet::offset`] exactly on every piece. `None` for a
    /// non-production configuration, where there is nothing to reproduce.
    pub reproduces_production_offset: Option<bool>,
    /// The verdict this envelope configuration reaches, without the contract
    /// half: `true` when every placement fits and no pair overlaps.
    pub envelope_admissible: bool,
    pub boundary_failure_count: usize,
    pub pair_failure_count: usize,
    pub pair_count: usize,
    /// Every pair whose envelopes overlap, plus the tightest `bisect_top` pairs
    /// by material clearance, each bisected. Sorted by `radius_slack`
    /// ascending, so the worst offender is first.
    pub pairs: Vec<PairRow>,
    /// Every placement whose envelope does not fit, plus the tightest
    /// `bisect_top` by material clearance, each bisected.
    pub boundaries: Vec<BoundaryRow>,
    /// How many rows above carry a saturated `r*` - a floor rather than the
    /// answer. Reported so a reader can see the count without scanning.
    pub saturated_pair_rows: usize,
    pub saturated_boundary_rows: usize,
    pub envelope_vertex_total: usize,
}

/// The result of running the engine's own two validators.
#[derive(Clone, Debug)]
pub struct AuthorityVerdict {
    /// `validate_placements_against_contract` — verdict (a).
    pub contract_only: Result<(), String>,
    /// `validate_and_measure_placements` — verdict (b), HEAD's acceptance
    /// authority.
    pub composite: Result<CompositeMetrics, String>,
    pub expansion_mm: f64,
    pub sheet_inset_mm: f64,
    pub contract_pair_clearance_mm: f64,
    pub contract_sheet_clearance_mm: f64,
    /// The material's own depth in the engine's published convention,
    /// `max source y + sheet_edge_clearance`.
    pub raw_source_depth_mm: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct CompositeMetrics {
    pub used_short_axis_span_mm: f64,
    pub used_long_axis_depth_mm: f64,
}

fn join_name(join: JoinType) -> &'static str {
    match join {
        JoinType::Miter => "miter",
        JoinType::Square => "square",
        JoinType::Bevel => "bevel",
        JoinType::Round => "round",
    }
}

fn independent_placements<'a>(
    pieces: &'a [GeneralFastPiece<'a>],
    placements: &'a [GeneralFastPlacement],
) -> Result<Vec<GeneralPlacement<'a>>, String> {
    let by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    placements
        .iter()
        .map(|placement| {
            let piece = by_id
                .get(placement.piece_id.as_str())
                .copied()
                .ok_or_else(|| format!("unknown piece {}", placement.piece_id))?;
            Ok(GeneralPlacement {
                piece_id: piece.id,
                polygon: piece.polygon,
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_x: placement.translate_short_axis,
                translate_y: placement.translate_long_axis,
            })
        })
        .collect()
}

/// Runs verdicts (a) and (b): the engine's own functions, unmodified.
pub fn authority_verdict(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> Result<AuthorityVerdict, String> {
    let independent = independent_placements(pieces, placements)?;
    let edge_clearance = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    Ok(AuthorityVerdict {
        contract_only: validate_placements_against_contract(pieces, placements, settings)
            .map_err(|error| error.to_string()),
        composite: validate_and_measure_placements(pieces, placements, settings)
            .map(|metrics| CompositeMetrics {
                used_short_axis_span_mm: metrics.used_short_axis_span_mm,
                used_long_axis_depth_mm: metrics.used_long_axis_depth_mm,
            })
            .map_err(|error| error.to_string()),
        expansion_mm: collision_expansion_mm(settings),
        sheet_inset_mm: collision_sheet_inset_mm(settings),
        contract_pair_clearance_mm: settings.total_padding_mm
            + 2.0 * settings.flattening_sag_tolerance_mm,
        contract_sheet_clearance_mm: edge_clearance + settings.flattening_sag_tolerance_mm,
        raw_source_depth_mm: raw_source_long_axis_depth_mm(&independent, edge_clearance)
            .map_err(|error| error.to_string())?,
    })
}

/// The inward chord deviation Clipper's round join can produce at `radius_mm`,
/// in millimetres, given an arc tolerance in grid units.
///
/// Clipper picks `steps_per_360 = pi / acos(1 - arc_tol / radius)` in grid
/// units, which makes the half-step sagitta `radius * (1 - cos(pi /
/// steps_per_360))` exactly `arc_tol`. So the answer is `arc_tol` itself, in
/// millimetres - and Clipper's own default substitution when the caller passes
/// `<= 0.01` is `radius / 500`.
fn round_inward_deviation_mm(radius_mm: f64, arc_tolerance_grid: f64) -> f64 {
    if arc_tolerance_grid > 0.01 {
        arc_tolerance_grid / 1000.0
    } else {
        radius_mm / 500.0
    }
}

fn build(
    polygon: &PolygonSet,
    placement: &GeneralFastPlacement,
    spec: &EnvelopeSpec,
    radius_mm: f64,
) -> Result<PolygonSet, GeneralPolygonError> {
    polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_short_axis,
            placement.translate_long_axis,
        )?
        .offset_with_join_shadow(radius_mm, spec.join, spec.miter_limit, spec.arc_tolerance_grid)
}

/// One micrometre, in millimetres: the canonical grid step, and the resolution
/// every `r*` in this module is exact to.
const GRID_STEP_MM: f64 = 0.001;

/// The largest integer-micrometre radius at which `predicate` still holds,
/// searched in `[0, ceiling]`, and whether the search **saturated**.
///
/// `predicate` must be monotone: true at `0`, and once false it stays false.
/// Both uses here are - offsets are nested and increasing in the radius, so
/// both "these two are disjoint" and "this one fits the rectangle" can only go
/// from true to false as the radius grows.
///
/// `None` means the predicate already fails at zero radius: for the pair test
/// the *material* overlaps, for the boundary test the material is already
/// outside the inset rectangle.
///
/// `Some((ceiling, true))` means the predicate still held at the ceiling, so
/// the true critical radius is somewhere at or above it and the number returned
/// is a floor rather than the answer. That happens for a placement sitting in
/// the middle of the sheet, whose envelope fits at any radius this gate cares
/// about - a row with no boundary information in it. The flag exists so such a
/// row is *labelled* rather than quoted: a saturated `r*` would otherwise
/// contribute a fictitious multi-millimetre slack to any statistic taken over
/// these rows.
fn largest_micron_radius(
    ceiling_microns: i64,
    mut predicate: impl FnMut(f64) -> Result<bool, GeneralPolygonError>,
) -> Result<Option<(i64, bool)>, GeneralPolygonError> {
    if !predicate(0.0)? {
        return Ok(None);
    }
    let (mut low, mut high) = (0i64, ceiling_microns.max(1));
    if predicate(high as f64 * GRID_STEP_MM)? {
        return Ok(Some((high, true)));
    }
    // invariant: predicate(low) holds, predicate(high) does not.
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if predicate(middle as f64 * GRID_STEP_MM)? {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok(Some((low, false)))
}

/// Measures one envelope configuration against one pose set.
///
/// `bisect_top` is how many of the tightest pairs (and placements) by material
/// clearance get a critical radius; every *failing* pair and placement gets one
/// regardless of where it ranks, because those are the ones a verdict has to
/// name. Bisection is `O(log radius)` Clipper offsets per row, so this is the
/// only knob that decides the run time.
pub fn census(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    spec: &EnvelopeSpec,
    bisect_top: usize,
) -> Result<Census, String> {
    let by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    let resolved = placements
        .iter()
        .map(|placement| {
            by_id
                .get(placement.piece_id.as_str())
                .copied()
                .ok_or_else(|| format!("unknown piece {}", placement.piece_id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let independent = independent_placements(pieces, placements)?;

    let inset = collision_sheet_inset_mm(settings);
    let production = collision_expansion_mm(settings);
    let (production_miter_limit, production_arc_tolerance) =
        PolygonSet::production_offset_join_shadow();
    let is_production_configuration = spec.join == JoinType::Miter
        && spec.miter_limit == production_miter_limit
        && spec.arc_tolerance_grid == production_arc_tolerance
        && spec.radius_mm == production;

    let mut envelopes = Vec::with_capacity(placements.len());
    let mut reproduces = is_production_configuration.then_some(true);
    for (placement, piece) in placements.iter().zip(&resolved) {
        let envelope = build(piece.polygon, placement, spec, spec.radius_mm).map_err(|e| e.to_string())?;
        if is_production_configuration {
            let real = piece
                .polygon
                .transformed(
                    placement.rotation_deg,
                    placement.mirrored,
                    placement.translate_short_axis,
                    placement.translate_long_axis,
                )
                .and_then(|posed| posed.offset(production))
                .map_err(|e| e.to_string())?;
            if real != envelope {
                reproduces = Some(false);
            }
        }
        envelopes.push(envelope);
    }
    let envelope_vertex_total = envelopes.iter().map(PolygonSet::vertex_count).sum();

    // --- boundary ---
    let ceiling_microns = |value: f64| ((value * 4.0 / GRID_STEP_MM).ceil() as i64).max(1);
    let mut boundary_material = Vec::with_capacity(placements.len());
    for placement in &independent {
        boundary_material.push(
            material_sheet_clearance_mm(
                placement,
                settings.sheet_short_axis_mm,
                settings.sheet_long_axis_mm,
            )
            .map_err(|error| error.to_string())?,
        );
    }
    let fits = |envelope: &PolygonSet| {
        envelope.fits_rect(
            inset,
            inset,
            settings.sheet_short_axis_mm - inset,
            settings.sheet_long_axis_mm - inset,
        )
    };
    let mut boundary_order = (0..placements.len()).collect::<Vec<_>>();
    boundary_order.sort_by(|a, b| {
        let key = |index: usize| {
            boundary_material[index]
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min)
        };
        key(*a).total_cmp(&key(*b))
    });
    let mut boundary_selected = boundary_order
        .iter()
        .copied()
        .take(bisect_top)
        .collect::<Vec<_>>();
    let mut boundary_failure_count = 0usize;
    for index in 0..placements.len() {
        if !fits(&envelopes[index]) {
            boundary_failure_count += 1;
            if !boundary_selected.contains(&index) {
                boundary_selected.push(index);
            }
        }
    }
    let mut boundaries = Vec::with_capacity(boundary_selected.len());
    for index in boundary_selected {
        let envelope = &envelopes[index];
        let bounds = envelope.bounds();
        let excursion = bounds
            .map(|bounds| {
                [
                    inset - bounds.min_x,
                    bounds.max_x - (settings.sheet_short_axis_mm - inset),
                    inset - bounds.min_y,
                    bounds.max_y - (settings.sheet_long_axis_mm - inset),
                ]
            })
            .unwrap_or([f64::NAN; 4]);
        let critical = largest_micron_radius(
            ceiling_microns(spec.radius_mm),
            |radius| Ok(fits(&build(resolved[index].polygon, &placements[index], spec, radius)?)),
        )
        .map_err(|error| error.to_string())?;
        boundaries.push(BoundaryRow {
            index,
            piece_id: placements[index].piece_id.clone(),
            material_clearance_mm: boundary_material[index],
            envelope_fits: fits(envelope),
            envelope_excursion_mm: excursion,
            critical_radius_mm: critical.map(|(value, _)| value as f64 * GRID_STEP_MM),
            radius_slack_mm: critical
                .map(|(value, _)| value as f64 * GRID_STEP_MM - spec.radius_mm),
            critical_radius_saturated: critical.is_some_and(|(_, saturated)| saturated),
        });
    }
    boundaries.sort_by(|a, b| {
        a.radius_slack_mm
            .unwrap_or(f64::INFINITY)
            .total_cmp(&b.radius_slack_mm.unwrap_or(f64::INFINITY))
    });

    // --- pairs ---
    let mut material = Vec::new();
    for first in 0..placements.len() {
        for second in (first + 1)..placements.len() {
            material.push((
                material_pair_distance_mm(&independent[first], &independent[second])
                    .map_err(|error| error.to_string())?,
                first,
                second,
            ));
        }
    }
    let pair_count = material.len();
    let mut ranked = material.clone();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut selected = ranked
        .iter()
        .take(bisect_top)
        .map(|row| (row.1, row.2))
        .collect::<Vec<_>>();
    let mut pair_failure_count = 0usize;
    let mut overlap_area = BTreeMap::new();
    for &(_, first, second) in &material {
        if polygons_overlap_exact(&envelopes[first], &envelopes[second])
            .map_err(|error| error.to_string())?
        {
            pair_failure_count += 1;
            if !selected.contains(&(first, second)) {
                selected.push((first, second));
            }
            overlap_area.insert(
                (first, second),
                envelopes[first]
                    .intersection_area_mm2(&envelopes[second])
                    .map_err(|error| error.to_string())?,
            );
        }
    }
    let material_by_pair = material
        .iter()
        .map(|row| ((row.1, row.2), row.0))
        .collect::<BTreeMap<_, _>>();
    let mut pairs = Vec::with_capacity(selected.len());
    for (first, second) in selected {
        let clearance = material_by_pair[&(first, second)];
        let critical = largest_micron_radius(ceiling_microns(spec.radius_mm), |radius| {
            let a = build(resolved[first].polygon, &placements[first], spec, radius)?;
            let b = build(resolved[second].polygon, &placements[second], spec, radius)?;
            Ok(!polygons_overlap_exact(&a, &b)?)
        })
        .map_err(|error| error.to_string())?;
        let critical_mm = critical.map(|(value, _)| value as f64 * GRID_STEP_MM);
        pairs.push(PairRow {
            critical_radius_saturated: critical.is_some_and(|(_, saturated)| saturated),
            first_index: first,
            second_index: second,
            first_piece_id: placements[first].piece_id.clone(),
            second_piece_id: placements[second].piece_id.clone(),
            material_clearance_mm: clearance,
            envelope_overlaps: overlap_area.contains_key(&(first, second)),
            envelope_intersection_area_mm2: overlap_area
                .get(&(first, second))
                .copied()
                .unwrap_or(0.0),
            critical_radius_mm: critical_mm,
            radius_slack_mm: critical_mm.map(|value| value - spec.radius_mm),
            clearance_slack_mm: critical_mm.map(|value| 2.0 * (value - spec.radius_mm)),
            join_cost_mm: critical_mm.map(|value| clearance - 2.0 * value),
        });
    }
    pairs.sort_by(|a, b| {
        a.radius_slack_mm
            .unwrap_or(f64::INFINITY)
            .total_cmp(&b.radius_slack_mm.unwrap_or(f64::INFINITY))
    });

    Ok(Census {
        label: spec.label.clone(),
        join: join_name(spec.join),
        radius_mm: spec.radius_mm,
        miter_limit: spec.miter_limit,
        arc_tolerance_grid: spec.arc_tolerance_grid,
        round_inward_deviation_mm: if spec.join == JoinType::Round {
            round_inward_deviation_mm(spec.radius_mm, spec.arc_tolerance_grid)
        } else {
            0.0
        },
        sheet_inset_mm: inset,
        reproduces_production_offset: reproduces,
        envelope_admissible: boundary_failure_count == 0 && pair_failure_count == 0,
        boundary_failure_count,
        pair_failure_count,
        pair_count,
        saturated_pair_rows: pairs.iter().filter(|row| row.critical_radius_saturated).count(),
        saturated_boundary_rows: boundaries
            .iter()
            .filter(|row| row.critical_radius_saturated)
            .count(),
        pairs,
        boundaries,
        envelope_vertex_total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bisection is the only piece of arithmetic in this module that a
    /// reader has to trust rather than read off the geometry, and every number
    /// the round reports is derived from it. These pin its three outcomes -
    /// found, saturated, refused at zero - against an oracle whose answer is
    /// known by construction, so a future edit that reintroduces an off-by-one
    /// fails here rather than silently shifting a millimetre.
    #[test]
    fn largest_micron_radius_finds_the_last_true_step() {
        for threshold in [1i64, 2, 7, 2_500, 2_501, 9_999] {
            let found = largest_micron_radius(10_000, |radius| {
                Ok((radius / GRID_STEP_MM).round() as i64 <= threshold)
            })
            .expect("the oracle cannot fail");
            assert_eq!(
                found,
                Some((threshold, false)),
                "threshold {threshold} should be found exactly and unsaturated"
            );
        }
    }

    #[test]
    fn largest_micron_radius_flags_a_saturated_search() {
        // True everywhere: the ceiling is returned and the row is labelled, so
        // no statistic can quote it as an answer.
        let found = largest_micron_radius(10_000, |_| Ok(true)).expect("the oracle cannot fail");
        assert_eq!(found, Some((10_000, true)));
    }

    #[test]
    fn largest_micron_radius_reports_none_when_zero_already_fails() {
        // For a pair this is "the material itself overlaps"; for a boundary it
        // is "the material is already outside the inset rectangle". Neither has
        // a critical radius, and reporting 0 would claim one.
        let found = largest_micron_radius(10_000, |_| Ok(false)).expect("the oracle cannot fail");
        assert_eq!(found, None);
    }

    /// Clipper substitutes `radius / 500` for any arc tolerance at or below
    /// 0.01 grid units, which at the 2.5 mm collision radius is a 5 um inward
    /// chord deviation - five canonical grid steps, and larger than every
    /// margin this gate measures. The evidence's soundness budget is derived
    /// from this function, so the substitution rule has to be pinned rather
    /// than assumed.
    #[test]
    fn round_inward_deviation_follows_clippers_default_substitution() {
        assert_eq!(round_inward_deviation_mm(2.5, 0.0), 0.005);
        assert_eq!(round_inward_deviation_mm(2.5, 0.01), 0.005);
        assert_eq!(round_inward_deviation_mm(2.5, 0.1), 0.0001);
        assert_eq!(round_inward_deviation_mm(2.502, 100.0), 0.1);
    }

    /// A miter offset contains the disc, so the shadow must never report the
    /// production configuration as anything but miter at the production
    /// constants - the flag that arms the run-time equivalence assertion.
    #[test]
    fn production_spec_is_the_production_join_at_the_production_constants() {
        let mut settings = GeneralFastSettings::deterministic_test(2000.0, 2700.0);
        settings.total_padding_mm = 5.0;
        settings.sheet_edge_clearance_mm = Some(5.0);
        settings.clearance_safety_margin_mm = 0.0;
        settings.flattening_sag_tolerance_mm = 0.0;
        settings.search_offset_allowance_mm = 0.002;
        let spec = EnvelopeSpec::production("probe", settings);
        let (miter_limit, arc_tolerance) = PolygonSet::production_offset_join_shadow();
        assert_eq!(spec.join, JoinType::Miter);
        assert_eq!(spec.miter_limit, miter_limit);
        assert_eq!(spec.arc_tolerance_grid, arc_tolerance);
        assert_eq!(spec.radius_mm, collision_expansion_mm(settings));
        assert_eq!(spec.radius_mm, 2.502);
        assert_eq!(collision_sheet_inset_mm(settings), 2.5);
        // Swapping the join keeps the radius, which is the whole point: the
        // round census has to differ from the miter one in shape alone.
        let round = spec.clone().with_join("round", JoinType::Round);
        assert_eq!(round.radius_mm, spec.radius_mm);
        assert_eq!(round.join, JoinType::Round);
    }
}
