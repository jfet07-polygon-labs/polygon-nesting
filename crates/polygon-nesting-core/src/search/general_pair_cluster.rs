#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use super::*;

const MAX_PAIR_ORIENTATION_TUPLES: usize = 2_048;
const MAX_TEMPLATE_CONTACT_ATTEMPTS: usize = 131_072;
const MAX_INTERNAL_PAIR_ROWS: usize = 8_192;
const MAX_RETAINED_TEMPLATES: usize = 128;
const MAX_TRANSFORMED_SOURCE_VERTICES: usize = 1_048_576;
const MAX_OFFSET_OUTPUT_VERTICES: usize = 1_048_576;
const MAX_INTERSECTION_INPUT_VERTICES: usize = 8_388_608;
const MAX_INTERSECTION_OUTPUT_VERTICES: usize = 1_048_576;
const MAX_TEMPLATES_PER_PAIR: usize = 4;

type PairFamilyKey = (Vec<i64>, bool, bool);
type PairTemplateKey = (i64, bool, i64, i64, i64, bool, i64, i64);

#[derive(Clone)]
#[allow(dead_code)]
pub(super) struct PairTemplate {
    pub first_input_index: usize,
    pub second_input_index: usize,
    pub first_rotation_deg: f64,
    pub first_mirrored: bool,
    pub first_translate_x: f64,
    pub first_translate_y: f64,
    pub second_rotation_deg: f64,
    pub second_mirrored: bool,
    pub second_translate_x: f64,
    pub second_translate_y: f64,
    pub first_collision: PolygonSet,
    pub second_collision: PolygonSet,
    pub long_axis_span_mm: f64,
    pub short_axis_span_mm: f64,
    pub envelope_waste_mm2: f64,
    key: PairTemplateKey,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(super) struct StablePairTemplates {
    pub first_input_index: usize,
    pub second_input_index: usize,
    pub templates: Vec<PairTemplate>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(super) struct PairTemplateCatalog {
    pub pairs: Vec<StablePairTemplates>,
    pub diagnostics: GeneralPairTemplateDiagnostics,
}

#[derive(Clone, Copy, Debug, Default)]
struct PairTemplateSlice {
    orientation_tuples: usize,
    contact_attempts: usize,
    exact_pair_rows: usize,
    retained_templates: usize,
    transformed_source_vertices: usize,
    offset_output_vertices: usize,
    intersection_input_vertices: usize,
    intersection_output_vertices: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct PairTemplateUsage {
    orientation_tuples: usize,
    contact_attempts: usize,
    exact_pair_rows: usize,
    transformed_source_vertices: usize,
    offset_output_vertices: usize,
    intersection_input_vertices: usize,
    intersection_output_vertices: usize,
    transient_rejected_output_vertices: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PairBuildOutcome {
    Complete,
    Exhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PairContactKey {
    kind: CandidateKind,
    fixed_feature_ordinal: usize,
    moving_feature_ordinal: usize,
    translate_x_grid: i64,
    translate_y_grid: i64,
}

#[derive(Clone, Copy, Debug)]
struct PairContact {
    translate_x: f64,
    translate_y: f64,
}

#[derive(Clone)]
struct PairOrientationTuple {
    first_rotation_deg: f64,
    first_mirrored: bool,
    second_rotation_deg: f64,
    second_mirrored: bool,
}

pub(super) fn build_pair_template_catalog(
    prepared: &[PreparedGeneralPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<PairTemplateCatalog, GeneralFastError> {
    let stable_pairs = eligible_stable_pairs(prepared);
    let mut diagnostics = GeneralPairTemplateDiagnostics {
        eligible_pairs: stable_pairs.len(),
        ..GeneralPairTemplateDiagnostics::default()
    };
    let mut pairs = Vec::with_capacity(stable_pairs.len());
    for (ordinal, (first_index, second_index)) in stable_pairs.iter().copied().enumerate() {
        let limits = pair_template_slice(ordinal, stable_pairs.len());
        let mut usage = PairTemplateUsage::default();
        let (outcome, templates) = build_stable_pair_templates(
            &prepared[first_index],
            &prepared[second_index],
            settings,
            limits,
            &mut usage,
        )?;
        accumulate_usage(&mut diagnostics, usage);
        if outcome == PairBuildOutcome::Exhausted {
            diagnostics.fallback_pairs += 1;
            pairs.push(StablePairTemplates {
                first_input_index: prepared[first_index].input_index,
                second_input_index: prepared[second_index].input_index,
                templates: Vec::new(),
            });
            continue;
        }
        diagnostics.retained_templates += templates.len();
        diagnostics.pairs_with_templates += usize::from(!templates.is_empty());
        pairs.push(StablePairTemplates {
            first_input_index: prepared[first_index].input_index,
            second_input_index: prepared[second_index].input_index,
            templates,
        });
    }
    Ok(PairTemplateCatalog { pairs, diagnostics })
}

fn eligible_stable_pairs(prepared: &[PreparedGeneralPiece<'_>]) -> Vec<(usize, usize)> {
    let mut families = BTreeMap::<PairFamilyKey, Vec<usize>>::new();
    for (index, piece) in prepared.iter().enumerate() {
        families
            .entry((
                piece.shape_family_key.clone(),
                piece.input.allow_rotation,
                piece.input.allow_mirror,
            ))
            .or_default()
            .push(index);
    }
    let mut pairs = Vec::new();
    for members in families.values_mut() {
        members.sort_by(|first, second| prepared[*first].input.id.cmp(prepared[*second].input.id));
        for pair in members.chunks_exact(2) {
            pairs.push((pair[0], pair[1]));
        }
    }
    pairs.sort_by(|(first_a, second_a), (first_b, second_b)| {
        let family_a = (
            &prepared[*first_a].shape_family_key,
            prepared[*first_a].input.allow_rotation,
            prepared[*first_a].input.allow_mirror,
        );
        let family_b = (
            &prepared[*first_b].shape_family_key,
            prepared[*first_b].input.allow_rotation,
            prepared[*first_b].input.allow_mirror,
        );
        family_a
            .cmp(&family_b)
            .then_with(|| prepared[*first_a].input.id.cmp(prepared[*first_b].input.id))
            .then_with(|| {
                prepared[*second_a]
                    .input
                    .id
                    .cmp(prepared[*second_b].input.id)
            })
    });
    pairs
}

fn pair_template_slice(ordinal: usize, pair_count: usize) -> PairTemplateSlice {
    let split = |total| deterministic_slice(total, ordinal, pair_count);
    PairTemplateSlice {
        orientation_tuples: split(MAX_PAIR_ORIENTATION_TUPLES),
        contact_attempts: split(MAX_TEMPLATE_CONTACT_ATTEMPTS),
        exact_pair_rows: split(MAX_INTERNAL_PAIR_ROWS),
        retained_templates: split(MAX_RETAINED_TEMPLATES).min(MAX_TEMPLATES_PER_PAIR),
        transformed_source_vertices: split(MAX_TRANSFORMED_SOURCE_VERTICES),
        offset_output_vertices: split(MAX_OFFSET_OUTPUT_VERTICES),
        intersection_input_vertices: split(MAX_INTERSECTION_INPUT_VERTICES),
        intersection_output_vertices: split(MAX_INTERSECTION_OUTPUT_VERTICES),
    }
}

fn deterministic_slice(total: usize, ordinal: usize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    total / count + usize::from(ordinal < total % count)
}

fn build_stable_pair_templates(
    first: &PreparedGeneralPiece<'_>,
    second: &PreparedGeneralPiece<'_>,
    settings: GeneralFastSettings,
    limits: PairTemplateSlice,
    usage: &mut PairTemplateUsage,
) -> Result<(PairBuildOutcome, Vec<PairTemplate>), GeneralFastError> {
    if limits.retained_templates == 0 {
        return Ok((PairBuildOutcome::Exhausted, Vec::new()));
    }
    let mut first_oriented = BTreeMap::<(i64, bool), PolygonSet>::new();
    let mut tuple_rows = Vec::<((f64, bool), Vec<(f64, bool)>)>::new();
    for (first_rotation_deg, first_mirrored) in
        angle_candidates(first, &[], settings, AngleScope::Full)
    {
        let Some(oriented) = charged_oriented_collision(
            first,
            first_rotation_deg,
            first_mirrored,
            settings,
            limits,
            usage,
        )?
        else {
            return Ok((PairBuildOutcome::Exhausted, Vec::new()));
        };
        let first_local = anchor_at_origin(&oriented)?;
        let first_bounds = first_local
            .bounds()
            .expect("pair orientation geometry is non-empty");
        let first_state = PlacedState {
            input_index: first.input_index,
            placement: GeneralFastPlacement {
                piece_id: first.input.id.to_owned(),
                rotation_deg: first_rotation_deg,
                mirrored: first_mirrored,
                translate_short_axis: -first_bounds.min_x,
                translate_long_axis: -first_bounds.min_y,
            },
            collision: first_local,
        };
        let second_orientations = angle_candidates(
            second,
            std::slice::from_ref(&first_state),
            settings,
            AngleScope::Full,
        );
        first_oriented.insert((angle_key(first_rotation_deg), first_mirrored), oriented);
        tuple_rows.push(((first_rotation_deg, first_mirrored), second_orientations));
    }
    let tuples = round_robin_orientation_tuples(tuple_rows, limits.orientation_tuples);
    usage.orientation_tuples = tuples.len();
    if tuples.is_empty() {
        return Ok((PairBuildOutcome::Complete, Vec::new()));
    }

    let mut second_oriented = BTreeMap::<(i64, bool), PolygonSet>::new();
    let mut candidates = BTreeMap::<PairTemplateKey, PairTemplate>::new();
    let tuple_count = tuples.len();
    for (tuple_ordinal, tuple) in tuples.into_iter().enumerate() {
        let first_collision = first_oriented
            .get(&(angle_key(tuple.first_rotation_deg), tuple.first_mirrored))
            .expect("pair tuples retain first orientations");
        let first_bounds = first_collision
            .bounds()
            .expect("pair orientation geometry is non-empty");
        let first_anchor_x = -first_bounds.min_x;
        let first_anchor_y = -first_bounds.min_y;
        let first_local = first_collision
            .translated(first_anchor_x, first_anchor_y)
            .map_err(GeneralFastError::Geometry)?;
        let second_key = (angle_key(tuple.second_rotation_deg), tuple.second_mirrored);
        if !second_oriented.contains_key(&second_key) {
            let Some(oriented) = charged_oriented_collision(
                second,
                tuple.second_rotation_deg,
                tuple.second_mirrored,
                settings,
                limits,
                usage,
            )?
            else {
                return Ok((PairBuildOutcome::Exhausted, Vec::new()));
            };
            second_oriented.insert(second_key, oriented);
        }
        let moving = second_oriented
            .get(&second_key)
            .expect("pair tuples retain second orientations");
        let contacts = pair_contacts(
            &first_local,
            moving,
            deterministic_slice(limits.contact_attempts, tuple_ordinal, tuple_count),
            usage,
        );
        let row_budget = deterministic_slice(limits.exact_pair_rows, tuple_ordinal, tuple_count);
        for contact in contacts.into_iter().take(row_budget) {
            let moving_local = moving
                .translated(contact.translate_x, contact.translate_y)
                .map_err(GeneralFastError::Geometry)?;
            let input_vertices = first_local
                .vertex_count()
                .saturating_add(moving_local.vertex_count());
            if usage
                .intersection_input_vertices
                .saturating_add(input_vertices)
                > limits.intersection_input_vertices
            {
                return Ok((PairBuildOutcome::Exhausted, Vec::new()));
            }
            usage.exact_pair_rows += 1;
            let complexity = first_local
                .intersection_area_with_complexity(&moving_local)
                .map_err(GeneralFastError::Geometry)?;
            usage.intersection_input_vertices = usage
                .intersection_input_vertices
                .saturating_add(complexity.input_vertices);
            if usage
                .intersection_output_vertices
                .saturating_add(complexity.output_vertices)
                > limits.intersection_output_vertices
            {
                usage.transient_rejected_output_vertices = usage
                    .transient_rejected_output_vertices
                    .saturating_add(complexity.output_vertices);
                return Ok((PairBuildOutcome::Exhausted, Vec::new()));
            }
            usage.intersection_output_vertices = usage
                .intersection_output_vertices
                .saturating_add(complexity.output_vertices);
            if complexity.area_mm2 > 0.0 {
                continue;
            }
            let template = canonical_template(
                first,
                second,
                &tuple,
                first_local.clone(),
                moving_local,
                first_anchor_x,
                first_anchor_y,
                contact,
            )?;
            candidates.entry(template.key).or_insert(template);
        }
    }
    Ok((
        PairBuildOutcome::Complete,
        retain_diverse_templates(
            candidates.into_values().collect(),
            limits.retained_templates,
        ),
    ))
}

fn charged_oriented_collision(
    piece: &PreparedGeneralPiece<'_>,
    rotation_deg: f64,
    mirrored: bool,
    settings: GeneralFastSettings,
    limits: PairTemplateSlice,
    usage: &mut PairTemplateUsage,
) -> Result<Option<PolygonSet>, GeneralFastError> {
    let source_vertices = piece.input.polygon.vertex_count();
    if usage
        .transformed_source_vertices
        .saturating_add(source_vertices)
        > limits.transformed_source_vertices
    {
        return Ok(None);
    }
    let collision = oriented_collision(piece, rotation_deg, mirrored, settings)
        .map_err(GeneralFastError::Geometry)?;
    let output_vertices = collision.vertex_count();
    if usage.offset_output_vertices.saturating_add(output_vertices) > limits.offset_output_vertices
    {
        usage.transient_rejected_output_vertices = usage
            .transient_rejected_output_vertices
            .saturating_add(output_vertices);
        return Ok(None);
    }
    usage.transformed_source_vertices = usage
        .transformed_source_vertices
        .saturating_add(source_vertices);
    usage.offset_output_vertices = usage.offset_output_vertices.saturating_add(output_vertices);
    Ok(Some(collision))
}

fn round_robin_orientation_tuples(
    rows: Vec<((f64, bool), Vec<(f64, bool)>)>,
    limit: usize,
) -> Vec<PairOrientationTuple> {
    let mut tuples = Vec::with_capacity(limit);
    let max_row = rows
        .iter()
        .map(|(_, second)| second.len())
        .max()
        .unwrap_or(0);
    for second_ordinal in 0..max_row {
        for ((first_rotation_deg, first_mirrored), second) in &rows {
            let Some((second_rotation_deg, second_mirrored)) = second.get(second_ordinal).copied()
            else {
                continue;
            };
            tuples.push(PairOrientationTuple {
                first_rotation_deg: *first_rotation_deg,
                first_mirrored: *first_mirrored,
                second_rotation_deg,
                second_mirrored,
            });
            if tuples.len() == limit {
                return tuples;
            }
        }
    }
    tuples
}

fn pair_contacts(
    fixed: &PolygonSet,
    moving: &PolygonSet,
    attempt_budget: usize,
    usage: &mut PairTemplateUsage,
) -> Vec<PairContact> {
    let fixed_points = contour_points(fixed);
    let fixed_edges = contour_edges(fixed);
    let moving_points = contour_points(moving);
    let moving_edges = contour_edges(moving);
    let mut contacts = BTreeMap::<PairContactKey, PairContact>::new();
    let mut seen_translations = BTreeSet::<(i64, i64)>::new();
    let attempt_limit = usage.contact_attempts.saturating_add(attempt_budget);
    let mut push =
        |kind, fixed_feature_ordinal, moving_feature_ordinal, translate_x, translate_y| -> bool {
            if usage.contact_attempts >= attempt_limit {
                return false;
            }
            usage.contact_attempts += 1;
            let (Some(translate_x_grid), Some(translate_y_grid)) =
                (grid_key(translate_x), grid_key(translate_y))
            else {
                return true;
            };
            if !seen_translations.insert((translate_x_grid, translate_y_grid)) {
                return true;
            }
            let key = PairContactKey {
                kind,
                fixed_feature_ordinal,
                moving_feature_ordinal,
                translate_x_grid,
                translate_y_grid,
            };
            contacts.insert(
                key,
                PairContact {
                    translate_x: from_grid(translate_x_grid as f64),
                    translate_y: from_grid(translate_y_grid as f64),
                },
            );
            true
        };

    for (fixed_ordinal, fixed_point) in fixed_points.iter().enumerate() {
        for (moving_ordinal, moving_point) in moving_points.iter().enumerate() {
            if !push(
                CandidateKind::VertexVertex,
                fixed_ordinal,
                moving_ordinal,
                fixed_point.x - moving_point.x,
                fixed_point.y - moving_point.y,
            ) {
                return contacts.into_values().collect();
            }
        }
    }
    for (fixed_ordinal, (fixed_start, fixed_end)) in fixed_edges.iter().enumerate() {
        for (moving_ordinal, moving_point) in moving_points.iter().enumerate() {
            let target = closest_point(*moving_point, *fixed_start, *fixed_end);
            if !push(
                CandidateKind::MovingVertexFixedEdge,
                fixed_ordinal,
                moving_ordinal,
                target.x - moving_point.x,
                target.y - moving_point.y,
            ) {
                return contacts.into_values().collect();
            }
        }
    }
    for (fixed_ordinal, fixed_point) in fixed_points.iter().enumerate() {
        for (moving_ordinal, (moving_start, moving_end)) in moving_edges.iter().enumerate() {
            let projected = closest_point(*fixed_point, *moving_start, *moving_end);
            if !push(
                CandidateKind::FixedVertexMovingEdge,
                fixed_ordinal,
                moving_ordinal,
                fixed_point.x - projected.x,
                fixed_point.y - projected.y,
            ) {
                return contacts.into_values().collect();
            }
        }
    }
    contacts.into_values().collect()
}

#[allow(clippy::too_many_arguments)]
fn canonical_template(
    first: &PreparedGeneralPiece<'_>,
    second: &PreparedGeneralPiece<'_>,
    tuple: &PairOrientationTuple,
    first_collision: PolygonSet,
    second_collision: PolygonSet,
    first_anchor_x: f64,
    first_anchor_y: f64,
    contact: PairContact,
) -> Result<PairTemplate, GeneralFastError> {
    let first_bounds = first_collision
        .bounds()
        .expect("pair template geometry is non-empty");
    let second_bounds = second_collision
        .bounds()
        .expect("pair template geometry is non-empty");
    let shift_x = -first_bounds.min_x.min(second_bounds.min_x);
    let shift_y = -first_bounds.min_y.min(second_bounds.min_y);
    let first_collision = first_collision
        .translated(shift_x, shift_y)
        .map_err(GeneralFastError::Geometry)?;
    let second_collision = second_collision
        .translated(shift_x, shift_y)
        .map_err(GeneralFastError::Geometry)?;
    let first_translate_x = first_anchor_x + shift_x;
    let first_translate_y = first_anchor_y + shift_y;
    let second_translate_x = contact.translate_x + shift_x;
    let second_translate_y = contact.translate_y + shift_y;
    let first_bounds = first_collision
        .bounds()
        .expect("pair template geometry is non-empty");
    let second_bounds = second_collision
        .bounds()
        .expect("pair template geometry is non-empty");
    let min_x = first_bounds.min_x.min(second_bounds.min_x);
    let min_y = first_bounds.min_y.min(second_bounds.min_y);
    let max_x = first_bounds.max_x.max(second_bounds.max_x);
    let max_y = first_bounds.max_y.max(second_bounds.max_y);
    let key = (
        angle_key(tuple.first_rotation_deg),
        tuple.first_mirrored,
        grid_key(first_translate_x).expect("template translations use the contractual grid"),
        grid_key(first_translate_y).expect("template translations use the contractual grid"),
        angle_key(tuple.second_rotation_deg),
        tuple.second_mirrored,
        grid_key(second_translate_x).expect("template translations use the contractual grid"),
        grid_key(second_translate_y).expect("template translations use the contractual grid"),
    );
    let envelope_area = (max_x - min_x) * (max_y - min_y);
    Ok(PairTemplate {
        first_input_index: first.input_index,
        second_input_index: second.input_index,
        first_rotation_deg: tuple.first_rotation_deg,
        first_mirrored: tuple.first_mirrored,
        first_translate_x,
        first_translate_y,
        second_rotation_deg: tuple.second_rotation_deg,
        second_mirrored: tuple.second_mirrored,
        second_translate_x,
        second_translate_y,
        envelope_waste_mm2: envelope_area
            - first_collision.area_mm2()
            - second_collision.area_mm2(),
        long_axis_span_mm: max_y - min_y,
        short_axis_span_mm: max_x - min_x,
        first_collision,
        second_collision,
        key,
    })
}

fn retain_diverse_templates(candidates: Vec<PairTemplate>, limit: usize) -> Vec<PairTemplate> {
    if candidates.len() <= limit {
        return candidates;
    }
    let mut selected = Vec::<PairTemplate>::with_capacity(limit);
    let mut selected_keys = BTreeSet::<PairTemplateKey>::new();

    let mut by_long_axis = candidates.clone();
    by_long_axis.sort_by(|first, second| {
        first
            .long_axis_span_mm
            .total_cmp(&second.long_axis_span_mm)
            .then_with(|| first.key.cmp(&second.key))
    });
    select_first_template(by_long_axis, &mut selected, &mut selected_keys);
    if selected.len() == limit {
        return selected;
    }
    let mut by_waste = candidates.clone();
    by_waste.sort_by(|first, second| {
        first
            .envelope_waste_mm2
            .total_cmp(&second.envelope_waste_mm2)
            .then_with(|| first.key.cmp(&second.key))
    });
    select_first_template(by_waste, &mut selected, &mut selected_keys);
    if selected.len() == limit {
        return selected;
    }
    let mut by_short_axis = candidates.clone();
    by_short_axis.sort_by(|first, second| {
        first
            .short_axis_span_mm
            .total_cmp(&second.short_axis_span_mm)
            .then_with(|| first.key.cmp(&second.key))
    });
    select_first_template(by_short_axis, &mut selected, &mut selected_keys);
    if selected.len() == limit {
        return selected;
    }
    let anchor = selected[0].key;
    let mut by_diversity = candidates;
    by_diversity.sort_by(|first, second| {
        template_diversity_key(second, anchor)
            .cmp(&template_diversity_key(first, anchor))
            .then_with(|| first.key.cmp(&second.key))
    });
    select_first_template(by_diversity, &mut selected, &mut selected_keys);
    selected
}

fn select_first_template(
    mut rows: Vec<PairTemplate>,
    selected: &mut Vec<PairTemplate>,
    selected_keys: &mut BTreeSet<PairTemplateKey>,
) {
    rows.retain(|row| !selected_keys.contains(&row.key));
    if let Some(row) = rows.into_iter().next() {
        selected_keys.insert(row.key);
        selected.push(row);
    }
}

fn template_diversity_key(template: &PairTemplate, anchor: PairTemplateKey) -> (bool, i64, i64) {
    let full_turn = angle_key(360.0 - 1.0 / ANGLE_KEY_SCALE) + 1;
    let circular_distance = |first: i64, second: i64| {
        let delta = (first - second).unsigned_abs() as i64;
        delta.min(full_turn - delta)
    };
    (
        template.first_mirrored != anchor.1 || template.second_mirrored != anchor.5,
        circular_distance(angle_key(template.first_rotation_deg), anchor.0),
        circular_distance(angle_key(template.second_rotation_deg), anchor.4),
    )
}

fn anchor_at_origin(polygon: &PolygonSet) -> Result<PolygonSet, GeneralFastError> {
    let bounds = polygon
        .bounds()
        .ok_or_else(|| GeneralPolygonError::from_message("cannot anchor empty pair geometry"))?;
    polygon
        .translated(-bounds.min_x, -bounds.min_y)
        .map_err(GeneralFastError::Geometry)
}

fn accumulate_usage(diagnostics: &mut GeneralPairTemplateDiagnostics, usage: PairTemplateUsage) {
    diagnostics.orientation_tuples += usage.orientation_tuples;
    diagnostics.contact_attempts += usage.contact_attempts;
    diagnostics.exact_pair_rows += usage.exact_pair_rows;
    diagnostics.transformed_source_vertices += usage.transformed_source_vertices;
    diagnostics.offset_output_vertices += usage.offset_output_vertices;
    diagnostics.intersection_input_vertices += usage.intersection_input_vertices;
    diagnostics.intersection_output_vertices += usage.intersection_output_vertices;
    diagnostics.transient_rejected_output_vertices += usage.transient_rejected_output_vertices;
}

const CONSTRUCTION_BEAM_WIDTH: usize = 8;
const MAX_UNIT_PROPOSALS_PER_BAND: usize = 32_768;
const MAX_CONTACT_ATTEMPTS_PER_BAND: usize = 262_144;
const MAX_EXACT_CHILD_FIXED_VISITS_PER_BAND: usize = 1_000_000;
const MAX_EXACT_CANDIDATE_ROWS_PER_BAND: usize = 32_768;
const PAIR_TRANSLATION_PROPOSALS_PER_CHILD: usize = 8;
const PAIR_FEASIBLE_ROWS_PER_TEMPLATE: usize = 8;
const SINGLETON_EXACT_ROWS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacroArm {
    UnbondedControl,
    RigidTreatment,
}

#[derive(Clone, Copy, Debug)]
enum ConstructionUnit {
    Pair(usize),
    Singleton(usize),
}

#[derive(Clone, Copy, Debug, Default)]
struct ConstructionLedger {
    proposal_attempts: usize,
    generated_proposals: usize,
    exact_child_fixed_visits: usize,
    exact_candidate_rows: usize,
    rigid_sheet_rejections: usize,
    rigid_overlap_rejections: usize,
    rigid_successors: usize,
}

impl ConstructionLedger {
    fn charge_proposals(&mut self, proposals: usize, attempts: usize) -> bool {
        if self.generated_proposals.saturating_add(proposals) > MAX_UNIT_PROPOSALS_PER_BAND
            || self.proposal_attempts.saturating_add(attempts) > MAX_CONTACT_ATTEMPTS_PER_BAND
        {
            return false;
        }
        self.generated_proposals += proposals;
        self.proposal_attempts += attempts;
        true
    }

    fn charge_exact_row(&mut self, child_fixed_visits: usize) -> bool {
        if self.exact_candidate_rows >= MAX_EXACT_CANDIDATE_ROWS_PER_BAND
            || self
                .exact_child_fixed_visits
                .saturating_add(child_fixed_visits)
                > MAX_EXACT_CHILD_FIXED_VISITS_PER_BAND
        {
            return false;
        }
        self.exact_candidate_rows += 1;
        self.exact_child_fixed_visits += child_fixed_visits;
        true
    }
}

pub(super) fn run_pair_cluster_experiment(
    prepared: &[PreparedGeneralPiece<'_>],
    _pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<GeneralPairClusterExperiment, GeneralFastError> {
    let catalog = build_pair_template_catalog(prepared, settings)?;
    Ok(GeneralPairClusterExperiment {
        templates: catalog.diagnostics,
        control: GeneralPairClusterArmDiagnostics {
            band_failures: vec![
                "mandatory macros, greedy pair seeds, and four-state pair shadows are closed"
                    .to_owned(),
            ],
            ..GeneralPairClusterArmDiagnostics::default()
        },
        treatment: GeneralPairClusterArmDiagnostics::default(),
    })
}

fn construction_units(
    prepared: &[PreparedGeneralPiece<'_>],
    catalog: &PairTemplateCatalog,
) -> Vec<ConstructionUnit> {
    let mut pair_by_first = BTreeMap::<usize, usize>::new();
    let mut paired_seconds = BTreeSet::<usize>::new();
    for (pair_index, pair) in catalog.pairs.iter().enumerate() {
        pair_by_first.insert(pair.first_input_index, pair_index);
        paired_seconds.insert(pair.second_input_index);
    }
    let mut units = Vec::new();
    for piece in shape_family_order(prepared) {
        if let Some(pair_index) = pair_by_first.get(&piece.input_index).copied() {
            units.push(ConstructionUnit::Pair(pair_index));
        } else if !paired_seconds.contains(&piece.input_index) {
            units.push(ConstructionUnit::Singleton(piece.input_index));
        }
    }
    units
}

fn run_macro_arm_portfolio(
    units: &[ConstructionUnit],
    catalog: &PairTemplateCatalog,
    prepared: &[PreparedGeneralPiece<'_>],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    arm: MacroArm,
) -> GeneralPairClusterArmDiagnostics {
    let mut diagnostics = GeneralPairClusterArmDiagnostics::default();
    let mut best = None::<GeneralFastResult>;
    for (band_label, factor) in [
        ("1.05x", Some(1.05)),
        ("1.25x", Some(1.25)),
        ("1.50x", Some(1.5)),
        ("2.00x", Some(2.0)),
        ("full-sheet", None),
    ] {
        diagnostics.band_variants_attempted += 1;
        let mut band_settings = settings;
        if let Some(factor) = factor {
            band_settings.sheet_long_axis_mm = fast_band_depth(prepared, settings, factor);
        }
        let (candidate, ledger) =
            match run_macro_beam(units, catalog, prepared, pieces, band_settings, arm) {
                Ok(result) => result,
                Err(error) => {
                    diagnostics.band_failures.push(format!(
                        "arm={arm:?} band={band_label} band_depth_mm={:.6}: {error}",
                        band_settings.sheet_long_axis_mm
                    ));
                    continue;
                }
            };
        diagnostics.completed_bands += 1;
        diagnostics.proposal_attempts += ledger.proposal_attempts;
        diagnostics.generated_proposals += ledger.generated_proposals;
        diagnostics.exact_child_fixed_visits += ledger.exact_child_fixed_visits;
        diagnostics.exact_candidate_rows += ledger.exact_candidate_rows;
        if best
            .as_ref()
            .is_none_or(|incumbent| compare_result_quality(&candidate, incumbent) == Ordering::Less)
        {
            best = Some(candidate);
        }
    }
    diagnostics.result = best;
    diagnostics
}

fn run_macro_beam(
    units: &[ConstructionUnit],
    catalog: &PairTemplateCatalog,
    prepared: &[PreparedGeneralPiece<'_>],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    arm: MacroArm,
) -> Result<(GeneralFastResult, ConstructionLedger), GeneralFastError> {
    let mut ledger = ConstructionLedger::default();
    let mut beam = vec![PartialLayout {
        placed: Vec::new(),
        unplaced_piece_ids: Vec::new(),
    }];
    for (unit_ordinal, unit) in units.iter().enumerate() {
        let mut successors = Vec::new();
        for state in &beam {
            let mut rows = match *unit {
                ConstructionUnit::Singleton(input_index) => {
                    singleton_successors(state, &prepared[input_index], settings, &mut ledger)?
                }
                ConstructionUnit::Pair(pair_index) => match arm {
                    MacroArm::UnbondedControl => control_pair_successors(
                        state,
                        &catalog.pairs[pair_index],
                        prepared,
                        settings,
                        &mut ledger,
                    )?,
                    MacroArm::RigidTreatment => treatment_pair_successors(
                        state,
                        &catalog.pairs[pair_index],
                        prepared,
                        settings,
                        &mut ledger,
                    )?,
                },
            };
            successors.append(&mut rows);
        }
        if successors.is_empty() {
            let unit_label = match *unit {
                ConstructionUnit::Singleton(input_index) => {
                    format!("singleton:{}", prepared[input_index].input.id)
                }
                ConstructionUnit::Pair(pair_index) => {
                    let pair = &catalog.pairs[pair_index];
                    format!(
                        "pair:{}+{} templates={}",
                        prepared[pair.first_input_index].input.id,
                        prepared[pair.second_input_index].input.id,
                        pair.templates.len()
                    )
                }
            };
            return Err(GeneralFastError::InvalidInput(format!(
                "the pair macro-step produced no complete successor at unit {unit_ordinal}/{} ({unit_label}, arm={arm:?}); ledger={ledger:?}",
                units.len()
            )));
        }
        let mut seen = BTreeSet::new();
        successors.retain(|state| seen.insert(partial_layout_state_key(state)));
        successors.sort_by(|first, second| {
            compare_pair_partial_layouts(first, second, catalog, settings)
        });
        successors.truncate(CONSTRUCTION_BEAM_WIDTH);
        beam = successors;
    }

    let mut best = None::<GeneralFastResult>;
    for state in beam {
        if state.placed.len() != pieces.len()
            || validate_result(pieces, &state.placed, settings).is_err()
        {
            continue;
        }
        let candidate = result_from_partial_layout(state, settings, ledger.exact_candidate_rows);
        if best
            .as_ref()
            .is_none_or(|incumbent| compare_result_quality(&candidate, incumbent) == Ordering::Less)
        {
            best = Some(candidate);
        }
    }
    best.map(|result| (result, ledger)).ok_or_else(|| {
        GeneralFastError::InvalidInput(
            "the pair macro-step produced no independently valid complete result".to_owned(),
        )
    })
}

fn singleton_successors(
    state: &PartialLayout,
    piece: &PreparedGeneralPiece<'_>,
    settings: GeneralFastSettings,
    ledger: &mut ConstructionLedger,
) -> Result<Vec<PartialLayout>, GeneralFastError> {
    let orientations = angle_candidates(piece, &state.placed, settings, AngleScope::Full);
    let search = best_candidate_for_orientations(
        piece,
        &state.placed,
        settings,
        &orientations,
        SINGLETON_EXACT_ROWS,
        4,
    )?;
    for _ in 0..search.exact_evaluations {
        if !ledger.charge_exact_row(state.placed.len()) {
            return Ok(Vec::new());
        }
    }
    Ok(search
        .candidates
        .into_iter()
        .map(|candidate| append_candidate(state, piece, candidate))
        .collect())
}

fn control_pair_successors(
    state: &PartialLayout,
    pair: &StablePairTemplates,
    prepared: &[PreparedGeneralPiece<'_>],
    settings: GeneralFastSettings,
    ledger: &mut ConstructionLedger,
) -> Result<Vec<PartialLayout>, GeneralFastError> {
    let first = &prepared[pair.first_input_index];
    let second = &prepared[pair.second_input_index];
    control_pair_without_templates(state, first, second, settings, ledger)
}

fn control_pair_without_templates(
    state: &PartialLayout,
    first: &PreparedGeneralPiece<'_>,
    second: &PreparedGeneralPiece<'_>,
    settings: GeneralFastSettings,
    ledger: &mut ConstructionLedger,
) -> Result<Vec<PartialLayout>, GeneralFastError> {
    let first_rows = singleton_successors(state, first, settings, ledger)?;
    let mut successors = Vec::new();
    for first_state in first_rows {
        successors.extend(singleton_successors(
            &first_state,
            second,
            settings,
            ledger,
        )?);
    }
    Ok(successors)
}

fn treatment_pair_successors(
    state: &PartialLayout,
    pair: &StablePairTemplates,
    prepared: &[PreparedGeneralPiece<'_>],
    settings: GeneralFastSettings,
    ledger: &mut ConstructionLedger,
) -> Result<Vec<PartialLayout>, GeneralFastError> {
    let mut successors = control_pair_without_templates(
        state,
        &prepared[pair.first_input_index],
        &prepared[pair.second_input_index],
        settings,
        ledger,
    )?;
    successors.extend(template_pair_successors(
        state, pair, prepared, settings, ledger,
    )?);
    Ok(successors)
}

fn template_pair_successors(
    state: &PartialLayout,
    pair: &StablePairTemplates,
    prepared: &[PreparedGeneralPiece<'_>],
    settings: GeneralFastSettings,
    ledger: &mut ConstructionLedger,
) -> Result<Vec<PartialLayout>, GeneralFastError> {
    let mut successors = Vec::new();
    for template in &pair.templates {
        let mut translations = BTreeMap::<(i64, i64), CandidateScore>::new();
        let first_bounds = template
            .first_collision
            .bounds()
            .expect("pair template geometry is non-empty");
        let second_bounds = template
            .second_collision
            .bounds()
            .expect("pair template geometry is non-empty");
        let group_bounds = crate::domain::IrregularBounds::new(
            first_bounds.min_x.min(second_bounds.min_x),
            first_bounds.min_y.min(second_bounds.min_y),
            first_bounds.max_x.max(second_bounds.max_x),
            first_bounds.max_y.max(second_bounds.max_y),
        );
        let sheet_inset = collision_sheet_inset_mm(settings);
        let current_depth = combined_bounds(&state.placed)
            .map(|bounds| bounds.max_y)
            .unwrap_or(sheet_inset);
        let group_supports = [
            (
                sheet_inset - group_bounds.min_x,
                sheet_inset - group_bounds.min_y,
            ),
            (
                settings.sheet_short_axis_mm - sheet_inset - group_bounds.max_x,
                sheet_inset - group_bounds.min_y,
            ),
            (
                sheet_inset - group_bounds.min_x,
                current_depth - group_bounds.min_y,
            ),
            (
                settings.sheet_short_axis_mm - sheet_inset - group_bounds.max_x,
                current_depth - group_bounds.min_y,
            ),
        ];
        let mut generated_group_supports = 0usize;
        for (translate_x, translate_y) in group_supports {
            let Some(key) = grid_key(translate_x).zip(grid_key(translate_y)) else {
                continue;
            };
            if translations.contains_key(&key) {
                continue;
            }
            let canonical_x = from_grid(key.0 as f64);
            let canonical_y = from_grid(key.1 as f64);
            translations.insert(
                key,
                score_pair_translation(state, template, canonical_x, canonical_y, settings),
            );
            generated_group_supports += 1;
        }
        if !ledger.charge_proposals(generated_group_supports, group_supports.len()) {
            return Ok(Vec::new());
        }
        for child in [&template.first_collision, &template.second_collision] {
            let (proposals, attempts) = translation_proposals(
                0.0,
                false,
                TranslationProposalInput {
                    oriented: child,
                    placed: &state.placed,
                    settings,
                    max_proposals: PAIR_TRANSLATION_PROPOSALS_PER_CHILD,
                    max_attempts: PAIR_TRANSLATION_PROPOSALS_PER_CHILD
                        .saturating_mul(PROPOSAL_BUDGET_MULTIPLIER),
                    fixed_piece_order_strategy: FixedPieceOrder::ShortSideFrontier,
                    contact_coverage: ContactCoverage::Fair,
                },
            )?;
            if !ledger.charge_proposals(proposals.len(), attempts) {
                return Ok(Vec::new());
            }
            for proposal in proposals {
                let key = (
                    grid_key(proposal.translate_x)
                        .expect("group proposals use the contractual grid"),
                    grid_key(proposal.translate_y)
                        .expect("group proposals use the contractual grid"),
                );
                translations.entry(key).or_insert_with(|| {
                    score_pair_translation(
                        state,
                        template,
                        proposal.translate_x,
                        proposal.translate_y,
                        settings,
                    )
                });
            }
        }
        translations.retain(|key, _| {
            let translate_x = from_grid(key.0 as f64);
            let translate_y = from_grid(key.1 as f64);
            group_bounds.min_x + translate_x >= sheet_inset
                && group_bounds.min_y + translate_y >= sheet_inset
                && group_bounds.max_x + translate_x <= settings.sheet_short_axis_mm - sheet_inset
                && group_bounds.max_y + translate_y <= settings.sheet_long_axis_mm - sheet_inset
        });
        let mut translations = translations.into_iter().collect::<Vec<_>>();
        translations.sort_by(|(first_key, first_score), (second_key, second_score)| {
            compare_group_scores(*first_score, *first_key, *second_score, *second_key)
        });
        let mut feasible_rows = 0usize;
        for ((translate_x_grid, translate_y_grid), _) in translations {
            let visits = state.placed.len().saturating_mul(2);
            if !ledger.charge_exact_row(visits) {
                return Ok(Vec::new());
            }
            let translate_x = from_grid(translate_x_grid as f64);
            let translate_y = from_grid(translate_y_grid as f64);
            let first_collision = template
                .first_collision
                .translated(translate_x, translate_y)
                .map_err(GeneralFastError::Geometry)?;
            let second_collision = template
                .second_collision
                .translated(translate_x, translate_y)
                .map_err(GeneralFastError::Geometry)?;
            if !collision_fits_sheet(&first_collision, settings)
                || !collision_fits_sheet(&second_collision, settings)
            {
                ledger.rigid_sheet_rejections += 1;
                continue;
            }
            let mut feasible = true;
            for fixed in &state.placed {
                if polygons_overlap_exact(&first_collision, &fixed.collision)?
                    || polygons_overlap_exact(&second_collision, &fixed.collision)?
                {
                    feasible = false;
                    break;
                }
            }
            if !feasible {
                ledger.rigid_overlap_rejections += 1;
                continue;
            }
            let mut successor = state.clone();
            successor.placed.push(PlacedState {
                input_index: template.first_input_index,
                placement: GeneralFastPlacement {
                    piece_id: prepared[template.first_input_index].input.id.to_owned(),
                    rotation_deg: template.first_rotation_deg,
                    mirrored: template.first_mirrored,
                    translate_short_axis: template.first_translate_x + translate_x,
                    translate_long_axis: template.first_translate_y + translate_y,
                },
                collision: first_collision,
            });
            successor.placed.push(PlacedState {
                input_index: template.second_input_index,
                placement: GeneralFastPlacement {
                    piece_id: prepared[template.second_input_index].input.id.to_owned(),
                    rotation_deg: template.second_rotation_deg,
                    mirrored: template.second_mirrored,
                    translate_short_axis: template.second_translate_x + translate_x,
                    translate_long_axis: template.second_translate_y + translate_y,
                },
                collision: second_collision,
            });
            ledger.rigid_successors += 1;
            successors.push(successor);
            feasible_rows += 1;
            if feasible_rows == PAIR_FEASIBLE_ROWS_PER_TEMPLATE {
                break;
            }
        }
    }
    Ok(successors)
}

fn append_candidate(
    state: &PartialLayout,
    piece: &PreparedGeneralPiece<'_>,
    candidate: Candidate,
) -> PartialLayout {
    let mut successor = state.clone();
    successor.placed.push(PlacedState {
        input_index: piece.input_index,
        placement: GeneralFastPlacement {
            piece_id: piece.input.id.to_owned(),
            rotation_deg: candidate.rotation_deg,
            mirrored: candidate.mirrored,
            translate_short_axis: candidate.translate_x,
            translate_long_axis: candidate.translate_y,
        },
        collision: candidate.collision,
    });
    successor
}

fn score_pair_translation(
    state: &PartialLayout,
    template: &PairTemplate,
    translate_x: f64,
    translate_y: f64,
    settings: GeneralFastSettings,
) -> CandidateScore {
    let first = template
        .first_collision
        .bounds()
        .expect("pair template geometry is non-empty");
    let second = template
        .second_collision
        .bounds()
        .expect("pair template geometry is non-empty");
    score_bounds(
        &state.placed,
        crate::domain::IrregularBounds::new(
            first.min_x.min(second.min_x) + translate_x,
            first.min_y.min(second.min_y) + translate_y,
            first.max_x.max(second.max_x) + translate_x,
            first.max_y.max(second.max_y) + translate_y,
        ),
        settings,
    )
}

fn compare_group_scores(
    first: CandidateScore,
    first_key: (i64, i64),
    second: CandidateScore,
    second_key: (i64, i64),
) -> Ordering {
    first
        .candidate_long_axis_position
        .total_cmp(&second.candidate_long_axis_position)
        .then_with(|| {
            first
                .candidate_short_axis_position
                .total_cmp(&second.candidate_short_axis_position)
        })
        .then_with(|| first.long_axis_depth.total_cmp(&second.long_axis_depth))
        .then_with(|| {
            first
                .unused_short_axis_projection
                .total_cmp(&second.unused_short_axis_projection)
        })
        .then_with(|| first.envelope_area.total_cmp(&second.envelope_area))
        .then_with(|| first_key.cmp(&second_key))
}

fn compare_pair_partial_layouts(
    first: &PartialLayout,
    second: &PartialLayout,
    catalog: &PairTemplateCatalog,
    settings: GeneralFastSettings,
) -> Ordering {
    let first_metrics = layout_metrics(&first.placed, settings);
    let second_metrics = layout_metrics(&second.placed, settings);
    let first_frontier = layout_frontier_metrics(&first.placed, settings);
    let second_frontier = layout_frontier_metrics(&second.placed, settings);
    second
        .placed
        .len()
        .cmp(&first.placed.len())
        .then_with(|| {
            first_metrics
                .used_long_axis_depth_mm
                .total_cmp(&second_metrics.used_long_axis_depth_mm)
        })
        .then_with(|| {
            pair_envelope_waste(first, catalog).total_cmp(&pair_envelope_waste(second, catalog))
        })
        .then_with(|| {
            first_frontier
                .void_area_mm2
                .total_cmp(&second_frontier.void_area_mm2)
        })
        .then_with(|| {
            first_frontier
                .roughness_mm
                .total_cmp(&second_frontier.roughness_mm)
        })
        .then_with(|| {
            first_metrics
                .unused_short_axis_projection_mm
                .total_cmp(&second_metrics.unused_short_axis_projection_mm)
        })
        .then_with(|| {
            canonical_placed_key(&first.placed).cmp(&canonical_placed_key(&second.placed))
        })
}

fn pair_envelope_waste(state: &PartialLayout, catalog: &PairTemplateCatalog) -> f64 {
    let by_input_index = state
        .placed
        .iter()
        .map(|placed| (placed.input_index, placed))
        .collect::<BTreeMap<_, _>>();
    catalog
        .pairs
        .iter()
        .filter_map(|pair| {
            let first = by_input_index.get(&pair.first_input_index)?;
            let second = by_input_index.get(&pair.second_input_index)?;
            let first_bounds = first.collision.bounds()?;
            let second_bounds = second.collision.bounds()?;
            let width = first_bounds.max_x.max(second_bounds.max_x)
                - first_bounds.min_x.min(second_bounds.min_x);
            let height = first_bounds.max_y.max(second_bounds.max_y)
                - first_bounds.min_y.min(second_bounds.min_y);
            Some(width * height - first.collision.area_mm2() - second.collision.area_mm2())
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> IrregularPoint {
        IrregularPoint::new(x, y)
    }

    fn triangle(offset: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(offset, offset),
            point(10.0 + offset, offset),
            point(5.0 + offset, 8.0 + offset),
        ])
        .unwrap()
    }

    #[test]
    fn deterministic_slices_exhaust_the_global_ceiling_without_transfer() {
        for total in [0, 1, 2, 7, 128, 2_048] {
            for count in 1..9 {
                let slices = (0..count)
                    .map(|ordinal| deterministic_slice(total, ordinal, count))
                    .collect::<Vec<_>>();
                assert_eq!(slices.iter().sum::<usize>(), total);
                assert!(slices.windows(2).all(|pair| pair[0] >= pair[1]));
                assert!(slices.iter().max().unwrap() - slices.iter().min().unwrap() <= 1);
            }
        }
    }

    #[test]
    fn capability_mismatches_never_form_a_stable_pair() {
        let polygon = triangle(0.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygon,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygon,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "c",
                polygon: &polygon,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "d",
                polygon: &polygon,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let prepared = prepare_general_pieces(
            &pieces,
            GeneralFastSettings::deterministic_test(100.0, 100.0),
        )
        .unwrap();

        assert_eq!(eligible_stable_pairs(&prepared), vec![(0, 1)]);
    }

    #[test]
    fn exact_pair_templates_are_normalized_and_non_overlapping() {
        let first = triangle(0.0);
        let second = triangle(0.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &first,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &second,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let prepared = prepare_general_pieces(&pieces, settings).unwrap();
        let catalog = build_pair_template_catalog(&prepared, settings).unwrap();

        assert_eq!(catalog.diagnostics.eligible_pairs, 1);
        assert_eq!(catalog.diagnostics.pairs_with_templates, 1);
        assert!(!catalog.pairs[0].templates.is_empty());
        for template in &catalog.pairs[0].templates {
            let first_bounds = template.first_collision.bounds().unwrap();
            let second_bounds = template.second_collision.bounds().unwrap();
            assert_eq!(first_bounds.min_x.min(second_bounds.min_x), 0.0);
            assert_eq!(first_bounds.min_y.min(second_bounds.min_y), 0.0);
            assert!(
                !polygons_overlap_exact(&template.first_collision, &template.second_collision)
                    .unwrap()
            );
        }
    }

    #[test]
    fn collision_equal_source_distinct_members_use_their_actual_geometry() {
        let first = triangle(0.0);
        let second = triangle(0.0004);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &first,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &second,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let prepared = prepare_general_pieces(&pieces, settings).unwrap();
        assert_eq!(prepared[0].shape_family_key, prepared[1].shape_family_key);
        let catalog = build_pair_template_catalog(&prepared, settings).unwrap();
        let template = &catalog.pairs[0].templates[0];
        let rebuilt_first = transformed_collision(
            &prepared[0],
            template.first_rotation_deg,
            template.first_mirrored,
            template.first_translate_x,
            template.first_translate_y,
            settings,
        )
        .unwrap();
        let rebuilt_second = transformed_collision(
            &prepared[1],
            template.second_rotation_deg,
            template.second_mirrored,
            template.second_translate_x,
            template.second_translate_y,
            settings,
        )
        .unwrap();

        assert_eq!(
            polygon_absolute_key(&rebuilt_first),
            polygon_absolute_key(&template.first_collision)
        );
        assert_eq!(
            polygon_absolute_key(&rebuilt_second),
            polygon_absolute_key(&template.second_collision)
        );
    }
}
