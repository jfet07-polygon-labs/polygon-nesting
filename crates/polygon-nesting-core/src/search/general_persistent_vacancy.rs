use super::*;

use std::mem::size_of;

const PERSISTENT_VACANCY_SEED_DOMAIN: u64 = 0x5650_4f50_3030_3031;
const TARGET_DEPTH_MM: f64 = 165.0;
const MIXED_PIECE_COUNT: usize = 61;
const EXPECTED_PARENT_FINGERPRINT: &str =
    "b9335a72cdcdd8df29be21450818f4ab1766ea1ea0b16765ad3998942a2ea6c5";
const EXPECTED_PARENT_DEPTH_MM: f64 = 168.361;
const MAX_LAYERS: usize = 40;
const BEAM_WIDTH: usize = 8;
const SELECTED_PIECES_PER_PARENT: usize = 2;
const ORIENTATIONS_PER_PIECE: usize = 12;
const POSITIONS_PER_ORIENTATION: usize = 32;
const FINALISTS_PER_PIECE: usize = 8;
const MACRO_CONTROL_MODE: usize = 8;
const MACRO_TREATMENT_MODE: usize = 9;
const PRESERVED_BEST_MACRO_MODE: usize = 10;
const REPAIR_CONTROL_MODE: usize = 14;
const REPAIR_TREATMENT_MODE: usize = 15;
const REPAIR_HORIZON: usize = 16;
const REPAIR_BEAM_WIDTH: usize = 4;
const REPAIR_PARENT_EXPANSIONS: usize = 1 + (REPAIR_HORIZON - 1) * REPAIR_BEAM_WIDTH;
const REPAIR_SEED_DOMAIN: u64 = 0x5650_5245_5041_4952;
const MAX_INACTIVE_PIECES: usize = 32;
const MAX_SOURCE_FEATURES: usize = 512;
const MAX_COLLISION_VERTICES: usize = 512;
const MAX_PARENT_EXPANSIONS: usize = MAX_LAYERS * (BEAM_WIDTH + 1);
const MAX_SELECTED_PIECE_SLOTS: usize = MAX_PARENT_EXPANSIONS * SELECTED_PIECES_PER_PARENT;
const MAX_ORIENTATION_STREAMS: usize = MAX_SELECTED_PIECE_SLOTS * ORIENTATIONS_PER_PIECE;
const MAX_SOURCE_FEATURE_VISITS: usize = MAX_SELECTED_PIECE_SLOTS * 2 * MAX_SOURCE_FEATURES;
const MAX_POSITION_SOURCES_PER_ORIENTATION: usize = 1 + 8 + MIXED_PIECE_COUNT * 8 + 16 + 16;
const MAX_POSITION_SOURCE_ATTEMPTS: usize =
    MAX_ORIENTATION_STREAMS * MAX_POSITION_SOURCES_PER_ORIENTATION;
const MAX_RETURNED_POSITIONS: usize = MAX_ORIENTATION_STREAMS * POSITIONS_PER_ORIENTATION;
const MAX_HAZARD_QUERIES: usize = MAX_RETURNED_POSITIONS;
const MAX_PROXY_PRESSURE_VISITS: usize = MAX_HAZARD_QUERIES * MIXED_PIECE_COUNT;
const MAX_EXACT_FINALIST_ROWS: usize = MAX_SELECTED_PIECE_SLOTS * FINALISTS_PER_PIECE;
const MAX_EXPERIMENTAL_COLLISION_BUILDS: usize =
    MIXED_PIECE_COUNT + MAX_ORIENTATION_STREAMS + MAX_EXACT_FINALIST_ROWS;
const MAX_VALIDATOR_AUDITS: usize = MAX_PARTIAL_AUDITS + MAX_COMPLETE_AUDITS;
const MAX_VALIDATOR_COLLISION_BUILDS: usize = MAX_VALIDATOR_AUDITS * MIXED_PIECE_COUNT * 2;
const MAX_EXPERIMENTAL_PAIR_VISITS: usize = MIXED_PIECE_COUNT * (MIXED_PIECE_COUNT - 1) / 2
    + MAX_EXACT_FINALIST_ROWS * (MIXED_PIECE_COUNT - 1);
const MAX_VALIDATOR_PAIR_VISITS: usize =
    MAX_VALIDATOR_AUDITS * MIXED_PIECE_COUNT * (MIXED_PIECE_COUNT - 1);
const MAX_TRANSFORMED_COLLISION_VERTICES: usize =
    (MAX_EXPERIMENTAL_COLLISION_BUILDS + MAX_VALIDATOR_COLLISION_BUILDS) * MAX_COLLISION_VERTICES;
const MAX_CLIPPER_INPUT_VERTICES: usize =
    (MAX_EXPERIMENTAL_PAIR_VISITS + MAX_VALIDATOR_PAIR_VISITS) * 2 * MAX_COLLISION_VERTICES;
const MAX_CLIPPER_OUTPUT_VERTICES: usize = 4_000_000;
const MAX_PARTIAL_AUDITS: usize = 41;
const MAX_COMPLETE_AUDITS: usize = 64;
const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;
const REPAIR_MAX_SELECTED_PIECE_SLOTS: usize = MAX_SELECTED_PIECE_SLOTS + REPAIR_PARENT_EXPANSIONS;
const REPAIR_MAX_ORIENTATION_STREAMS: usize =
    REPAIR_MAX_SELECTED_PIECE_SLOTS * ORIENTATIONS_PER_PIECE;
const REPAIR_MAX_SOURCE_FEATURE_VISITS: usize =
    REPAIR_MAX_SELECTED_PIECE_SLOTS * 2 * MAX_SOURCE_FEATURES;
const REPAIR_MAX_POSITION_SOURCE_ATTEMPTS: usize =
    REPAIR_MAX_ORIENTATION_STREAMS * MAX_POSITION_SOURCES_PER_ORIENTATION;
const REPAIR_MAX_RETURNED_POSITIONS: usize =
    REPAIR_MAX_ORIENTATION_STREAMS * POSITIONS_PER_ORIENTATION;
const REPAIR_MAX_HAZARD_QUERIES: usize = REPAIR_MAX_RETURNED_POSITIONS;
const REPAIR_MAX_PROXY_PRESSURE_VISITS: usize = REPAIR_MAX_HAZARD_QUERIES * MIXED_PIECE_COUNT;
const REPAIR_MAX_EXACT_FINALIST_ROWS: usize = REPAIR_MAX_SELECTED_PIECE_SLOTS * FINALISTS_PER_PIECE;
const REPAIR_MAX_EXPERIMENTAL_COLLISION_BUILDS: usize =
    MIXED_PIECE_COUNT + REPAIR_MAX_ORIENTATION_STREAMS + REPAIR_MAX_EXACT_FINALIST_ROWS;
const REPAIR_MAX_PARTIAL_AUDITS: usize = MAX_PARTIAL_AUDITS + REPAIR_HORIZON + 1;
const REPAIR_MAX_COMPLETE_AUDITS: usize =
    MAX_COMPLETE_AUDITS + REPAIR_PARENT_EXPANSIONS * FINALISTS_PER_PIECE;
const REPAIR_MAX_VALIDATOR_AUDITS: usize = REPAIR_MAX_PARTIAL_AUDITS + REPAIR_MAX_COMPLETE_AUDITS;
const REPAIR_MAX_VALIDATOR_COLLISION_BUILDS: usize =
    REPAIR_MAX_VALIDATOR_AUDITS * MIXED_PIECE_COUNT * 2;
const REPAIR_MAX_EXPERIMENTAL_PAIR_VISITS: usize = MIXED_PIECE_COUNT * (MIXED_PIECE_COUNT - 1) / 2
    + REPAIR_MAX_EXACT_FINALIST_ROWS * (MIXED_PIECE_COUNT - 1);
const REPAIR_MAX_VALIDATOR_PAIR_VISITS: usize =
    REPAIR_MAX_VALIDATOR_AUDITS * MIXED_PIECE_COUNT * (MIXED_PIECE_COUNT - 1);
const REPAIR_MAX_TRANSFORMED_COLLISION_VERTICES: usize = (REPAIR_MAX_EXPERIMENTAL_COLLISION_BUILDS
    + REPAIR_MAX_VALIDATOR_COLLISION_BUILDS)
    * MAX_COLLISION_VERTICES;
const REPAIR_MAX_CLIPPER_INPUT_VERTICES: usize = (REPAIR_MAX_EXPERIMENTAL_PAIR_VISITS
    + REPAIR_MAX_VALIDATOR_PAIR_VISITS)
    * 2
    * MAX_COLLISION_VERTICES;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VacancyTransition {
    inserted: usize,
    ejected: Vec<usize>,
}

#[derive(Clone)]
struct VacancyState {
    placements: Vec<RelaxedPlacement>,
    active: Vec<bool>,
    collisions: Vec<Option<Arc<PolygonSet>>>,
    last_transition: Option<VacancyTransition>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VacancyStateIdentity {
    active_placements: Vec<(usize, i64, bool, i64, i64)>,
    inactive: Vec<usize>,
    last_transition: Option<VacancyTransition>,
}

#[derive(Clone)]
struct RepairNode {
    state: VacancyState,
    queue: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RepairNodeIdentity {
    state: VacancyStateIdentity,
    queue: Vec<usize>,
}

struct SelectedPieceExpansion {
    selection: GeneralPersistentVacancySelectionSlotDiagnostics,
    proposal_order_hash: String,
    exact_row_order_hash: String,
    generated_child_order_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ContactEdge {
    first_id: String,
    second_id: String,
    axis: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ContactSignature {
    active_ids: Vec<String>,
    edges: Vec<ContactEdge>,
}

#[derive(Clone, Debug)]
struct PieceDifficulty {
    expanded_area_grid2: i128,
    hull_deficit_grid2: i128,
    minimum_side_grid: i64,
    material_area_grid2: i128,
}

#[derive(Clone)]
struct RankedProposal {
    placement: RelaxedPlacement,
    proxy_loss: f64,
    orientation_ordinal: usize,
    diversity_key: u64,
}

struct SelectedInactivePieces {
    indices: Vec<usize>,
    rotation_start_index: Option<usize>,
}

#[derive(Clone)]
struct EliteSnapshot {
    fingerprint: String,
    inactive_piece_count: usize,
    inactive_area_grid2: i128,
    inactive_difficulty_sequence: Vec<(i128, i128, i64, String)>,
    ejected_material_area_grid2: i128,
    ejected_piece_count: usize,
    active_frontier_grid: i64,
    identity: VacancyStateIdentity,
}

struct RunWork {
    diagnostics: GeneralPersistentVacancyWorkDiagnostics,
    limits: WorkLimits,
}

#[derive(Clone, Copy)]
struct WorkLimits {
    selected_piece_slots: usize,
    orientation_streams: usize,
    source_feature_visits: usize,
    position_source_attempts: usize,
    returned_positions: usize,
    hazard_queries: usize,
    proxy_pressure_visits: usize,
    exact_finalist_rows: usize,
    experimental_collision_builds: usize,
    validator_collision_builds: usize,
    experimental_pair_visits: usize,
    validator_pair_visits: usize,
    transformed_collision_vertices: usize,
    clipper_input_vertices: usize,
    partial_audits: usize,
    complete_audits: usize,
}

impl Default for WorkLimits {
    fn default() -> Self {
        Self {
            selected_piece_slots: MAX_SELECTED_PIECE_SLOTS,
            orientation_streams: MAX_ORIENTATION_STREAMS,
            source_feature_visits: MAX_SOURCE_FEATURE_VISITS,
            position_source_attempts: MAX_POSITION_SOURCE_ATTEMPTS,
            returned_positions: MAX_RETURNED_POSITIONS,
            hazard_queries: MAX_HAZARD_QUERIES,
            proxy_pressure_visits: MAX_PROXY_PRESSURE_VISITS,
            exact_finalist_rows: MAX_EXACT_FINALIST_ROWS,
            experimental_collision_builds: MAX_EXPERIMENTAL_COLLISION_BUILDS,
            validator_collision_builds: MAX_VALIDATOR_COLLISION_BUILDS,
            experimental_pair_visits: MAX_EXPERIMENTAL_PAIR_VISITS,
            validator_pair_visits: MAX_VALIDATOR_PAIR_VISITS,
            transformed_collision_vertices: MAX_TRANSFORMED_COLLISION_VERTICES,
            clipper_input_vertices: MAX_CLIPPER_INPUT_VERTICES,
            partial_audits: MAX_PARTIAL_AUDITS,
            complete_audits: MAX_COMPLETE_AUDITS,
        }
    }
}

impl WorkLimits {
    fn repair() -> Self {
        Self {
            selected_piece_slots: REPAIR_MAX_SELECTED_PIECE_SLOTS,
            orientation_streams: REPAIR_MAX_ORIENTATION_STREAMS,
            source_feature_visits: REPAIR_MAX_SOURCE_FEATURE_VISITS,
            position_source_attempts: REPAIR_MAX_POSITION_SOURCE_ATTEMPTS,
            returned_positions: REPAIR_MAX_RETURNED_POSITIONS,
            hazard_queries: REPAIR_MAX_HAZARD_QUERIES,
            proxy_pressure_visits: REPAIR_MAX_PROXY_PRESSURE_VISITS,
            exact_finalist_rows: REPAIR_MAX_EXACT_FINALIST_ROWS,
            experimental_collision_builds: REPAIR_MAX_EXPERIMENTAL_COLLISION_BUILDS,
            validator_collision_builds: REPAIR_MAX_VALIDATOR_COLLISION_BUILDS,
            experimental_pair_visits: REPAIR_MAX_EXPERIMENTAL_PAIR_VISITS,
            validator_pair_visits: REPAIR_MAX_VALIDATOR_PAIR_VISITS,
            transformed_collision_vertices: REPAIR_MAX_TRANSFORMED_COLLISION_VERTICES,
            clipper_input_vertices: REPAIR_MAX_CLIPPER_INPUT_VERTICES,
            partial_audits: REPAIR_MAX_PARTIAL_AUDITS,
            complete_audits: REPAIR_MAX_COMPLETE_AUDITS,
        }
    }
}

impl Default for RunWork {
    fn default() -> Self {
        Self {
            diagnostics: GeneralPersistentVacancyWorkDiagnostics::default(),
            limits: WorkLimits::default(),
        }
    }
}

impl RunWork {
    fn for_mode(_mode: usize) -> Self {
        Self {
            diagnostics: GeneralPersistentVacancyWorkDiagnostics::default(),
            limits: WorkLimits::default(),
        }
    }

    fn cap(&self, reason: &str) -> String {
        format!("cap: {reason}")
    }

    fn charge_source_features(&mut self, amount: usize) -> Result<(), String> {
        self.diagnostics.source_feature_visits = self
            .diagnostics
            .source_feature_visits
            .saturating_add(amount);
        if self.diagnostics.source_feature_visits > self.limits.source_feature_visits {
            return Err(self.cap("source-feature visit budget exhausted"));
        }
        Ok(())
    }

    fn charge_position_sources(&mut self, amount: usize) -> Result<(), String> {
        self.diagnostics.position_source_attempts = self
            .diagnostics
            .position_source_attempts
            .saturating_add(amount);
        if self.diagnostics.position_source_attempts > self.limits.position_source_attempts {
            return Err(self.cap("position-source attempt budget exhausted"));
        }
        Ok(())
    }

    fn charge_experimental_pair(&mut self) -> Result<(), String> {
        self.diagnostics.experimental_pair_visits =
            self.diagnostics.experimental_pair_visits.saturating_add(1);
        if self.diagnostics.experimental_pair_visits > self.limits.experimental_pair_visits {
            return Err(self.cap("experimental pair-visit budget exhausted"));
        }
        Ok(())
    }

    fn charge_validator_audit(&mut self, complete: bool) -> Result<(), String> {
        if complete {
            if self.diagnostics.complete_audits >= self.limits.complete_audits {
                return Err(self.cap("complete-audit budget exhausted"));
            }
            self.diagnostics.complete_audits += 1;
        } else {
            if self.diagnostics.partial_audits >= self.limits.partial_audits {
                return Err(self.cap("partial-audit budget exhausted"));
            }
            self.diagnostics.partial_audits += 1;
        }
        let collision_builds = 2usize.saturating_mul(61);
        let pair_visits = 2usize.saturating_mul(61 * 60 / 2);
        let collision_vertices = collision_builds.saturating_mul(MAX_COLLISION_VERTICES);
        let input_vertices = pair_visits.saturating_mul(2 * MAX_COLLISION_VERTICES);
        if self
            .diagnostics
            .validator_collision_builds
            .saturating_add(collision_builds)
            > self.limits.validator_collision_builds
        {
            return Err(self.cap("validator collision-build budget exhausted"));
        }
        if self
            .diagnostics
            .validator_pair_visits
            .saturating_add(pair_visits)
            > self.limits.validator_pair_visits
        {
            return Err(self.cap("validator pair-visit budget exhausted"));
        }
        if self
            .diagnostics
            .transformed_collision_vertices
            .saturating_add(collision_vertices)
            > self.limits.transformed_collision_vertices
        {
            return Err(self.cap("transformed collision-vertex budget exhausted"));
        }
        if self
            .diagnostics
            .clipper_input_vertices
            .saturating_add(input_vertices)
            > self.limits.clipper_input_vertices
        {
            return Err(self.cap("validator Clipper input-vertex budget exhausted"));
        }
        self.diagnostics.validator_collision_builds += collision_builds;
        self.diagnostics.validator_pair_visits += pair_visits;
        self.diagnostics.transformed_collision_vertices += collision_vertices;
        self.diagnostics.clipper_input_vertices += input_vertices;
        Ok(())
    }
}

fn uses_macro_expansion(mode: usize) -> bool {
    matches!(
        mode,
        MACRO_CONTROL_MODE
            | MACRO_TREATMENT_MODE
            | PRESERVED_BEST_MACRO_MODE
            | REPAIR_CONTROL_MODE
            | REPAIR_TREATMENT_MODE
    )
}

fn admits_macro_children(mode: usize) -> bool {
    matches!(
        mode,
        MACRO_TREATMENT_MODE
            | PRESERVED_BEST_MACRO_MODE
            | REPAIR_CONTROL_MODE
            | REPAIR_TREATMENT_MODE
    )
}

fn uses_preserved_best_macro(mode: usize) -> bool {
    matches!(
        mode,
        PRESERVED_BEST_MACRO_MODE | REPAIR_CONTROL_MODE | REPAIR_TREATMENT_MODE
    )
}

fn uses_repair_expedition(mode: usize) -> bool {
    matches!(mode, REPAIR_CONTROL_MODE | REPAIR_TREATMENT_MODE)
}

struct MacroParentChoice<'a> {
    state: &'a VacancyState,
    origin: Option<&'static str>,
    preserved_parent_absent_from_ordinary: Option<bool>,
}

fn select_macro_parent<'a>(
    ordinary_children: &'a [VacancyState],
    preserved_best: Option<&'a VacancyState>,
    mode: usize,
) -> Option<MacroParentChoice<'a>> {
    let ordinary = ordinary_children
        .iter()
        .find(|state| state.active.iter().any(|active| !*active))?;
    if !uses_preserved_best_macro(mode) {
        return Some(MacroParentChoice {
            state: ordinary,
            origin: None,
            preserved_parent_absent_from_ordinary: None,
        });
    }
    let Some(preserved) = preserved_best else {
        return Some(MacroParentChoice {
            state: ordinary,
            origin: Some("ordinaryBest"),
            preserved_parent_absent_from_ordinary: None,
        });
    };
    let preserved_absent = ordinary_children
        .iter()
        .all(|state| !same_state_identity(state, preserved));
    if preserved_absent && preserved.active.iter().any(|active| !*active) {
        Some(MacroParentChoice {
            state: preserved,
            origin: Some("bestEverArea"),
            preserved_parent_absent_from_ordinary: Some(true),
        })
    } else {
        Some(MacroParentChoice {
            state: ordinary,
            origin: Some("ordinaryBest"),
            preserved_parent_absent_from_ordinary: Some(false),
        })
    }
}

pub(super) fn run_persistent_vacancy_population(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    _relaxed_settings: GeneralRelaxedSettings,
    parent_placements: &[GeneralCoupledSeparatorPlacementDiagnostics],
    mode: usize,
) -> GeneralPersistentVacancyDiagnostics {
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode,
        seed_domain: PERSISTENT_VACANCY_SEED_DOMAIN,
        target_depth_mm: TARGET_DEPTH_MM,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let mut work = RunWork::for_mode(mode);
    match run_population(
        pieces,
        fast_settings,
        parent_placements,
        mode,
        &mut diagnostics,
        &mut work,
    ) {
        Ok(Some((state, metrics))) => {
            diagnostics.exact_valid = true;
            diagnostics.independent_depth_mm = Some(metrics);
            let placements = fast_placements(&state, pieces, false);
            diagnostics.final_placement_fingerprint =
                Some(coupled_fast_placement_fingerprint(&placements));
            diagnostics.final_placements = coupled_placement_diagnostics(&placements);
        }
        Ok(None) => {
            diagnostics.failure_reason = Some(
                "persistent vacancy population exhausted its bounded layers without a complete state"
                    .to_owned(),
            );
        }
        Err(reason) => {
            diagnostics.cap_exhausted = reason.strip_prefix("cap: ").map(str::to_owned);
            diagnostics.failure_reason = Some(reason);
        }
    }
    diagnostics.work = work.diagnostics;
    diagnostics
}

fn run_population(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    parent_placements: &[GeneralCoupledSeparatorPlacementDiagnostics],
    mode: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<Option<(VacancyState, f64)>, String> {
    if !matches!(
        mode,
        1 | 2
            | 3
            | 4
            | 5
            | 6
            | MACRO_CONTROL_MODE
            | MACRO_TREATMENT_MODE
            | PRESERVED_BEST_MACRO_MODE
            | REPAIR_CONTROL_MODE
            | REPAIR_TREATMENT_MODE
    ) {
        return Err(
            "persistent vacancy mode must be 1, 2, 3, 4, 5, 6, 8, 9, 10, 14, or 15; retired modes 7 and 11 through 13 are unavailable"
                .to_owned(),
        );
    }
    if pieces.len() != MIXED_PIECE_COUNT {
        return Err("persistent vacancy experiment is pinned to Mixed-61".to_owned());
    }
    if parent_placements.len() != pieces.len() {
        return Err("persistent vacancy parent is not a complete exact-valid layout".to_owned());
    }
    let parent_fast = diagnostic_fast_placements(parent_placements);
    validate_and_measure_placements(pieces, &parent_fast, fast_settings)
        .map_err(|error| format!("persistent vacancy parent validation: {error}"))?;
    let parent_fingerprint = coupled_fast_placement_fingerprint(&parent_fast);
    diagnostics.parent_fingerprint = Some(parent_fingerprint.clone());
    if parent_fingerprint != EXPECTED_PARENT_FINGERPRINT {
        return Err(format!(
            "persistent vacancy parent fingerprint mismatch: expected {EXPECTED_PARENT_FINGERPRINT}, got {parent_fingerprint}"
        ));
    }
    let parent_depth = coupled_independent_source_depth(pieces, &parent_fast, fast_settings)
        .map_err(|error| format!("persistent vacancy parent depth: {error}"))?;
    if grid_key(parent_depth) != grid_key(EXPECTED_PARENT_DEPTH_MM) {
        return Err(format!(
            "persistent vacancy parent depth mismatch: expected {EXPECTED_PARENT_DEPTH_MM}, got {parent_depth}"
        ));
    }
    for piece in pieces {
        if piece.polygon.vertex_count() > MAX_SOURCE_FEATURES {
            return Err(format!(
                "piece {} exceeds the {MAX_SOURCE_FEATURES}-feature experiment cap",
                piece.id
            ));
        }
    }

    diagnostics.attempted = true;
    let target_settings = GeneralFastSettings {
        sheet_long_axis_mm: TARGET_DEPTH_MM,
        ..fast_settings
    };
    let baseline = relaxed_state_from_diagnostics(pieces, parent_placements)?;
    let (initial, difficulty, inactive_order) =
        initial_vacancy_state(pieces, target_settings, baseline, diagnostics, work)?;
    diagnostics.initial_state_fingerprint = Some(state_fingerprint(&initial, pieces));
    diagnostics.initial_active_piece_ids = active_ids(&initial, pieces);
    diagnostics.initial_inactive_piece_ids = inactive_order
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect();
    diagnostics.initial_inactive_order_hash = Some(id_order_hash(&inactive_order, pieces));
    audit_state(&initial, pieces, target_settings, false, work)?;

    let hazard_catalog = Arc::new(
        JaguaHazardCatalog::new(pieces, target_settings)
            .map_err(|error| format!("persistent vacancy hazard catalog: {error}"))?,
    );
    let baseline_placements = initial.placements.clone();
    let mut population = vec![initial];
    let mut best_ever_area: Option<EliteSnapshot> = None;
    let mut best_ever_count: Option<EliteSnapshot> = None;
    let mut best_ever_area_state: Option<VacancyState> = None;
    let mut best_ever_count_state: Option<VacancyState> = None;
    let mut retained_carryovers = BTreeSet::new();
    for layer in 0..MAX_LAYERS {
        let layer_entry_work = generation_work_snapshot(work.diagnostics);
        let entering_population_hash = population_hash(&population, pieces);
        let expanded_carryover_fingerprints = population
            .iter()
            .map(|state| state_fingerprint(state, pieces))
            .filter(|fingerprint| retained_carryovers.contains(fingerprint))
            .collect::<Vec<_>>();
        let carryover_states = if mode == 5 {
            distinct_elite_states(&population, pieces, &difficulty)
        } else {
            Vec::new()
        };
        let offered_carryover_fingerprints = carryover_states
            .iter()
            .map(|state| state_fingerprint(state, pieces))
            .collect::<Vec<_>>();
        let mut children = Vec::new();
        let mut selected_piece_ids = BTreeSet::new();
        let mut parent_selections = Vec::new();
        let direct_before = diagnostics.direct_insertions;
        let ejections_before = diagnostics.ejection_insertions;
        for parent_state in &population {
            expand_parent(
                parent_state,
                &baseline_placements,
                pieces,
                target_settings,
                &difficulty,
                &hazard_catalog,
                layer,
                mode,
                diagnostics,
                work,
                &mut selected_piece_ids,
                &mut parent_selections,
                &mut children,
            )?;
        }
        if children.is_empty() {
            return Err(format!(
                "persistent vacancy layer {layer} produced no exact-valid child"
            ));
        }
        let selected_piece_ids = selected_piece_ids.into_iter().collect::<Vec<_>>();
        let mut generated_live_state_bytes = state_vec_bytes(&children);
        let carryover_live_state_bytes = state_vec_bytes(&carryover_states);
        let mut combined_pool_backing_bytes = children
            .len()
            .saturating_add(carryover_states.len())
            .saturating_mul(size_of::<VacancyState>());
        let mut largest_clone_bytes = 0usize;
        for state in children.iter().chain(&carryover_states) {
            let bytes = size_of::<VacancyState>().saturating_add(state_heap_bytes(state));
            largest_clone_bytes = largest_clone_bytes.max(bytes);
        }
        let mut retained_clone_bytes = largest_clone_bytes.saturating_mul(2);
        let preserved_best_live_state_bytes = sidecar_bytes(
            best_ever_area_state.as_ref(),
            best_ever_count_state.as_ref(),
        );
        preflight_raw_live_memory(
            &population,
            preserved_best_live_state_bytes,
            generated_live_state_bytes,
            carryover_live_state_bytes,
            retained_clone_bytes,
            combined_pool_backing_bytes,
            &selected_piece_ids,
            &parent_selections,
            0,
            diagnostics,
            work,
        )?;
        children.sort_by(|first, second| compare_states(first, second, pieces, &difficulty));
        let before_dedup = children.len();
        children.dedup_by(|first, second| same_state_identity(first, second));
        diagnostics.deduplicated_states = diagnostics
            .deduplicated_states
            .saturating_add(before_dedup.saturating_sub(children.len()));
        let ordinary_child_order_hash = child_order_hash(&children, pieces);

        let mut macro_expansion = None;
        let mut combined_macro_children = None;
        if uses_macro_expansion(mode) {
            let macro_parent = select_macro_parent(&children, best_ever_area_state.as_ref(), mode);
            if let Some(macro_parent) = macro_parent {
                let parent_origin = macro_parent.origin.map(str::to_owned);
                let preserved_parent_absent_from_ordinary =
                    macro_parent.preserved_parent_absent_from_ordinary;
                let macro_parent_clone_bytes = owned_state_bytes(Some(macro_parent.state));
                let prospective_macro_parent_live_state_bytes =
                    state_vec_bytes(&children).saturating_add(macro_parent_clone_bytes);
                preflight_raw_live_memory(
                    &population,
                    preserved_best_live_state_bytes,
                    prospective_macro_parent_live_state_bytes,
                    carryover_live_state_bytes,
                    retained_clone_bytes,
                    combined_pool_backing_bytes,
                    &selected_piece_ids,
                    &parent_selections,
                    0,
                    diagnostics,
                    work,
                )?;
                let macro_parent = macro_parent.state.clone();
                let macro_parent_fingerprint = state_fingerprint(&macro_parent, pieces);
                let macro_work_before = generation_work_snapshot(work.diagnostics);
                let macro_direct_before = diagnostics.direct_insertions;
                let macro_ejections_before = diagnostics.ejection_insertions;
                let mut macro_selected_piece_ids = BTreeSet::new();
                let mut macro_parent_selections = Vec::new();
                let mut macro_children = Vec::new();
                expand_parent(
                    &macro_parent,
                    &baseline_placements,
                    pieces,
                    target_settings,
                    &difficulty,
                    &hazard_catalog,
                    layer,
                    mode,
                    diagnostics,
                    work,
                    &mut macro_selected_piece_ids,
                    &mut macro_parent_selections,
                    &mut macro_children,
                )?;
                if macro_parent_selections.len() != 1 {
                    return Err(format!(
                        "persistent vacancy layer {layer} recorded {} macro parent selections",
                        macro_parent_selections.len()
                    ));
                }
                let macro_selected_piece_ids =
                    macro_selected_piece_ids.into_iter().collect::<Vec<_>>();
                let raw_macro_live_state_bytes = state_vec_bytes(&children)
                    .saturating_add(state_vec_bytes(&macro_children))
                    .saturating_add(macro_parent_clone_bytes);
                generated_live_state_bytes =
                    generated_live_state_bytes.max(raw_macro_live_state_bytes);
                for state in &macro_children {
                    let bytes = size_of::<VacancyState>().saturating_add(state_heap_bytes(state));
                    largest_clone_bytes = largest_clone_bytes.max(bytes);
                }
                retained_clone_bytes = largest_clone_bytes.saturating_mul(2);
                preflight_raw_live_memory(
                    &population,
                    preserved_best_live_state_bytes,
                    raw_macro_live_state_bytes,
                    carryover_live_state_bytes,
                    retained_clone_bytes,
                    combined_pool_backing_bytes,
                    &selected_piece_ids,
                    &parent_selections,
                    pending_selection_diagnostic_bytes(
                        &macro_selected_piece_ids,
                        &macro_parent_selections,
                    ),
                    diagnostics,
                    work,
                )?;
                macro_children
                    .sort_by(|first, second| compare_states(first, second, pieces, &difficulty));
                let macro_before_dedup = macro_children.len();
                macro_children.dedup_by(|first, second| same_state_identity(first, second));
                diagnostics.deduplicated_states = diagnostics
                    .deduplicated_states
                    .saturating_add(macro_before_dedup.saturating_sub(macro_children.len()));
                let macro_child_order_hash = child_order_hash(&macro_children, pieces);
                let macro_generated_children = macro_children.len();
                let novel_child_fingerprints =
                    novel_macro_child_fingerprints(&children, &macro_children, pieces);
                let admitted_children = if admits_macro_children(mode) {
                    novel_child_fingerprints.len()
                } else {
                    0
                };
                macro_expansion = Some(GeneralPersistentVacancyMacroExpansionDiagnostics {
                    parent_state_fingerprint: macro_parent_fingerprint,
                    parent_origin,
                    preserved_parent_absent_from_ordinary,
                    generated_children: macro_generated_children,
                    child_order_hash: macro_child_order_hash,
                    novel_child_fingerprints,
                    admitted_children,
                    retained_child_fingerprints: Vec::new(),
                    direct_insertions: diagnostics
                        .direct_insertions
                        .saturating_sub(macro_direct_before),
                    ejection_insertions: diagnostics
                        .ejection_insertions
                        .saturating_sub(macro_ejections_before),
                    selected_piece_ids: macro_selected_piece_ids,
                    parent_selection: macro_parent_selections.remove(0),
                    work: work_delta(
                        generation_work_snapshot(work.diagnostics),
                        macro_work_before,
                    ),
                });
                let combined_capacity = children.len().saturating_add(macro_children.len());
                let prospective_combined_clone_bytes = combined_capacity
                    .saturating_mul(size_of::<VacancyState>())
                    .saturating_add(state_slice_bytes(&children));
                let raw_combined_live_state_bytes = state_vec_bytes(&children)
                    .saturating_add(state_vec_bytes(&macro_children))
                    .saturating_add(macro_parent_clone_bytes)
                    .saturating_add(prospective_combined_clone_bytes);
                generated_live_state_bytes =
                    generated_live_state_bytes.max(raw_combined_live_state_bytes);
                combined_pool_backing_bytes = combined_pool_backing_bytes.max(
                    combined_capacity
                        .saturating_add(carryover_states.len())
                        .saturating_mul(size_of::<VacancyState>()),
                );
                preflight_raw_live_memory(
                    &population,
                    preserved_best_live_state_bytes,
                    raw_combined_live_state_bytes,
                    carryover_live_state_bytes,
                    retained_clone_bytes,
                    combined_pool_backing_bytes,
                    &selected_piece_ids,
                    &parent_selections,
                    macro_expansion
                        .as_ref()
                        .map_or(0, macro_expansion_diagnostic_heap_bytes),
                    diagnostics,
                    work,
                )?;
                let mut combined_children = Vec::with_capacity(combined_capacity);
                combined_children.extend(children.iter().cloned());
                combined_children.append(&mut macro_children);
                combined_children
                    .sort_by(|first, second| compare_states(first, second, pieces, &difficulty));
                let combined_before_dedup = combined_children.len();
                combined_children.dedup_by(|first, second| same_state_identity(first, second));
                diagnostics.deduplicated_states = diagnostics
                    .deduplicated_states
                    .saturating_add(combined_before_dedup.saturating_sub(combined_children.len()));
                combined_macro_children = Some(combined_children);
            }
        }

        let complete_candidates = combined_macro_children.as_ref().unwrap_or(&children);
        let complete_count = complete_candidates
            .iter()
            .take_while(|state| state.active.iter().all(|active| *active))
            .count();
        let complete_candidate_order_hash =
            child_order_hash(&complete_candidates[..complete_count], pieces);
        diagnostics.complete_states = diagnostics.complete_states.saturating_add(complete_count);
        let accepted_complete = if uses_macro_expansion(mode) {
            let (combined_accepted, ordinary_accepted) = audit_macro_complete_candidates(
                &complete_candidates[..complete_count],
                &children,
                pieces,
                target_settings,
                diagnostics,
                work,
            )?;
            if admits_macro_children(mode) {
                combined_accepted
            } else {
                ordinary_accepted
            }
        } else {
            audit_first_complete_candidate(
                &complete_candidates[..complete_count],
                pieces,
                target_settings,
                diagnostics,
                work,
            )?
        };
        children.retain(|state| state.active.iter().any(|active| !*active));
        if let Some(combined_children) = &mut combined_macro_children {
            combined_children.retain(|state| state.active.iter().any(|active| !*active));
        }
        children = select_macro_retention_children(children, combined_macro_children.take(), mode);
        if children.is_empty() && accepted_complete.is_none() {
            return Err(format!(
                "persistent vacancy layer {layer} retained only publication-invalid complete states"
            ));
        }

        let generated_children = children.len();
        let effective_carryover_fingerprints = if mode == 5 {
            let ordinary_partial_fingerprints = children
                .iter()
                .map(|state| state_fingerprint(state, pieces))
                .collect::<BTreeSet<_>>();
            carryover_states
                .iter()
                .map(|state| state_fingerprint(state, pieces))
                .filter(|fingerprint| !ordinary_partial_fingerprints.contains(fingerprint))
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
        let pre_carryover_work =
            work_delta(generation_work_snapshot(work.diagnostics), layer_entry_work);
        if accepted_complete.is_none() {
            let (combined, carryover_deduplicated) =
                retention_pool(children, carryover_states, pieces, &difficulty, mode);
            children = combined;
            diagnostics.deduplicated_states = diagnostics
                .deduplicated_states
                .saturating_add(carryover_deduplicated);
        }
        let (next, distinct_signatures) = if let Some((candidate, _)) = &accepted_complete {
            (vec![candidate.clone()], 1)
        } else {
            retain_population(children, pieces, &difficulty, mode)
        };
        if next.is_empty() {
            return Err(format!(
                "persistent vacancy layer {layer} retained no state"
            ));
        }
        if admits_macro_children(mode) {
            if let Some(macro_diagnostics) = &mut macro_expansion {
                macro_diagnostics.retained_child_fingerprints = next
                    .iter()
                    .filter_map(|state| {
                        let fingerprint = state_fingerprint(state, pieces);
                        macro_diagnostics
                            .novel_child_fingerprints
                            .contains(&fingerprint)
                            .then_some(fingerprint)
                    })
                    .collect();
            }
        }
        enforce_population_width(mode, accepted_complete.is_some(), next.len(), layer)?;
        diagnostics.distinct_signatures_retained = diagnostics
            .distinct_signatures_retained
            .saturating_add(distinct_signatures);
        if accepted_complete.is_none() {
            audit_state(&next[0], pieces, target_settings, false, work)?;
        }
        let retained_carryover_fingerprints = if mode == 5 {
            next.iter()
                .map(|state| state_fingerprint(state, pieces))
                .filter(|fingerprint| effective_carryover_fingerprints.contains(fingerprint))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let (area_elite, count_elite) = population_elites(&next, pieces, &difficulty);
        let area_snapshot = elite_snapshot(area_elite, pieces, &difficulty);
        let count_snapshot = elite_snapshot(count_elite, pieces, &difficulty);
        let area_improved = best_ever_area.as_ref().map_or(true, |current| {
            compare_area_snapshots(&area_snapshot, current).is_lt()
        });
        let count_improved = best_ever_count.as_ref().map_or(true, |current| {
            compare_count_snapshots(&count_snapshot, current).is_lt()
        });
        let replace_area =
            uses_preserved_best_macro(mode) && area_improved && accepted_complete.is_none();
        let replace_count =
            uses_repair_expedition(mode) && count_improved && accepted_complete.is_none();
        if replace_area || replace_count {
            let transient_sidecar_bytes = preserved_best_live_state_bytes
                .saturating_add(
                    replace_area
                        .then(|| owned_state_bytes(Some(area_elite)))
                        .unwrap_or(0),
                )
                .saturating_add(
                    replace_count
                        .then(|| owned_state_bytes(Some(count_elite)))
                        .unwrap_or(0),
                );
            preflight_raw_live_memory(
                &population,
                transient_sidecar_bytes,
                generated_live_state_bytes,
                carryover_live_state_bytes,
                retained_clone_bytes,
                combined_pool_backing_bytes,
                &selected_piece_ids,
                &parent_selections,
                macro_expansion
                    .as_ref()
                    .map_or(0, macro_expansion_diagnostic_heap_bytes),
                diagnostics,
                work,
            )?;
            let next_area = replace_area.then(|| area_elite.clone());
            let next_count = replace_count.then(|| count_elite.clone());
            if let Some(next_area) = next_area {
                best_ever_area_state = Some(next_area);
            }
            if let Some(next_count) = next_count {
                best_ever_count_state = Some(next_count);
            }
        }
        update_best_area(&mut best_ever_area, &area_snapshot);
        update_best_count(&mut best_ever_count, &count_snapshot);
        let best_ever_area_snapshot = best_ever_area
            .as_ref()
            .expect("the current area elite initializes best-ever history");
        let best_ever_count_snapshot = best_ever_count
            .as_ref()
            .expect("the current count elite initializes best-ever history");
        if uses_preserved_best_macro(mode)
            && accepted_complete.is_none()
            && best_ever_area_state
                .as_ref()
                .map(|state| state_fingerprint(state, pieces))
                .as_deref()
                != Some(best_ever_area_snapshot.fingerprint.as_str())
        {
            return Err(format!(
                "persistent vacancy layer {layer} diverged preserved best-area state from diagnostics"
            ));
        }
        if uses_repair_expedition(mode)
            && accepted_complete.is_none()
            && best_ever_count_state
                .as_ref()
                .map(|state| state_fingerprint(state, pieces))
                .as_deref()
                != Some(best_ever_count_snapshot.fingerprint.as_str())
        {
            return Err(format!(
                "persistent vacancy layer {layer} diverged preserved best-count state from diagnostics"
            ));
        }
        let best_identity = state_identity(&next[0]);
        let layer_diagnostics = GeneralPersistentVacancyLayerDiagnostics {
            layer,
            parents: population.len(),
            generated_children,
            retained_states: next.len(),
            distinct_contact_signatures: distinct_signatures,
            selected_piece_ids,
            parent_selections,
            direct_insertions: diagnostics.direct_insertions.saturating_sub(direct_before),
            ejection_insertions: diagnostics
                .ejection_insertions
                .saturating_sub(ejections_before),
            best_inactive_piece_count: best_identity.inactive.len(),
            best_inactive_piece_ids: best_identity
                .inactive
                .iter()
                .map(|index| pieces[*index].id.to_owned())
                .collect(),
            best_inactive_area_grid2: inactive_area(&next[0], &difficulty).to_string(),
            best_state_fingerprint: state_fingerprint(&next[0], pieces),
            retained_population_hash: uses_preserved_best_macro(mode)
                .then(|| population_hash(&next, pieces)),
            macro_expansion,
            elite: Some(GeneralPersistentVacancyEliteLayerDiagnostics {
                entering_population_hash,
                ordinary_child_order_hash,
                complete_candidate_order_hash,
                pre_carryover_work,
                area_elite_fingerprint: area_snapshot.fingerprint.clone(),
                area_elite_inactive_piece_count: area_snapshot.inactive_piece_count,
                area_elite_inactive_area_grid2: area_snapshot.inactive_area_grid2.to_string(),
                count_elite_fingerprint: count_snapshot.fingerprint.clone(),
                count_elite_inactive_piece_count: count_snapshot.inactive_piece_count,
                count_elite_inactive_area_grid2: count_snapshot.inactive_area_grid2.to_string(),
                best_ever_area_elite_fingerprint: best_ever_area_snapshot.fingerprint.clone(),
                best_ever_area_elite_inactive_piece_count: best_ever_area_snapshot
                    .inactive_piece_count,
                best_ever_area_elite_inactive_area_grid2: best_ever_area_snapshot
                    .inactive_area_grid2
                    .to_string(),
                best_ever_count_elite_fingerprint: best_ever_count_snapshot.fingerprint.clone(),
                best_ever_count_elite_inactive_piece_count: best_ever_count_snapshot
                    .inactive_piece_count,
                best_ever_count_elite_inactive_area_grid2: best_ever_count_snapshot
                    .inactive_area_grid2
                    .to_string(),
                offered_carryovers_distinct: offered_carryover_fingerprints.len() > 1,
                offered_carryover_fingerprints,
                retained_carryover_fingerprints: retained_carryover_fingerprints.clone(),
                expanded_carryover_fingerprints,
            }),
        };
        preflight_live_memory(
            &population,
            sidecar_bytes(
                best_ever_area_state.as_ref(),
                best_ever_count_state.as_ref(),
            ),
            generated_live_state_bytes,
            carryover_live_state_bytes,
            retained_clone_bytes,
            combined_pool_backing_bytes,
            diagnostics,
            &layer_diagnostics,
            work,
        )?;
        charge_retained_memory(
            &next,
            best_ever_area_state.as_ref(),
            sidecar_bytes(
                best_ever_area_state.as_ref(),
                best_ever_count_state.as_ref(),
            ),
            diagnostics,
            &layer_diagnostics,
            work,
        )?;
        diagnostics.layers.push(layer_diagnostics);
        diagnostics.layers_completed = layer + 1;
        if let Some(complete) = accepted_complete {
            return Ok(Some(complete));
        }
        retained_carryovers = retained_carryover_fingerprints.into_iter().collect();
        population = next;
    }
    let pre_expedition_work = generation_work_snapshot(work.diagnostics);
    if uses_preserved_best_macro(mode) {
        diagnostics.pre_expedition_work = Some(pre_expedition_work);
        diagnostics.pre_expedition_behavior_hash = Some(pre_expedition_behavior_hash(diagnostics)?);
    }
    if !uses_repair_expedition(mode) {
        return Ok(None);
    }
    drop(population);
    drop(best_ever_area_state);
    let root = best_ever_count_state
        .as_ref()
        .ok_or_else(|| "persistent vacancy repair has no preserved best-count root".to_owned())?;
    let mut expedition_events = GeneralPersistentVacancyDiagnostics::default();
    let mut expedition_work = RunWork {
        diagnostics: repair_generation_work_start(work.diagnostics),
        limits: WorkLimits::repair(),
    };
    let mut repair_root_dual_valid = false;
    let expedition_work_before = generation_work_snapshot(work.diagnostics);
    let outcome = run_repair_expedition(
        root,
        &baseline_placements,
        pieces,
        target_settings,
        &difficulty,
        &hazard_catalog,
        mode,
        diagnostics,
        &mut expedition_events,
        &mut repair_root_dual_valid,
        &mut expedition_work,
    );
    let (accepted, repair) = match outcome {
        Ok(success) => success,
        Err(reason) => {
            let repair = failed_repair_diagnostics(
                root,
                pieces,
                &difficulty,
                mode,
                repair_root_dual_valid,
                &reason,
                repair_work_delta(
                    generation_work_snapshot(expedition_work.diagnostics),
                    expedition_work_before,
                    expedition_work.diagnostics,
                ),
            );
            merge_repair_work(work, expedition_work.diagnostics);
            diagnostics.repair_expedition = Some(repair);
            return Err(reason);
        }
    };
    commit_repair_expedition(
        diagnostics,
        work,
        expedition_events,
        expedition_work,
        repair,
    );
    Ok(accepted)
}

fn failed_repair_diagnostics(
    root: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
    mode: usize,
    root_dual_valid: bool,
    reason: &str,
    work: GeneralPersistentVacancyWorkDiagnostics,
) -> GeneralPersistentVacancyRepairDiagnostics {
    let root_queue = difficulty_inactive_order(root, pieces, difficulty);
    GeneralPersistentVacancyRepairDiagnostics {
        scheduler_family: repair_scheduler_family(mode).to_owned(),
        seed_domain: REPAIR_SEED_DOMAIN,
        root_state_fingerprint: state_fingerprint(root, pieces),
        root_inactive_piece_count: inactive_piece_count(root),
        root_inactive_area_grid2: inactive_area(root, difficulty).to_string(),
        root_queue_piece_ids: queue_piece_ids(&root_queue, pieces),
        root_dual_valid,
        work,
        cap_exhausted: reason.strip_prefix("cap: ").map(str::to_owned),
        failure_reason: Some(reason.to_owned()),
        ..GeneralPersistentVacancyRepairDiagnostics::default()
    }
}

fn repair_scheduler_family(mode: usize) -> &'static str {
    if mode == REPAIR_CONTROL_MODE {
        "oneSlotGlobalHardest"
    } else {
        "oneSlotDisplacedFirst"
    }
}

fn commit_repair_expedition(
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
    events: GeneralPersistentVacancyDiagnostics,
    expedition_work: RunWork,
    repair: GeneralPersistentVacancyRepairDiagnostics,
) {
    diagnostics.direct_insertions = diagnostics
        .direct_insertions
        .saturating_add(events.direct_insertions);
    diagnostics.ejection_insertions = diagnostics
        .ejection_insertions
        .saturating_add(events.ejection_insertions);
    diagnostics.immediate_reversals_rejected = diagnostics
        .immediate_reversals_rejected
        .saturating_add(events.immediate_reversals_rejected);
    diagnostics.complete_states = diagnostics
        .complete_states
        .saturating_add(events.complete_states);
    diagnostics.publication_rejections = diagnostics
        .publication_rejections
        .saturating_add(events.publication_rejections);
    merge_repair_work(work, expedition_work.diagnostics);
    diagnostics.repair_expedition = Some(repair);
}

fn repair_generation_work_start(
    mut diagnostics: GeneralPersistentVacancyWorkDiagnostics,
) -> GeneralPersistentVacancyWorkDiagnostics {
    diagnostics.retained_peak_bytes = 0;
    diagnostics.selector_diagnostic_peak_bytes = 0;
    diagnostics.total_retained_peak_bytes = 0;
    diagnostics
}

fn merge_repair_work(work: &mut RunWork, repair: GeneralPersistentVacancyWorkDiagnostics) {
    let previous = work.diagnostics;
    work.diagnostics = repair;
    work.diagnostics.retained_peak_bytes =
        previous.retained_peak_bytes.max(repair.retained_peak_bytes);
    work.diagnostics.selector_diagnostic_peak_bytes = previous
        .selector_diagnostic_peak_bytes
        .max(repair.selector_diagnostic_peak_bytes);
    work.diagnostics.total_retained_peak_bytes = previous
        .total_retained_peak_bytes
        .max(repair.total_retained_peak_bytes);
}

#[allow(clippy::too_many_arguments)]
fn run_repair_expedition(
    root: &VacancyState,
    baseline: &[RelaxedPlacement],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    difficulty: &[PieceDifficulty],
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    mode: usize,
    base_diagnostics: &GeneralPersistentVacancyDiagnostics,
    expedition_events: &mut GeneralPersistentVacancyDiagnostics,
    root_dual_valid: &mut bool,
    work: &mut RunWork,
) -> Result<
    (
        Option<(VacancyState, f64)>,
        GeneralPersistentVacancyRepairDiagnostics,
    ),
    String,
> {
    preflight_repair_memory(root, pieces, base_diagnostics, "root", work)?;
    let work_before = generation_work_snapshot(work.diagnostics);
    audit_state(root, pieces, settings, false, work)?;
    *root_dual_valid = true;
    let root_queue = difficulty_inactive_order(root, pieces, difficulty);
    validate_repair_queue(root, &root_queue)?;
    let root_node = RepairNode {
        state: root.clone(),
        queue: root_queue.clone(),
    };
    let root_count = inactive_piece_count(root);
    let root_area = inactive_area(root, difficulty);
    let mut repair = GeneralPersistentVacancyRepairDiagnostics {
        scheduler_family: repair_scheduler_family(mode).to_owned(),
        seed_domain: REPAIR_SEED_DOMAIN,
        root_state_fingerprint: state_fingerprint(root, pieces),
        root_inactive_piece_count: root_count,
        root_inactive_area_grid2: root_area.to_string(),
        root_queue_piece_ids: queue_piece_ids(&root_queue, pieces),
        root_dual_valid: true,
        depths: Vec::with_capacity(REPAIR_HORIZON),
        ..GeneralPersistentVacancyRepairDiagnostics::default()
    };
    let mut frontier = Vec::with_capacity(REPAIR_BEAM_WIDTH);
    frontier.push(root_node.clone());
    let mut seen = BTreeSet::new();
    seen.insert(repair_node_identity(&root_node));
    let mut best_partial = root_node;
    let mut best_complete: Option<(VacancyState, f64, String)> = None;

    for expansion_depth in 0..REPAIR_HORIZON {
        preflight_repair_memory(root, pieces, base_diagnostics, "depth allocation", work)?;
        let depth_work_before = generation_work_snapshot(work.diagnostics);
        let direct_before = expedition_events.direct_insertions;
        let ejection_before = expedition_events.ejection_insertions;
        let expanded_parents = frontier.len();
        let mut raw = Vec::with_capacity(REPAIR_BEAM_WIDTH * FINALISTS_PER_PIECE);
        let mut expansion_diagnostics = Vec::with_capacity(expanded_parents);
        for parent in &frontier {
            let piece_index = *parent
                .queue
                .first()
                .ok_or_else(|| "repair frontier contains a complete node".to_owned())?;
            let poses = parent
                .state
                .placements
                .iter()
                .map(hazard_pose)
                .collect::<Vec<_>>();
            let mut index = JaguaHazardIndex::from_catalog_active(
                pieces,
                settings,
                TARGET_DEPTH_MM,
                &poses,
                &parent.state.active,
                hazard_catalog,
            )
            .map_err(|error| format!("persistent vacancy repair hazard index: {error}"))?;
            let transition_seed = repair_transition_seed(&parent.state, piece_index, pieces);
            let mut children = Vec::with_capacity(FINALISTS_PER_PIECE);
            let expansion_work_before = generation_work_snapshot(work.diagnostics);
            let expansion = expand_selected_piece(
                &parent.state,
                baseline,
                pieces,
                settings,
                &mut index,
                transition_seed,
                0,
                piece_index,
                expedition_events,
                work,
                &mut children,
            )?;
            expansion_diagnostics.push(GeneralPersistentVacancyRepairExpansionDiagnostics {
                parent_augmented_identity_hash: repair_node_hash(parent, pieces),
                parent_state_fingerprint: state_fingerprint(&parent.state, pieces),
                parent_queue_piece_ids: queue_piece_ids(&parent.queue, pieces),
                selected_piece_id: pieces[piece_index].id.to_owned(),
                transition_seed,
                angle_seed: expansion.selection.angle_seed,
                diversity_seed: expansion.selection.diversity_seed,
                proposal_order_hash: expansion.proposal_order_hash,
                exact_row_order_hash: expansion.exact_row_order_hash,
                generated_child_order_hash: expansion.generated_child_order_hash,
                work: work_delta(
                    generation_work_snapshot(work.diagnostics),
                    expansion_work_before,
                ),
            });
            for child in children {
                let queue =
                    repair_child_queue(parent, &child, piece_index, pieces, difficulty, mode)?;
                raw.push(RepairNode {
                    state: child,
                    queue,
                });
            }
        }
        let generated_children = raw.len();
        raw.sort_by(|first, second| compare_repair_nodes(first, second, pieces, difficulty));
        let before_dedup = raw.len();
        raw.dedup_by(|first, second| repair_node_identity(first) == repair_node_identity(second));
        let deduplicated_children = before_dedup.saturating_sub(raw.len());
        let before_transposition = raw.len();
        raw.retain(|node| seen.insert(repair_node_identity(node)));
        let transposed_children = before_transposition.saturating_sub(raw.len());

        let mut complete = Vec::new();
        let mut incomplete = Vec::with_capacity(raw.len());
        for node in raw {
            if node.queue.is_empty() {
                complete.push(node);
            } else {
                incomplete.push(node);
            }
        }
        expedition_events.complete_states = expedition_events
            .complete_states
            .saturating_add(complete.len());
        for node in &complete {
            if let Some((state, independent_depth)) =
                audit_complete_candidate(&node.state, pieces, settings, expedition_events, work)?
            {
                let fingerprint =
                    coupled_fast_placement_fingerprint(&fast_placements(&state, pieces, false));
                let replace =
                    best_complete
                        .as_ref()
                        .is_none_or(|(_, current_depth, current_fp)| {
                            independent_depth
                                .total_cmp(current_depth)
                                .then_with(|| fingerprint.cmp(current_fp))
                                .is_lt()
                        });
                if replace {
                    best_complete = Some((state, independent_depth, fingerprint));
                }
            }
        }
        incomplete.sort_by(|first, second| compare_repair_nodes(first, second, pieces, difficulty));
        incomplete.truncate(REPAIR_BEAM_WIDTH);
        if let Some(best) = incomplete.first() {
            audit_state(&best.state, pieces, settings, false, work)?;
            if compare_repair_nodes(best, &best_partial, pieces, difficulty).is_lt() {
                best_partial = best.clone();
            }
        }
        preflight_repair_memory(root, pieces, base_diagnostics, "depth diagnostics", work)?;
        let frontier_hash = repair_frontier_hash(&incomplete, pieces);
        let frontier_diagnostics = incomplete
            .iter()
            .map(|node| repair_node_diagnostics(node, pieces, difficulty))
            .collect::<Vec<_>>();
        repair
            .depths
            .push(GeneralPersistentVacancyRepairDepthDiagnostics {
                expansion_depth,
                expanded_parents,
                generated_children,
                deduplicated_children,
                transposed_children,
                complete_candidates: complete.len(),
                direct_insertions: expedition_events
                    .direct_insertions
                    .saturating_sub(direct_before),
                ejection_insertions: expedition_events
                    .ejection_insertions
                    .saturating_sub(ejection_before),
                expansions: expansion_diagnostics,
                frontier_hash,
                best_inactive_piece_count: incomplete
                    .first()
                    .map(|node| inactive_piece_count(&node.state)),
                best_inactive_area_grid2: incomplete
                    .first()
                    .map(|node| inactive_area(&node.state, difficulty).to_string()),
                frontier: frontier_diagnostics,
                work: work_delta(
                    generation_work_snapshot(work.diagnostics),
                    depth_work_before,
                ),
            });
        frontier = incomplete;
    }

    let accepted = if let Some((state, independent_depth, fingerprint)) = best_complete {
        repair.endpoint_state_fingerprint = Some(state_fingerprint(&state, pieces));
        repair.endpoint_inactive_piece_count = Some(0);
        repair.endpoint_inactive_area_grid2 = Some("0".to_owned());
        repair.endpoint_pareto_improves_root = true;
        repair.complete_endpoint = true;
        repair.independent_depth_mm = Some(independent_depth);
        repair.final_placement_fingerprint = Some(fingerprint);
        Some((state, independent_depth))
    } else {
        let endpoint_count = inactive_piece_count(&best_partial.state);
        let endpoint_area = inactive_area(&best_partial.state, difficulty);
        repair.endpoint_state_fingerprint = Some(state_fingerprint(&best_partial.state, pieces));
        repair.endpoint_inactive_piece_count = Some(endpoint_count);
        repair.endpoint_inactive_area_grid2 = Some(endpoint_area.to_string());
        repair.endpoint_pareto_improves_root = endpoint_count <= root_count
            && endpoint_area <= root_area
            && (endpoint_count < root_count || endpoint_area < root_area);
        None
    };
    repair.work = repair_work_delta(
        generation_work_snapshot(work.diagnostics),
        work_before,
        work.diagnostics,
    );
    Ok((accepted, repair))
}

fn difficulty_inactive_order(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Vec<usize> {
    let mut inactive = (0..state.active.len())
        .filter(|index| !state.active[*index])
        .collect::<Vec<_>>();
    inactive.sort_by(|first, second| {
        difficulty[*second]
            .expanded_area_grid2
            .cmp(&difficulty[*first].expanded_area_grid2)
            .then_with(|| {
                difficulty[*second]
                    .hull_deficit_grid2
                    .cmp(&difficulty[*first].hull_deficit_grid2)
            })
            .then_with(|| {
                difficulty[*second]
                    .minimum_side_grid
                    .cmp(&difficulty[*first].minimum_side_grid)
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    inactive
}

fn repair_child_queue(
    parent: &RepairNode,
    child: &VacancyState,
    inserted: usize,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
    mode: usize,
) -> Result<Vec<usize>, String> {
    let queue = if mode == REPAIR_CONTROL_MODE {
        difficulty_inactive_order(child, pieces, difficulty)
    } else {
        let transition = child
            .last_transition
            .as_ref()
            .ok_or_else(|| "repair child has no transition".to_owned())?;
        if transition.inserted != inserted || parent.queue.first().copied() != Some(inserted) {
            return Err("repair child transition does not match its queue head".to_owned());
        }
        let mut displaced = transition.ejected.clone();
        displaced.sort_by(|first, second| {
            difficulty[*second]
                .expanded_area_grid2
                .cmp(&difficulty[*first].expanded_area_grid2)
                .then_with(|| {
                    difficulty[*second]
                        .hull_deficit_grid2
                        .cmp(&difficulty[*first].hull_deficit_grid2)
                })
                .then_with(|| {
                    difficulty[*second]
                        .minimum_side_grid
                        .cmp(&difficulty[*first].minimum_side_grid)
                })
                .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
        });
        displaced.extend(
            parent
                .queue
                .iter()
                .copied()
                .filter(|index| *index != inserted),
        );
        displaced
    };
    validate_repair_queue(child, &queue)?;
    Ok(queue)
}

fn validate_repair_queue(state: &VacancyState, queue: &[usize]) -> Result<(), String> {
    let expected = state
        .active
        .iter()
        .enumerate()
        .filter_map(|(index, active)| (!*active).then_some(index))
        .collect::<BTreeSet<_>>();
    let actual = queue.iter().copied().collect::<BTreeSet<_>>();
    if actual.len() != queue.len() || actual != expected {
        return Err("repair queue does not contain every inactive piece exactly once".to_owned());
    }
    Ok(())
}

fn repair_node_identity(node: &RepairNode) -> RepairNodeIdentity {
    RepairNodeIdentity {
        state: state_identity(&node.state),
        queue: node.queue.clone(),
    }
}

fn compare_repair_nodes(
    first: &RepairNode,
    second: &RepairNode,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Ordering {
    compare_count_states(&first.state, &second.state, pieces, difficulty).then_with(|| {
        queue_piece_ids(&first.queue, pieces).cmp(&queue_piece_ids(&second.queue, pieces))
    })
}

fn repair_transition_seed(
    state: &VacancyState,
    piece_index: usize,
    pieces: &[GeneralFastPiece<'_>],
) -> u64 {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-repair-expedition-v1\0");
    digest.update(state_digest(state, pieces));
    update_framed_id(&mut digest, pieces[piece_index].id);
    let bytes: [u8; 32] = digest.finalize().into();
    let key = u64::from_be_bytes(bytes[..8].try_into().expect("SHA-256 has eight bytes"));
    derive_seed(REPAIR_SEED_DOMAIN ^ key, 0, piece_index)
}

fn queue_piece_ids(queue: &[usize], pieces: &[GeneralFastPiece<'_>]) -> Vec<String> {
    queue
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect()
}

fn repair_node_hash(node: &RepairNode, pieces: &[GeneralFastPiece<'_>]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-repair-node-v1\0");
    digest.update(state_digest(&node.state, pieces));
    digest.update((node.queue.len() as u32).to_be_bytes());
    for index in &node.queue {
        update_framed_id(&mut digest, pieces[*index].id);
    }
    format!("{:x}", digest.finalize())
}

fn repair_frontier_hash(frontier: &[RepairNode], pieces: &[GeneralFastPiece<'_>]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-repair-frontier-v1\0");
    digest.update((frontier.len() as u32).to_be_bytes());
    for node in frontier {
        let hash = repair_node_hash(node, pieces);
        digest.update((hash.len() as u32).to_be_bytes());
        digest.update(hash.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn repair_node_diagnostics(
    node: &RepairNode,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> GeneralPersistentVacancyRepairNodeDiagnostics {
    GeneralPersistentVacancyRepairNodeDiagnostics {
        augmented_identity_hash: repair_node_hash(node, pieces),
        state_fingerprint: state_fingerprint(&node.state, pieces),
        queue_piece_ids: queue_piece_ids(&node.queue, pieces),
        inactive_piece_count: inactive_piece_count(&node.state),
        inactive_area_grid2: inactive_area(&node.state, difficulty).to_string(),
    }
}

fn preflight_repair_memory(
    root: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    diagnostics: &GeneralPersistentVacancyDiagnostics,
    phase: &str,
    work: &mut RunWork,
) -> Result<(), String> {
    const RAW_NODE_CAPACITY: usize = REPAIR_BEAM_WIDTH * FINALISTS_PER_PIECE;
    const MAX_DIAGNOSTIC_NODES: usize = REPAIR_HORIZON * REPAIR_BEAM_WIDTH;
    const MAX_TRANSITION_IDENTITIES: usize = 1 + REPAIR_PARENT_EXPANSIONS * FINALISTS_PER_PIECE;
    const MAX_LIVE_NODE_OWNERS: usize = RAW_NODE_CAPACITY + REPAIR_BEAM_WIDTH + 4;
    const NODE_VECTOR_BACKING: usize = REPAIR_BEAM_WIDTH + 3 * RAW_NODE_CAPACITY;
    const HASH_BYTES: usize = 64;
    let max_piece_id_bytes = pieces.iter().map(|piece| piece.id.len()).max().unwrap_or(0);
    let maximum_collision_heap_bytes = size_of::<PolygonSet>()
        .saturating_add(MAX_COLLISION_VERTICES * size_of::<IrregularPoint>());
    let maximum_state_heap_bytes = state_heap_bytes(root)
        .saturating_add(inactive_piece_count(root).saturating_mul(maximum_collision_heap_bytes));
    let state_bytes = size_of::<VacancyState>().saturating_add(maximum_state_heap_bytes);
    let node_bytes = state_bytes
        .saturating_add(size_of::<Vec<usize>>())
        .saturating_add(MAX_INACTIVE_PIECES * size_of::<usize>());
    let identity_bytes = size_of::<RepairNodeIdentity>()
        .saturating_add(pieces.len() * size_of::<(usize, i64, bool, i64, i64)>())
        .saturating_add(pieces.len() * size_of::<usize>())
        .saturating_add(MAX_INACTIVE_PIECES * size_of::<usize>())
        .saturating_add(MAX_INACTIVE_PIECES * size_of::<usize>())
        .saturating_add(4 * size_of::<usize>());
    let diagnostic_node_bytes = size_of::<GeneralPersistentVacancyRepairNodeDiagnostics>()
        .saturating_add(2 * HASH_BYTES)
        .saturating_add(40)
        .saturating_add(MAX_INACTIVE_PIECES * (size_of::<String>() + max_piece_id_bytes));
    let expansion_diagnostic_bytes =
        size_of::<GeneralPersistentVacancyRepairExpansionDiagnostics>()
            .saturating_add(5 * HASH_BYTES)
            .saturating_add(max_piece_id_bytes)
            .saturating_add(MAX_INACTIVE_PIECES * (size_of::<String>() + max_piece_id_bytes));
    let comparator_queue_key_bytes =
        2usize.saturating_mul(MAX_INACTIVE_PIECES * (size_of::<String>() + max_piece_id_bytes));
    let repair_header_bytes = size_of::<GeneralPersistentVacancyRepairDiagnostics>()
        .saturating_add(32)
        .saturating_add(3 * HASH_BYTES)
        .saturating_add(2 * 40)
        .saturating_add(2 * (512 + max_piece_id_bytes))
        .saturating_add(MAX_INACTIVE_PIECES * (size_of::<String>() + max_piece_id_bytes));
    let depth_scalar_string_bytes = REPAIR_HORIZON * (HASH_BYTES + 40);
    let diagnostic_reserved = persistent_diagnostic_bytes(diagnostics)
        .saturating_add(MAX_DIAGNOSTIC_NODES * diagnostic_node_bytes)
        .saturating_add(REPAIR_PARENT_EXPANSIONS * expansion_diagnostic_bytes)
        .saturating_add(repair_header_bytes)
        .saturating_add(depth_scalar_string_bytes)
        .saturating_add(
            REPAIR_HORIZON * size_of::<GeneralPersistentVacancyRepairDepthDiagnostics>(),
        );
    let reserved = diagnostic_reserved
        .saturating_add(MAX_LIVE_NODE_OWNERS * node_bytes)
        .saturating_add(NODE_VECTOR_BACKING * size_of::<RepairNode>())
        .saturating_add(MAX_TRANSITION_IDENTITIES * identity_bytes)
        .saturating_add(comparator_queue_key_bytes);
    work.diagnostics.selector_diagnostic_peak_bytes = work
        .diagnostics
        .selector_diagnostic_peak_bytes
        .max(diagnostic_reserved);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(reserved);
    if reserved > MAX_RETAINED_BYTES {
        return Err(work.cap(&format!(
            "repair-expedition {phase} memory budget exhausted"
        )));
    }
    Ok(())
}

fn initial_vacancy_state(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<(VacancyState, Vec<PieceDifficulty>, Vec<usize>), String> {
    let mut collisions = Vec::with_capacity(pieces.len());
    let mut difficulty = Vec::with_capacity(pieces.len());
    for placement in &baseline.placements {
        let collision = build_collision(pieces[placement.input_index], placement, settings, work)?;
        difficulty.push(piece_difficulty(pieces[placement.input_index], &collision)?);
        collisions.push(Some(Arc::new(collision)));
    }
    let inset = collision_sheet_inset_mm(settings);
    let mut active = vec![true; pieces.len()];
    let mut inactive_order = Vec::new();
    for index in 0..pieces.len() {
        let collision = collisions[index]
            .as_ref()
            .ok_or_else(|| format!("missing initializer collision for piece {index}"))?;
        if !collision.fits_rect(
            inset,
            inset,
            settings.sheet_short_axis_mm - inset,
            TARGET_DEPTH_MM - inset,
        ) {
            let overflow = boundary_overflow_grid(collision, settings)?;
            if overflow <= 0 {
                return Err(format!(
                    "piece {} failed target containment without positive grid overflow",
                    pieces[index].id
                ));
            }
            active[index] = false;
            inactive_order.push((index, overflow));
        }
    }
    inactive_order.sort_by(|(first, first_overflow), (second, second_overflow)| {
        second_overflow
            .cmp(first_overflow)
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    let inactive_order = inactive_order
        .into_iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if inactive_order.is_empty() {
        return Err("target initializer removed no boundary offender".to_owned());
    }
    if inactive_order.len() > MAX_INACTIVE_PIECES
        || pieces.len().saturating_sub(inactive_order.len()) * 2 < pieces.len()
    {
        return Err(format!(
            "target initializer retained {} active and {} inactive pieces",
            pieces.len().saturating_sub(inactive_order.len()),
            inactive_order.len()
        ));
    }
    for index in &inactive_order {
        collisions[*index] = None;
    }
    let state = VacancyState {
        placements: baseline.placements,
        active,
        collisions,
        last_transition: None,
    };
    verify_exact_active_pairs(&state, work)?;
    diagnostics.direct_insertions = 0;
    Ok((state, difficulty, inactive_order))
}

#[allow(clippy::too_many_arguments)]
fn expand_parent(
    parent: &VacancyState,
    baseline: &[RelaxedPlacement],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    difficulty: &[PieceDifficulty],
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    layer: usize,
    mode: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
    selected_piece_ids: &mut BTreeSet<String>,
    parent_selections: &mut Vec<GeneralPersistentVacancyParentSelectionDiagnostics>,
    children: &mut Vec<VacancyState>,
) -> Result<(), String> {
    let poses = parent
        .placements
        .iter()
        .map(hazard_pose)
        .collect::<Vec<_>>();
    let mut index = JaguaHazardIndex::from_catalog_active(
        pieces,
        settings,
        TARGET_DEPTH_MM,
        &poses,
        &parent.active,
        hazard_catalog,
    )
    .map_err(|error| format!("persistent vacancy partial hazard index: {error}"))?;
    let parent_seed = parent_seed_key(parent, pieces);
    let transition_seed = derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ parent_seed, layer, 0);
    let selection = selected_inactive_pieces(parent, pieces, difficulty, layer, mode);
    let hardest_piece_id = selection
        .indices
        .first()
        .map(|index| pieces[*index].id.to_owned())
        .ok_or_else(|| "persistent vacancy parent has no inactive piece".to_owned())?;
    let coverage_piece_id = selection
        .rotation_start_index
        .and_then(|_| selection.indices.get(1))
        .map(|index| pieces[*index].id.to_owned());
    let stable_inactive = stable_inactive_order(parent, pieces);
    let mut selection_diagnostics = GeneralPersistentVacancyParentSelectionDiagnostics {
        parent_state_fingerprint: state_fingerprint(parent, pieces),
        inactive_order_hash: id_order_hash(&stable_inactive, pieces),
        scheduler_family: scheduler_family(mode).to_owned(),
        hardest_piece_id,
        rotation_start_index: selection.rotation_start_index,
        coverage_piece_id,
        transition_seed,
        slots: Vec::with_capacity(selection.indices.len()),
    };
    for (selected_ordinal, piece_index) in selection.indices.into_iter().enumerate() {
        selected_piece_ids.insert(pieces[piece_index].id.to_owned());
        let expansion = expand_selected_piece(
            parent,
            baseline,
            pieces,
            settings,
            &mut index,
            transition_seed,
            selected_ordinal,
            piece_index,
            diagnostics,
            work,
            children,
        )?;
        selection_diagnostics.slots.push(expansion.selection);
    }
    parent_selections.push(selection_diagnostics);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn expand_selected_piece(
    parent: &VacancyState,
    baseline: &[RelaxedPlacement],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    index: &mut JaguaHazardIndex,
    transition_seed: u64,
    selected_ordinal: usize,
    piece_index: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
    children: &mut Vec<VacancyState>,
) -> Result<SelectedPieceExpansion, String> {
    work.diagnostics.selected_piece_slots = work.diagnostics.selected_piece_slots.saturating_add(1);
    if work.diagnostics.selected_piece_slots > work.limits.selected_piece_slots {
        return Err(work.cap("selected-piece slot budget exhausted"));
    }
    work.charge_source_features(pieces[piece_index].polygon.vertex_count().saturating_mul(2))?;
    let angle_seed = derive_seed(
        transition_seed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN,
        selected_ordinal,
        piece_index,
    );
    let orientations =
        conflict_ruin_orientations(pieces[piece_index], &baseline[piece_index], angle_seed);
    let diversity_seed = derive_seed(
        transition_seed ^ CONFLICT_RUIN_DIVERSITY_SEED_DOMAIN,
        selected_ordinal,
        piece_index,
    );
    let mut merged = Vec::new();
    for (orientation_ordinal, (rotation_deg, mirrored)) in orientations.into_iter().enumerate() {
        work.diagnostics.orientation_streams =
            work.diagnostics.orientation_streams.saturating_add(1);
        if work.diagnostics.orientation_streams > work.limits.orientation_streams {
            return Err(work.cap("orientation-stream budget exhausted"));
        }
        let orientation = RelaxedPlacement {
            input_index: piece_index,
            rotation_deg,
            mirrored,
            translate_x: 0.0,
            translate_y: 0.0,
        };
        let local_collision = build_collision(pieces[piece_index], &orientation, settings, work)?;
        let position_seed = derive_seed(
            transition_seed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN,
            selected_ordinal
                .saturating_mul(ORIENTATIONS_PER_PIECE)
                .saturating_add(orientation_ordinal),
            piece_index,
        );
        let proposals = vacancy_positions(
            &baseline[piece_index],
            &orientation,
            &local_collision,
            parent,
            settings,
            position_seed,
            work,
        )?;
        let mut ranked = Vec::new();
        for placement in proposals {
            work.diagnostics.hazard_queries = work.diagnostics.hazard_queries.saturating_add(1);
            if work.diagnostics.hazard_queries > work.limits.hazard_queries {
                return Err(work.cap("hazard-query budget exhausted"));
            }
            let pose = hazard_pose(&placement);
            let query = match index.query_unplaced(piece_index, pose) {
                Ok(query) => query,
                Err(error) if error.to_string().contains("query envelope") => continue,
                Err(error) => return Err(format!("persistent vacancy hazard query: {error}")),
            };
            let GeneralHazardQuery::Complete {
                boundary,
                colliding_piece_ids,
            } = query
            else {
                return Err("persistent vacancy unplaced query unexpectedly pruned".to_owned());
            };
            if boundary {
                continue;
            }
            let mut proxy_loss = 0.0;
            for fixed_piece_id in colliding_piece_ids {
                if !parent.active[fixed_piece_id] {
                    return Err("inactive hazard leaked into vacancy query".to_owned());
                }
                work.diagnostics.proxy_pressure_visits =
                    work.diagnostics.proxy_pressure_visits.saturating_add(1);
                if work.diagnostics.proxy_pressure_visits > work.limits.proxy_pressure_visits {
                    return Err(work.cap("proxy-pressure visit budget exhausted"));
                }
                proxy_loss += index
                    .collision_pressure(piece_index, pose, fixed_piece_id)
                    .map_err(|error| format!("persistent vacancy pressure: {error}"))?;
            }
            ranked.push(RankedProposal {
                diversity_key: conflict_ruin_diversity_key(&placement, diversity_seed),
                placement,
                proxy_loss,
                orientation_ordinal,
            });
        }
        ranked.sort_by(compare_proposals);
        ranked.truncate(2);
        merged.extend(ranked);
    }
    merged.sort_by(compare_proposals);
    let proposal_order_hash =
        ranked_proposal_order_hash(b"persistent-vacancy-proposal-order-v1\0", &merged);
    let mut placement_keys = BTreeSet::new();
    merged.retain(|proposal| placement_keys.insert(placement_key(&proposal.placement)));
    merged.truncate(FINALISTS_PER_PIECE);
    let exact_row_order_hash =
        ranked_proposal_order_hash(b"persistent-vacancy-exact-row-order-v1\0", &merged);
    let child_start = children.len();
    for finalist in merged {
        work.diagnostics.exact_finalist_rows =
            work.diagnostics.exact_finalist_rows.saturating_add(1);
        if work.diagnostics.exact_finalist_rows > work.limits.exact_finalist_rows {
            return Err(work.cap("exact-finalist row budget exhausted"));
        }
        if let Some(child) = exact_vacancy_child(
            parent,
            pieces,
            piece_index,
            finalist.placement,
            settings,
            diagnostics,
            work,
        )? {
            children.push(child);
        }
    }
    Ok(SelectedPieceExpansion {
        selection: GeneralPersistentVacancySelectionSlotDiagnostics {
            selected_ordinal,
            piece_id: pieces[piece_index].id.to_owned(),
            angle_seed,
            diversity_seed,
        },
        proposal_order_hash,
        exact_row_order_hash,
        generated_child_order_hash: child_order_hash(&children[child_start..], pieces),
    })
}

fn ranked_proposal_order_hash(domain: &[u8], proposals: &[RankedProposal]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((proposals.len() as u32).to_be_bytes());
    for proposal in proposals {
        let (index, angle, mirrored, x, y) = placement_key(&proposal.placement);
        digest.update((index as u32).to_be_bytes());
        digest.update(angle.to_be_bytes());
        digest.update([u8::from(mirrored)]);
        digest.update(x.to_be_bytes());
        digest.update(y.to_be_bytes());
        digest.update((proposal.orientation_ordinal as u32).to_be_bytes());
        digest.update(proposal.diversity_key.to_be_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn compare_proposals(first: &RankedProposal, second: &RankedProposal) -> Ordering {
    first
        .proxy_loss
        .total_cmp(&second.proxy_loss)
        .then_with(|| first.orientation_ordinal.cmp(&second.orientation_ordinal))
        .then_with(|| first.diversity_key.cmp(&second.diversity_key))
        .then_with(|| placement_key(&first.placement).cmp(&placement_key(&second.placement)))
}

fn audit_complete_candidate(
    candidate: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<Option<(VacancyState, f64)>, String> {
    match audit_state(candidate, pieces, settings, true, work) {
        Ok(_) => {
            let placements = fast_placements(candidate, pieces, false);
            let independent = coupled_independent_source_depth(pieces, &placements, settings)
                .map_err(|error| format!("persistent vacancy final depth: {error}"))?;
            Ok(Some((candidate.clone(), independent)))
        }
        Err(reason) if !reason.starts_with("cap: ") => {
            diagnostics.publication_rejections =
                diagnostics.publication_rejections.saturating_add(1);
            Ok(None)
        }
        Err(reason) => Err(reason),
    }
}

fn audit_first_complete_candidate(
    candidates: &[VacancyState],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<Option<(VacancyState, f64)>, String> {
    for candidate in candidates {
        if let Some(accepted) =
            audit_complete_candidate(candidate, pieces, settings, diagnostics, work)?
        {
            return Ok(Some(accepted));
        }
    }
    Ok(None)
}

type AcceptedVacancyState = Option<(VacancyState, f64)>;

fn audit_macro_complete_candidates(
    candidates: &[VacancyState],
    ordinary_children: &[VacancyState],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<(AcceptedVacancyState, AcceptedVacancyState), String> {
    let mut first_combined = None;
    let mut first_ordinary = None;
    for candidate in candidates {
        let is_ordinary = ordinary_children
            .iter()
            .any(|ordinary| same_state_identity(candidate, ordinary));
        let Some(accepted) =
            audit_complete_candidate(candidate, pieces, settings, diagnostics, work)?
        else {
            continue;
        };
        if first_combined.is_none() {
            first_combined = Some(accepted.clone());
        }
        if is_ordinary && first_ordinary.is_none() {
            first_ordinary = Some(accepted);
        }
    }
    Ok((first_combined, first_ordinary))
}

fn exact_vacancy_child(
    parent: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    piece_index: usize,
    placement: RelaxedPlacement,
    settings: GeneralFastSettings,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<Option<VacancyState>, String> {
    let collision = Arc::new(build_collision(
        pieces[piece_index],
        &placement,
        settings,
        work,
    )?);
    let inset = collision_sheet_inset_mm(settings);
    if !collision.fits_rect(
        inset,
        inset,
        settings.sheet_short_axis_mm - inset,
        TARGET_DEPTH_MM - inset,
    ) {
        return Ok(None);
    }
    let mut blockers = Vec::new();
    for fixed_index in 0..pieces.len() {
        if !parent.active[fixed_index] {
            continue;
        }
        work.charge_experimental_pair()?;
        let fixed = parent.collisions[fixed_index]
            .as_ref()
            .ok_or_else(|| format!("active piece {fixed_index} has no collision"))?;
        if exact_intersection_area(&collision, fixed, work)? > 0.0 {
            blockers.push(fixed_index);
            if blockers.len() > 2 {
                return Ok(None);
            }
        }
    }
    blockers.sort_by(|first, second| pieces[*first].id.cmp(pieces[*second].id));
    if let Some(previous) = &parent.last_transition {
        if previous.ejected.contains(&piece_index) && blockers.contains(&previous.inserted) {
            diagnostics.immediate_reversals_rejected =
                diagnostics.immediate_reversals_rejected.saturating_add(1);
            return Ok(None);
        }
    }
    let inactive_before = parent.active.iter().filter(|active| !**active).count();
    let inactive_after = inactive_before
        .saturating_sub(1)
        .saturating_add(blockers.len());
    if inactive_after > MAX_INACTIVE_PIECES {
        return Ok(None);
    }
    let mut child = parent.clone();
    for blocker in &blockers {
        child.active[*blocker] = false;
        child.collisions[*blocker] = None;
    }
    child.placements[piece_index] = placement;
    child.active[piece_index] = true;
    child.collisions[piece_index] = Some(collision);
    child.last_transition = Some(VacancyTransition {
        inserted: piece_index,
        ejected: blockers.clone(),
    });
    if blockers.is_empty() {
        diagnostics.direct_insertions = diagnostics.direct_insertions.saturating_add(1);
    } else {
        diagnostics.ejection_insertions = diagnostics.ejection_insertions.saturating_add(1);
    }
    Ok(Some(child))
}

fn vacancy_positions(
    baseline: &RelaxedPlacement,
    orientation: &RelaxedPlacement,
    local_collision: &PolygonSet,
    parent: &VacancyState,
    settings: GeneralFastSettings,
    seed: u64,
    work: &mut RunWork,
) -> Result<Vec<RelaxedPlacement>, String> {
    let bounds = local_collision
        .bounds()
        .ok_or_else(|| "vacancy orientation has empty collision geometry".to_owned())?;
    let inset = collision_sheet_inset_mm(settings);
    let min_x = inset - bounds.min_x;
    let max_x = settings.sheet_short_axis_mm - inset - bounds.max_x;
    let min_y = inset - bounds.min_y;
    let max_y = TARGET_DEPTH_MM - inset - bounds.max_y;
    if min_x > max_x || min_y > max_y {
        return Ok(Vec::new());
    }
    let baseline_x = baseline.translate_x.clamp(min_x, max_x);
    let baseline_y = baseline.translate_y.clamp(min_y, max_y);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    let mut categories = vec![Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    categories[0].push((baseline_x, baseline_y));
    categories[1].extend([
        (min_x, min_y),
        (min_x, max_y),
        (max_x, min_y),
        (max_x, max_y),
        (min_x, center_y),
        (max_x, center_y),
        (center_x, min_y),
        (center_x, max_y),
    ]);
    for (fixed_index, fixed_collision) in parent.collisions.iter().enumerate() {
        if !parent.active[fixed_index] {
            continue;
        }
        let fixed_bounds = fixed_collision
            .as_ref()
            .and_then(|collision| collision.bounds())
            .ok_or_else(|| format!("active piece {fixed_index} has no collision bounds"))?;
        let left = (fixed_bounds.min_x - bounds.max_x).clamp(min_x, max_x);
        let right = (fixed_bounds.max_x - bounds.min_x).clamp(min_x, max_x);
        let below = (fixed_bounds.min_y - bounds.max_y).clamp(min_y, max_y);
        let above = (fixed_bounds.max_y - bounds.min_y).clamp(min_y, max_y);
        categories[2].extend([
            (left, baseline_y),
            (right, baseline_y),
            (baseline_x, below),
            (baseline_x, above),
            (left, below),
            (left, above),
            (right, below),
            (right, above),
        ]);
    }
    let width = (bounds.max_x - bounds.min_x).max(settings.total_padding_mm);
    let height = (bounds.max_y - bounds.min_y).max(settings.total_padding_mm);
    let mut focused_rng = SplitMix64::new(seed ^ 0xF0C5_5EED_0000_0001);
    for _ in 0..16 {
        categories[3].push((
            (baseline_x + focused_rng.range(-2.0 * width, 2.0 * width)).clamp(min_x, max_x),
            (baseline_y + focused_rng.range(-2.0 * height, 2.0 * height)).clamp(min_y, max_y),
        ));
    }
    let mut global_rng = SplitMix64::new(seed ^ 0x610B_A11E_0000_0001);
    for _ in 0..16 {
        categories[4].push((
            global_rng.range(min_x, max_x),
            global_rng.range(min_y, max_y),
        ));
    }
    work.charge_position_sources(categories.iter().map(Vec::len).sum())?;
    let mut category_indices = vec![0usize; categories.len()];
    let mut keys = BTreeSet::new();
    let mut placements = Vec::with_capacity(POSITIONS_PER_ORIENTATION);
    while placements.len() < POSITIONS_PER_ORIENTATION {
        let mut progressed = false;
        for category in 0..categories.len() {
            let Some((x, y)) = categories[category]
                .get(category_indices[category])
                .copied()
            else {
                continue;
            };
            category_indices[category] += 1;
            progressed = true;
            let placement = RelaxedPlacement {
                input_index: orientation.input_index,
                rotation_deg: orientation.rotation_deg,
                mirrored: orientation.mirrored,
                translate_x: snap_mm(x),
                translate_y: snap_mm(y),
            };
            if keys.insert(placement_key(&placement)) {
                placements.push(placement);
                if placements.len() == POSITIONS_PER_ORIENTATION {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    work.diagnostics.returned_positions = work
        .diagnostics
        .returned_positions
        .saturating_add(placements.len());
    if work.diagnostics.returned_positions > work.limits.returned_positions {
        return Err(work.cap("returned-position budget exhausted"));
    }
    Ok(placements)
}

fn retention_pool(
    mut ordinary: Vec<VacancyState>,
    mut carryovers: Vec<VacancyState>,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
    mode: usize,
) -> (Vec<VacancyState>, usize) {
    if mode != 5 {
        return (ordinary, 0);
    }
    ordinary.append(&mut carryovers);
    ordinary.sort_by(|first, second| compare_states(first, second, pieces, difficulty));
    let before_dedup = ordinary.len();
    ordinary.dedup_by(|first, second| same_state_identity(first, second));
    let deduplicated = before_dedup.saturating_sub(ordinary.len());
    (ordinary, deduplicated)
}

fn enforce_population_width(
    mode: usize,
    terminal_complete: bool,
    retained: usize,
    layer: usize,
) -> Result<(), String> {
    if !terminal_complete
        && matches!(
            mode,
            5 | 6 | MACRO_CONTROL_MODE | MACRO_TREATMENT_MODE | PRESERVED_BEST_MACRO_MODE
        )
        && retained != BEAM_WIDTH
    {
        return Err(format!(
            "persistent vacancy layer {layer} changed dual-objective width: expected {BEAM_WIDTH}, got {retained}"
        ));
    }
    Ok(())
}

fn retain_population(
    sorted: Vec<VacancyState>,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
    mode: usize,
) -> (Vec<VacancyState>, usize) {
    if matches!(mode, 1 | 3) || uses_macro_expansion(mode) {
        let retained = sorted.into_iter().take(BEAM_WIDTH).collect::<Vec<_>>();
        let signatures = retained
            .iter()
            .map(|state| contact_signature(state, pieces))
            .collect::<BTreeSet<_>>()
            .len();
        return (retained, signatures);
    }
    if matches!(mode, 5 | 6) {
        let mut retained = Vec::with_capacity(BEAM_WIDTH.min(sorted.len()));
        if let Some(area_elite) = sorted.first() {
            retained.push(area_elite.clone());
        }
        if let Some(count_elite) = sorted
            .iter()
            .min_by(|first, second| compare_count_states(first, second, pieces, difficulty))
        {
            if retained
                .iter()
                .all(|state| !same_state_identity(state, count_elite))
            {
                retained.push(count_elite.clone());
            }
        }
        for state in sorted {
            if retained
                .iter()
                .any(|selected| same_state_identity(selected, &state))
            {
                continue;
            }
            retained.push(state);
            if retained.len() == BEAM_WIDTH {
                break;
            }
        }
        let signatures = retained
            .iter()
            .map(|state| contact_signature(state, pieces))
            .collect::<BTreeSet<_>>()
            .len();
        return (retained, signatures);
    }
    let mut signatures = BTreeSet::new();
    let mut selected_indices = BTreeSet::new();
    let mut retained = Vec::new();
    for (index, state) in sorted.iter().enumerate() {
        let signature = contact_signature(state, pieces);
        if signatures.insert(signature) {
            selected_indices.insert(index);
            retained.push(state.clone());
            if retained.len() == BEAM_WIDTH {
                return (retained, signatures.len());
            }
        }
    }
    for (index, state) in sorted.into_iter().enumerate() {
        if selected_indices.contains(&index) {
            continue;
        }
        retained.push(state);
        if retained.len() == BEAM_WIDTH {
            break;
        }
    }
    (retained, signatures.len())
}

fn audit_state(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    complete: bool,
    work: &mut RunWork,
) -> Result<GeneralPlacementMetrics, String> {
    validate_state_structure(state, pieces.len())?;
    if complete != state.active.iter().all(|active| *active) {
        return Err("audit completeness does not match the active set".to_owned());
    }
    work.charge_validator_audit(complete)?;
    let active_pieces = pieces
        .iter()
        .enumerate()
        .filter(|(index, _)| state.active[*index])
        .map(|(_, piece)| *piece)
        .collect::<Vec<_>>();
    let placements = fast_placements(state, pieces, true);
    if active_pieces.len() != placements.len() {
        return Err("filtered audit piece and placement counts disagree".to_owned());
    }
    validate_and_measure_placements(&active_pieces, &placements, settings)
        .map_err(|error| format!("persistent vacancy dual audit: {error}"))
}

fn validate_state_structure(state: &VacancyState, piece_count: usize) -> Result<(), String> {
    if state.placements.len() != piece_count
        || state.active.len() != piece_count
        || state.collisions.len() != piece_count
    {
        return Err("vacancy state vectors do not match the piece count".to_owned());
    }
    let mut seen = vec![false; piece_count];
    for (slot, placement) in state.placements.iter().enumerate() {
        if placement.input_index >= piece_count
            || seen[placement.input_index]
            || placement.input_index != slot
        {
            return Err(
                "vacancy state has an unknown, duplicate, or misplaced stable ID".to_owned(),
            );
        }
        seen[placement.input_index] = true;
        if state.active[slot] != state.collisions[slot].is_some() {
            return Err("vacancy active bits and collision slots disagree".to_owned());
        }
    }
    if seen.iter().any(|present| !*present) {
        return Err("vacancy state is missing a stable ID".to_owned());
    }
    Ok(())
}

fn verify_exact_active_pairs(state: &VacancyState, work: &mut RunWork) -> Result<(), String> {
    for first in 0..state.active.len() {
        if !state.active[first] {
            continue;
        }
        for second in (first + 1)..state.active.len() {
            if !state.active[second] {
                continue;
            }
            work.charge_experimental_pair()?;
            let first_collision = state.collisions[first]
                .as_ref()
                .ok_or_else(|| format!("active piece {first} has no collision"))?;
            let second_collision = state.collisions[second]
                .as_ref()
                .ok_or_else(|| format!("active piece {second} has no collision"))?;
            if exact_intersection_area(first_collision, second_collision, work)? > 0.0 {
                return Err(format!(
                    "initializer active pieces {first} and {second} overlap"
                ));
            }
        }
    }
    Ok(())
}

fn build_collision(
    piece: GeneralFastPiece<'_>,
    placement: &RelaxedPlacement,
    settings: GeneralFastSettings,
    work: &mut RunWork,
) -> Result<PolygonSet, String> {
    work.diagnostics.experimental_collision_builds = work
        .diagnostics
        .experimental_collision_builds
        .saturating_add(1);
    if work.diagnostics.experimental_collision_builds > work.limits.experimental_collision_builds {
        return Err(work.cap("experimental collision-build budget exhausted"));
    }
    let collision = piece
        .polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_x,
            placement.translate_y,
        )
        .and_then(|polygon| polygon.offset(collision_expansion_mm(settings)))
        .map_err(|error| format!("persistent vacancy collision geometry: {error}"))?;
    if collision.vertex_count() > MAX_COLLISION_VERTICES {
        return Err(format!(
            "piece {} collision exceeds the {MAX_COLLISION_VERTICES}-vertex experiment cap",
            piece.id
        ));
    }
    work.diagnostics.transformed_collision_vertices = work
        .diagnostics
        .transformed_collision_vertices
        .saturating_add(collision.vertex_count());
    if work.diagnostics.transformed_collision_vertices > work.limits.transformed_collision_vertices
    {
        return Err(work.cap("transformed collision-vertex budget exhausted"));
    }
    Ok(collision)
}

fn exact_intersection_area(
    first: &PolygonSet,
    second: &PolygonSet,
    work: &mut RunWork,
) -> Result<f64, String> {
    if bounds_are_disjoint(first, second)? {
        return Ok(0.0);
    }
    let input_vertices = first.vertex_count().saturating_add(second.vertex_count());
    if work
        .diagnostics
        .clipper_input_vertices
        .saturating_add(input_vertices)
        > work.limits.clipper_input_vertices
    {
        return Err(work.cap("Clipper input-vertex budget exhausted"));
    }
    let result = first
        .intersection_area_with_complexity(second)
        .map_err(|error| format!("persistent vacancy exact intersection: {error}"))?;
    let next_output = work
        .diagnostics
        .clipper_output_vertices
        .saturating_add(result.output_vertices);
    if next_output > MAX_CLIPPER_OUTPUT_VERTICES {
        return Err(work.cap("Clipper output-vertex budget exhausted"));
    }
    work.diagnostics.clipper_input_vertices = work
        .diagnostics
        .clipper_input_vertices
        .saturating_add(result.input_vertices);
    work.diagnostics.clipper_output_vertices = next_output;
    Ok(result.area_mm2)
}

fn bounds_are_disjoint(first: &PolygonSet, second: &PolygonSet) -> Result<bool, String> {
    let first = first
        .bounds()
        .ok_or_else(|| "first exact polygon has no bounds".to_owned())?;
    let second = second
        .bounds()
        .ok_or_else(|| "second exact polygon has no bounds".to_owned())?;
    Ok(grid_key(first.max_x) <= grid_key(second.min_x)
        || grid_key(second.max_x) <= grid_key(first.min_x)
        || grid_key(first.max_y) <= grid_key(second.min_y)
        || grid_key(second.max_y) <= grid_key(first.min_y))
}

fn relaxed_state_from_diagnostics(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralCoupledSeparatorPlacementDiagnostics],
) -> Result<RelaxedState, String> {
    let by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut slots = vec![None; pieces.len()];
    for placement in placements {
        let index = *by_id
            .get(placement.piece_id.as_str())
            .ok_or_else(|| format!("unknown parent piece {}", placement.piece_id))?;
        if slots[index].is_some() {
            return Err(format!("duplicate parent piece {}", placement.piece_id));
        }
        slots[index] = Some(RelaxedPlacement {
            input_index: index,
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_x: placement.translate_short_axis,
            translate_y: placement.translate_long_axis,
        });
    }
    let placements = slots
        .into_iter()
        .enumerate()
        .map(|(index, placement)| {
            placement.ok_or_else(|| format!("parent is missing piece {}", pieces[index].id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RelaxedState {
        placements,
        strip_depth_mm: TARGET_DEPTH_MM,
    })
}

fn diagnostic_fast_placements(
    placements: &[GeneralCoupledSeparatorPlacementDiagnostics],
) -> Vec<GeneralFastPlacement> {
    placements
        .iter()
        .map(|placement| GeneralFastPlacement {
            piece_id: placement.piece_id.clone(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_short_axis,
            translate_long_axis: placement.translate_long_axis,
        })
        .collect()
}

fn fast_placements(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    active_only: bool,
) -> Vec<GeneralFastPlacement> {
    state
        .placements
        .iter()
        .filter(|placement| !active_only || state.active[placement.input_index])
        .map(|placement| GeneralFastPlacement {
            piece_id: pieces[placement.input_index].id.to_owned(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_x,
            translate_long_axis: placement.translate_y,
        })
        .collect()
}

fn piece_difficulty(
    piece: GeneralFastPiece<'_>,
    collision: &PolygonSet,
) -> Result<PieceDifficulty, String> {
    let bounds = collision
        .bounds()
        .ok_or_else(|| format!("piece {} collision has no bounds", piece.id))?;
    let points = collision
        .regions()
        .iter()
        .flat_map(|region| {
            std::iter::once(region.outer.points())
                .chain(region.holes.iter().map(|hole| hole.points()))
        })
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let hull = crate::geometry::convex::compute_convex_hull(&points);
    let hull_area = polygon_area_mm2(&hull.points);
    let expanded_area = collision.area_mm2();
    Ok(PieceDifficulty {
        expanded_area_grid2: doubled_area_grid2(expanded_area),
        hull_deficit_grid2: doubled_area_grid2((hull_area - expanded_area).max(0.0)),
        minimum_side_grid: grid_key((bounds.max_x - bounds.min_x).min(bounds.max_y - bounds.min_y)),
        material_area_grid2: doubled_area_grid2(piece.polygon.area_mm2()),
    })
}

fn polygon_area_mm2(points: &[IrregularPoint]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(first, second)| first.x * second.y - second.x * first.y)
        .sum::<f64>()
        .abs()
        / 2.0
}

fn doubled_area_grid2(area_mm2: f64) -> i128 {
    (area_mm2 * 2_000_000.0).round() as i128
}

fn boundary_overflow_grid(
    collision: &PolygonSet,
    settings: GeneralFastSettings,
) -> Result<i64, String> {
    let bounds = collision
        .bounds()
        .ok_or_else(|| "boundary overflow requires non-empty geometry".to_owned())?;
    let inset = collision_sheet_inset_mm(settings);
    let min_x = grid_key(inset);
    let min_y = grid_key(inset);
    let max_x = grid_key(settings.sheet_short_axis_mm - inset);
    let max_y = grid_key(TARGET_DEPTH_MM - inset);
    Ok([
        min_x.saturating_sub(grid_key(bounds.min_x)),
        min_y.saturating_sub(grid_key(bounds.min_y)),
        grid_key(bounds.max_x).saturating_sub(max_x),
        grid_key(bounds.max_y).saturating_sub(max_y),
    ]
    .into_iter()
    .max()
    .unwrap_or(0))
}

fn selected_inactive_pieces(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
    layer: usize,
    mode: usize,
) -> SelectedInactivePieces {
    let mut inactive = (0..state.active.len())
        .filter(|index| !state.active[*index])
        .collect::<Vec<_>>();
    inactive.sort_by(|first, second| {
        difficulty[*second]
            .expanded_area_grid2
            .cmp(&difficulty[*first].expanded_area_grid2)
            .then_with(|| {
                difficulty[*second]
                    .hull_deficit_grid2
                    .cmp(&difficulty[*first].hull_deficit_grid2)
            })
            .then_with(|| {
                difficulty[*second]
                    .minimum_side_grid
                    .cmp(&difficulty[*first].minimum_side_grid)
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    if !(matches!(mode, 3 | 4 | 5 | 6) || uses_macro_expansion(mode)) || inactive.len() <= 1 {
        inactive.truncate(SELECTED_PIECES_PER_PARENT);
        return SelectedInactivePieces {
            indices: inactive,
            rotation_start_index: None,
        };
    }
    let hardest = inactive[0];
    let stable = stable_inactive_order(state, pieces);
    let start = layer % stable.len();
    let coverage = (0..stable.len())
        .map(|offset| stable[(start + offset) % stable.len()])
        .find(|index| *index != hardest)
        .expect("more than one inactive piece has a non-hard coverage slot");
    SelectedInactivePieces {
        indices: vec![hardest, coverage],
        rotation_start_index: Some(start),
    }
}

fn stable_inactive_order(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> Vec<usize> {
    let mut inactive = (0..state.active.len())
        .filter(|index| !state.active[*index])
        .collect::<Vec<_>>();
    inactive.sort_by(|first, second| pieces[*first].id.cmp(pieces[*second].id));
    inactive
}

fn scheduler_family(mode: usize) -> &'static str {
    if matches!(mode, 3 | 4 | 5 | 6) || uses_macro_expansion(mode) {
        "hardPlusStatelessRotation"
    } else {
        "twoHardest"
    }
}

fn compare_states(
    first: &VacancyState,
    second: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Ordering {
    inactive_area(first, difficulty)
        .cmp(&inactive_area(second, difficulty))
        .then_with(|| {
            inactive_difficulty_sequence(first, pieces, difficulty)
                .cmp(&inactive_difficulty_sequence(second, pieces, difficulty))
        })
        .then_with(|| {
            first
                .active
                .iter()
                .filter(|active| !**active)
                .count()
                .cmp(&second.active.iter().filter(|active| !**active).count())
        })
        .then_with(|| {
            ejected_material_area(first, difficulty).cmp(&ejected_material_area(second, difficulty))
        })
        .then_with(|| {
            first
                .last_transition
                .as_ref()
                .map_or(0, |transition| transition.ejected.len())
                .cmp(
                    &second
                        .last_transition
                        .as_ref()
                        .map_or(0, |transition| transition.ejected.len()),
                )
        })
        .then_with(|| {
            active_frontier_grid(first, pieces).cmp(&active_frontier_grid(second, pieces))
        })
        .then_with(|| compare_state_identity(first, second))
}

fn compare_count_states(
    first: &VacancyState,
    second: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Ordering {
    inactive_piece_count(first)
        .cmp(&inactive_piece_count(second))
        .then_with(|| inactive_area(first, difficulty).cmp(&inactive_area(second, difficulty)))
        .then_with(|| {
            inactive_difficulty_sequence(first, pieces, difficulty)
                .cmp(&inactive_difficulty_sequence(second, pieces, difficulty))
        })
        .then_with(|| {
            ejected_material_area(first, difficulty).cmp(&ejected_material_area(second, difficulty))
        })
        .then_with(|| ejected_piece_count(first).cmp(&ejected_piece_count(second)))
        .then_with(|| {
            active_frontier_grid(first, pieces).cmp(&active_frontier_grid(second, pieces))
        })
        .then_with(|| compare_state_identity(first, second))
}

fn inactive_piece_count(state: &VacancyState) -> usize {
    state.active.iter().filter(|active| !**active).count()
}

fn ejected_piece_count(state: &VacancyState) -> usize {
    state
        .last_transition
        .as_ref()
        .map_or(0, |transition| transition.ejected.len())
}

fn inactive_area(state: &VacancyState, difficulty: &[PieceDifficulty]) -> i128 {
    state
        .active
        .iter()
        .enumerate()
        .filter(|(_, active)| !**active)
        .map(|(index, _)| difficulty[index].expanded_area_grid2)
        .sum()
}

fn inactive_difficulty_sequence(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Vec<(i128, i128, i64, String)> {
    let mut inactive = (0..state.active.len())
        .filter(|index| !state.active[*index])
        .collect::<Vec<_>>();
    inactive.sort_by(|first, second| {
        difficulty[*second]
            .expanded_area_grid2
            .cmp(&difficulty[*first].expanded_area_grid2)
            .then_with(|| {
                difficulty[*second]
                    .hull_deficit_grid2
                    .cmp(&difficulty[*first].hull_deficit_grid2)
            })
            .then_with(|| {
                difficulty[*second]
                    .minimum_side_grid
                    .cmp(&difficulty[*first].minimum_side_grid)
            })
            .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
    });
    inactive
        .into_iter()
        .map(|index| {
            (
                difficulty[index].expanded_area_grid2,
                difficulty[index].hull_deficit_grid2,
                difficulty[index].minimum_side_grid,
                pieces[index].id.to_owned(),
            )
        })
        .collect()
}

fn ejected_material_area(state: &VacancyState, difficulty: &[PieceDifficulty]) -> i128 {
    state.last_transition.as_ref().map_or(0, |transition| {
        transition
            .ejected
            .iter()
            .map(|index| difficulty[*index].material_area_grid2)
            .sum()
    })
}

fn active_frontier_grid(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> i64 {
    state
        .placements
        .iter()
        .filter(|placement| state.active[placement.input_index])
        .filter_map(|placement| {
            pieces[placement.input_index]
                .polygon
                .transformed(
                    placement.rotation_deg,
                    placement.mirrored,
                    placement.translate_x,
                    placement.translate_y,
                )
                .ok()
                .and_then(|polygon| polygon.bounds())
                .map(|bounds| grid_key(bounds.max_y))
        })
        .max()
        .unwrap_or(i64::MIN)
}

fn state_identity(state: &VacancyState) -> VacancyStateIdentity {
    VacancyStateIdentity {
        active_placements: state
            .placements
            .iter()
            .filter(|placement| state.active[placement.input_index])
            .map(placement_key)
            .collect(),
        inactive: (0..state.active.len())
            .filter(|index| !state.active[*index])
            .collect(),
        last_transition: state.last_transition.clone(),
    }
}

fn compare_state_identity(first: &VacancyState, second: &VacancyState) -> Ordering {
    first
        .placements
        .iter()
        .filter(|placement| first.active[placement.input_index])
        .map(placement_key)
        .cmp(
            second
                .placements
                .iter()
                .filter(|placement| second.active[placement.input_index])
                .map(placement_key),
        )
        .then_with(|| {
            first
                .active
                .iter()
                .enumerate()
                .filter(|(_, active)| !**active)
                .map(|(index, _)| index)
                .cmp(
                    second
                        .active
                        .iter()
                        .enumerate()
                        .filter(|(_, active)| !**active)
                        .map(|(index, _)| index),
                )
        })
        .then_with(|| first.last_transition.cmp(&second.last_transition))
}

fn same_state_identity(first: &VacancyState, second: &VacancyState) -> bool {
    compare_state_identity(first, second).is_eq()
}

fn state_fingerprint(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> String {
    state_digest(state, pieces)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn population_hash(population: &[VacancyState], pieces: &[GeneralFastPiece<'_>]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-population-v1\0");
    digest.update((population.len() as u32).to_be_bytes());
    for state in population {
        digest.update(state_digest(state, pieces));
    }
    format!("{:x}", digest.finalize())
}

fn child_order_hash(children: &[VacancyState], pieces: &[GeneralFastPiece<'_>]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-child-order-v1\0");
    digest.update((children.len() as u32).to_be_bytes());
    for state in children {
        digest.update([u8::from(state.active.iter().all(|active| *active))]);
        digest.update(state_digest(state, pieces));
    }
    format!("{:x}", digest.finalize())
}

fn novel_macro_child_fingerprints(
    ordinary_children: &[VacancyState],
    macro_children: &[VacancyState],
    pieces: &[GeneralFastPiece<'_>],
) -> Vec<String> {
    macro_children
        .iter()
        .filter(|macro_child| {
            ordinary_children
                .iter()
                .all(|ordinary_child| !same_state_identity(macro_child, ordinary_child))
        })
        .map(|state| state_fingerprint(state, pieces))
        .collect()
}

fn select_macro_retention_children(
    ordinary_children: Vec<VacancyState>,
    combined_children: Option<Vec<VacancyState>>,
    mode: usize,
) -> Vec<VacancyState> {
    if admits_macro_children(mode) {
        combined_children.unwrap_or(ordinary_children)
    } else {
        ordinary_children
    }
}

fn population_elites<'a>(
    population: &'a [VacancyState],
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> (&'a VacancyState, &'a VacancyState) {
    let area = population
        .iter()
        .min_by(|first, second| compare_states(first, second, pieces, difficulty))
        .expect("an elite population is non-empty");
    let count = population
        .iter()
        .min_by(|first, second| compare_count_states(first, second, pieces, difficulty))
        .expect("an elite population is non-empty");
    (area, count)
}

fn distinct_elite_states(
    population: &[VacancyState],
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> Vec<VacancyState> {
    let (area, count) = population_elites(population, pieces, difficulty);
    let mut elites = vec![area.clone()];
    if !same_state_identity(area, count) {
        elites.push(count.clone());
    }
    elites
}

fn elite_snapshot(
    state: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    difficulty: &[PieceDifficulty],
) -> EliteSnapshot {
    EliteSnapshot {
        fingerprint: state_fingerprint(state, pieces),
        inactive_piece_count: inactive_piece_count(state),
        inactive_area_grid2: inactive_area(state, difficulty),
        inactive_difficulty_sequence: inactive_difficulty_sequence(state, pieces, difficulty),
        ejected_material_area_grid2: ejected_material_area(state, difficulty),
        ejected_piece_count: ejected_piece_count(state),
        active_frontier_grid: active_frontier_grid(state, pieces),
        identity: state_identity(state),
    }
}

fn compare_area_snapshots(first: &EliteSnapshot, second: &EliteSnapshot) -> Ordering {
    first
        .inactive_area_grid2
        .cmp(&second.inactive_area_grid2)
        .then_with(|| {
            first
                .inactive_difficulty_sequence
                .cmp(&second.inactive_difficulty_sequence)
        })
        .then_with(|| first.inactive_piece_count.cmp(&second.inactive_piece_count))
        .then_with(|| {
            first
                .ejected_material_area_grid2
                .cmp(&second.ejected_material_area_grid2)
        })
        .then_with(|| first.ejected_piece_count.cmp(&second.ejected_piece_count))
        .then_with(|| first.active_frontier_grid.cmp(&second.active_frontier_grid))
        .then_with(|| first.identity.cmp(&second.identity))
}

fn compare_count_snapshots(first: &EliteSnapshot, second: &EliteSnapshot) -> Ordering {
    first
        .inactive_piece_count
        .cmp(&second.inactive_piece_count)
        .then_with(|| first.inactive_area_grid2.cmp(&second.inactive_area_grid2))
        .then_with(|| {
            first
                .inactive_difficulty_sequence
                .cmp(&second.inactive_difficulty_sequence)
        })
        .then_with(|| {
            first
                .ejected_material_area_grid2
                .cmp(&second.ejected_material_area_grid2)
        })
        .then_with(|| first.ejected_piece_count.cmp(&second.ejected_piece_count))
        .then_with(|| first.active_frontier_grid.cmp(&second.active_frontier_grid))
        .then_with(|| first.identity.cmp(&second.identity))
}

fn update_best_area(best: &mut Option<EliteSnapshot>, candidate: &EliteSnapshot) {
    if best.as_ref().map_or(true, |current| {
        compare_area_snapshots(candidate, current).is_lt()
    }) {
        *best = Some(candidate.clone());
    }
}

fn update_best_count(best: &mut Option<EliteSnapshot>, candidate: &EliteSnapshot) {
    if best.as_ref().map_or(true, |current| {
        compare_count_snapshots(candidate, current).is_lt()
    }) {
        *best = Some(candidate.clone());
    }
}

fn parent_seed_key(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> u64 {
    let digest = state_digest(state, pieces);
    u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 has eight bytes"))
}

fn state_digest(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-state-v1\0");
    let active_placement_count = state
        .placements
        .iter()
        .filter(|placement| state.active[placement.input_index])
        .count();
    digest.update((active_placement_count as u32).to_be_bytes());
    for (index, angle, mirrored, x, y) in state
        .placements
        .iter()
        .filter(|placement| state.active[placement.input_index])
        .map(placement_key)
    {
        update_framed_id(&mut digest, pieces[index].id);
        digest.update(angle.to_be_bytes());
        digest.update([u8::from(mirrored)]);
        digest.update(x.to_be_bytes());
        digest.update(y.to_be_bytes());
    }
    let inactive_count = inactive_piece_count(state);
    digest.update((inactive_count as u32).to_be_bytes());
    for index in (0..state.active.len()).filter(|index| !state.active[*index]) {
        update_framed_id(&mut digest, pieces[index].id);
    }
    match &state.last_transition {
        None => digest.update([0]),
        Some(transition) => {
            digest.update([1]);
            update_framed_id(&mut digest, pieces[transition.inserted].id);
            digest.update((transition.ejected.len() as u32).to_be_bytes());
            for index in &transition.ejected {
                update_framed_id(&mut digest, pieces[*index].id);
            }
        }
    }
    digest.finalize().into()
}

fn update_framed_id(digest: &mut Sha256, id: &str) {
    digest.update((id.len() as u32).to_be_bytes());
    digest.update(id.as_bytes());
}

fn active_ids(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> Vec<String> {
    (0..state.active.len())
        .filter(|index| state.active[*index])
        .map(|index| pieces[index].id.to_owned())
        .collect()
}

fn id_order_hash(indices: &[usize], pieces: &[GeneralFastPiece<'_>]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-inactive-order-v1\0");
    digest.update((indices.len() as u32).to_be_bytes());
    for index in indices {
        update_framed_id(&mut digest, pieces[*index].id);
    }
    format!("{:x}", digest.finalize())
}

fn contact_signature(state: &VacancyState, pieces: &[GeneralFastPiece<'_>]) -> ContactSignature {
    let active_ids = active_ids(state, pieces);
    let mut edges = Vec::new();
    for first in 0..state.active.len() {
        if !state.active[first] {
            continue;
        }
        for second in (first + 1)..state.active.len() {
            if !state.active[second] {
                continue;
            }
            let Some(first_bounds) = state.collisions[first]
                .as_ref()
                .and_then(|collision| collision.bounds())
            else {
                continue;
            };
            let Some(second_bounds) = state.collisions[second]
                .as_ref()
                .and_then(|collision| collision.bounds())
            else {
                continue;
            };
            let x_contact = (grid_key(first_bounds.max_x) == grid_key(second_bounds.min_x)
                || grid_key(second_bounds.max_x) == grid_key(first_bounds.min_x))
                && grid_key(first_bounds.max_y).min(grid_key(second_bounds.max_y))
                    > grid_key(first_bounds.min_y).max(grid_key(second_bounds.min_y));
            let y_contact = (grid_key(first_bounds.max_y) == grid_key(second_bounds.min_y)
                || grid_key(second_bounds.max_y) == grid_key(first_bounds.min_y))
                && grid_key(first_bounds.max_x).min(grid_key(second_bounds.max_x))
                    > grid_key(first_bounds.min_x).max(grid_key(second_bounds.min_x));
            let axis = match (x_contact, y_contact) {
                (true, false) => Some(0),
                (false, true) => Some(1),
                _ => None,
            };
            if let Some(axis) = axis {
                let (first_id, second_id) = if pieces[first].id <= pieces[second].id {
                    (pieces[first].id, pieces[second].id)
                } else {
                    (pieces[second].id, pieces[first].id)
                };
                edges.push(ContactEdge {
                    first_id: first_id.to_owned(),
                    second_id: second_id.to_owned(),
                    axis,
                });
            }
        }
    }
    edges.sort();
    ContactSignature { active_ids, edges }
}

#[cfg(test)]
fn contact_signature_hash(signature: &ContactSignature) -> String {
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-contact-v1\0");
    digest.update((signature.active_ids.len() as u32).to_be_bytes());
    for id in &signature.active_ids {
        update_framed_id(&mut digest, id);
    }
    digest.update((signature.edges.len() as u32).to_be_bytes());
    for edge in &signature.edges {
        update_framed_id(&mut digest, &edge.first_id);
        update_framed_id(&mut digest, &edge.second_id);
        digest.update([edge.axis]);
    }
    format!("{:x}", digest.finalize())
}

fn charge_retained_memory(
    population: &[VacancyState],
    preserved_best: Option<&VacancyState>,
    complete_sidecar_bytes: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    pending_layer: &GeneralPersistentVacancyLayerDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    diagnostics.layers.reserve(1);
    let legacy_state_bytes = legacy_state_slice_bytes(population)
        .saturating_add(preserved_best.map_or(0, legacy_state_heap_bytes));
    let state_bytes = state_slice_bytes(population)
        .saturating_add(population.len().saturating_mul(size_of::<VacancyState>()))
        .saturating_add(complete_sidecar_bytes);
    let diagnostic_bytes = persistent_diagnostic_bytes(diagnostics)
        .saturating_add(layer_diagnostic_heap_bytes(pending_layer));
    let total_bytes = state_bytes.saturating_add(diagnostic_bytes);
    work.diagnostics.retained_peak_bytes =
        work.diagnostics.retained_peak_bytes.max(legacy_state_bytes);
    work.diagnostics.selector_diagnostic_peak_bytes = work
        .diagnostics
        .selector_diagnostic_peak_bytes
        .max(diagnostic_bytes);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(total_bytes);
    if total_bytes > MAX_RETAINED_BYTES {
        return Err(work.cap("retained-memory budget exhausted"));
    }
    Ok(())
}

fn preflight_live_memory(
    entering_population: &Vec<VacancyState>,
    preserved_best_live_state_bytes: usize,
    ordinary_live_state_bytes: usize,
    carryover_live_state_bytes: usize,
    retained_clone_bytes: usize,
    combined_pool_backing_bytes: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    pending_layer: &GeneralPersistentVacancyLayerDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    diagnostics.layers.reserve(1);
    let diagnostic_bytes = persistent_diagnostic_bytes(diagnostics)
        .saturating_add(layer_diagnostic_heap_bytes(pending_layer));
    let total_bytes = state_vec_bytes(entering_population)
        .saturating_add(preserved_best_live_state_bytes)
        .saturating_add(ordinary_live_state_bytes)
        .saturating_add(carryover_live_state_bytes)
        .saturating_add(retained_clone_bytes)
        .saturating_add(combined_pool_backing_bytes)
        .saturating_add(diagnostic_bytes);
    work.diagnostics.selector_diagnostic_peak_bytes = work
        .diagnostics
        .selector_diagnostic_peak_bytes
        .max(diagnostic_bytes);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(total_bytes);
    if total_bytes > MAX_RETAINED_BYTES {
        return Err(work.cap("live-pool memory budget exhausted"));
    }
    Ok(())
}

fn preflight_raw_live_memory(
    entering_population: &Vec<VacancyState>,
    preserved_best_live_state_bytes: usize,
    ordinary_live_state_bytes: usize,
    carryover_live_state_bytes: usize,
    retained_clone_bytes: usize,
    combined_pool_backing_bytes: usize,
    selected_piece_ids: &[String],
    parent_selections: &[GeneralPersistentVacancyParentSelectionDiagnostics],
    pending_auxiliary_diagnostic_bytes: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    const ELITE_DIAGNOSTIC_HEAP_UPPER_BOUND: usize = 8 * 1024;

    diagnostics.layers.reserve(1);
    let pending_selector_bytes = selected_piece_ids
        .len()
        .saturating_mul(size_of::<String>())
        .saturating_add(
            selected_piece_ids
                .iter()
                .map(String::capacity)
                .sum::<usize>(),
        )
        .saturating_add(
            parent_selections
                .len()
                .saturating_mul(size_of::<GeneralPersistentVacancyParentSelectionDiagnostics>()),
        )
        .saturating_add(
            parent_selections
                .iter()
                .map(parent_selection_heap_bytes)
                .sum::<usize>(),
        )
        .saturating_add(pending_auxiliary_diagnostic_bytes)
        .saturating_add(ELITE_DIAGNOSTIC_HEAP_UPPER_BOUND);
    let diagnostic_bytes =
        persistent_diagnostic_bytes(diagnostics).saturating_add(pending_selector_bytes);
    let total_bytes = state_vec_bytes(entering_population)
        .saturating_add(preserved_best_live_state_bytes)
        .saturating_add(ordinary_live_state_bytes)
        .saturating_add(carryover_live_state_bytes)
        .saturating_add(retained_clone_bytes)
        .saturating_add(combined_pool_backing_bytes)
        .saturating_add(diagnostic_bytes);
    work.diagnostics.selector_diagnostic_peak_bytes = work
        .diagnostics
        .selector_diagnostic_peak_bytes
        .max(diagnostic_bytes);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(total_bytes);
    if total_bytes > MAX_RETAINED_BYTES {
        return Err(work.cap("pre-deduplication live-pool memory budget exhausted"));
    }
    Ok(())
}

fn pending_selection_diagnostic_bytes(
    selected_piece_ids: &Vec<String>,
    parent_selections: &Vec<GeneralPersistentVacancyParentSelectionDiagnostics>,
) -> usize {
    string_vec_bytes(selected_piece_ids, selected_piece_ids.capacity())
        .saturating_add(
            parent_selections
                .capacity()
                .saturating_mul(size_of::<GeneralPersistentVacancyParentSelectionDiagnostics>()),
        )
        .saturating_add(
            parent_selections
                .iter()
                .map(parent_selection_heap_bytes)
                .sum::<usize>(),
        )
}

fn state_vec_bytes(states: &Vec<VacancyState>) -> usize {
    states
        .capacity()
        .saturating_mul(size_of::<VacancyState>())
        .saturating_add(state_slice_bytes(states))
}

fn owned_state_bytes(state: Option<&VacancyState>) -> usize {
    state.map_or(0, |state| {
        size_of::<VacancyState>().saturating_add(state_heap_bytes(state))
    })
}

fn sidecar_bytes(area: Option<&VacancyState>, count: Option<&VacancyState>) -> usize {
    owned_state_bytes(area).saturating_add(owned_state_bytes(count))
}

fn state_slice_bytes(states: &[VacancyState]) -> usize {
    states.iter().map(state_heap_bytes).sum()
}

fn legacy_state_slice_bytes(states: &[VacancyState]) -> usize {
    states.iter().map(legacy_state_heap_bytes).sum()
}

fn legacy_state_heap_bytes(state: &VacancyState) -> usize {
    state.placements.capacity() * size_of::<RelaxedPlacement>()
        + state.active.capacity() * size_of::<bool>()
        + state.collisions.capacity() * size_of::<Option<Arc<PolygonSet>>>()
        + state
            .collisions
            .iter()
            .filter_map(Option::as_ref)
            .map(|collision| {
                collision.vertex_count() * size_of::<IrregularPoint>() + size_of::<PolygonSet>()
            })
            .sum::<usize>()
}

fn state_heap_bytes(state: &VacancyState) -> usize {
    legacy_state_heap_bytes(state)
        + state.last_transition.as_ref().map_or(0, |transition| {
            transition.ejected.capacity() * size_of::<usize>()
        })
}

fn generation_work_snapshot(
    mut diagnostics: GeneralPersistentVacancyWorkDiagnostics,
) -> GeneralPersistentVacancyWorkDiagnostics {
    diagnostics.retained_peak_bytes = 0;
    diagnostics.selector_diagnostic_peak_bytes = 0;
    diagnostics.total_retained_peak_bytes = 0;
    diagnostics
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreExpeditionBehaviorRecord<'a> {
    seed_domain: u64,
    target_depth_mm: f64,
    parent_fingerprint: &'a Option<String>,
    initial_state_fingerprint: &'a Option<String>,
    initial_active_piece_ids: &'a [String],
    initial_inactive_piece_ids: &'a [String],
    initial_inactive_order_hash: &'a Option<String>,
    layers_completed: usize,
    direct_insertions: usize,
    ejection_insertions: usize,
    immediate_reversals_rejected: usize,
    deduplicated_states: usize,
    distinct_signatures_retained: usize,
    complete_states: usize,
    publication_rejections: usize,
    pre_expedition_work: GeneralPersistentVacancyWorkDiagnostics,
    layers: &'a [GeneralPersistentVacancyLayerDiagnostics],
}

fn pre_expedition_behavior_hash(
    diagnostics: &GeneralPersistentVacancyDiagnostics,
) -> Result<String, String> {
    let pre_expedition_work = diagnostics
        .pre_expedition_work
        .ok_or_else(|| "pre-expedition work snapshot is missing".to_owned())?;
    let record = PreExpeditionBehaviorRecord {
        seed_domain: diagnostics.seed_domain,
        target_depth_mm: diagnostics.target_depth_mm,
        parent_fingerprint: &diagnostics.parent_fingerprint,
        initial_state_fingerprint: &diagnostics.initial_state_fingerprint,
        initial_active_piece_ids: &diagnostics.initial_active_piece_ids,
        initial_inactive_piece_ids: &diagnostics.initial_inactive_piece_ids,
        initial_inactive_order_hash: &diagnostics.initial_inactive_order_hash,
        layers_completed: diagnostics.layers_completed,
        direct_insertions: diagnostics.direct_insertions,
        ejection_insertions: diagnostics.ejection_insertions,
        immediate_reversals_rejected: diagnostics.immediate_reversals_rejected,
        deduplicated_states: diagnostics.deduplicated_states,
        distinct_signatures_retained: diagnostics.distinct_signatures_retained,
        complete_states: diagnostics.complete_states,
        publication_rejections: diagnostics.publication_rejections,
        pre_expedition_work,
        layers: &diagnostics.layers,
    };
    let json = serde_json::to_vec(&record)
        .map_err(|error| format!("pre-expedition behavior serialization: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(b"persistent-vacancy-pre-expedition-v1\0");
    digest.update(json);
    Ok(format!("{:x}", digest.finalize()))
}

fn work_delta(
    after: GeneralPersistentVacancyWorkDiagnostics,
    before: GeneralPersistentVacancyWorkDiagnostics,
) -> GeneralPersistentVacancyWorkDiagnostics {
    GeneralPersistentVacancyWorkDiagnostics {
        selected_piece_slots: after
            .selected_piece_slots
            .saturating_sub(before.selected_piece_slots),
        orientation_streams: after
            .orientation_streams
            .saturating_sub(before.orientation_streams),
        source_feature_visits: after
            .source_feature_visits
            .saturating_sub(before.source_feature_visits),
        position_source_attempts: after
            .position_source_attempts
            .saturating_sub(before.position_source_attempts),
        returned_positions: after
            .returned_positions
            .saturating_sub(before.returned_positions),
        hazard_queries: after.hazard_queries.saturating_sub(before.hazard_queries),
        proxy_pressure_visits: after
            .proxy_pressure_visits
            .saturating_sub(before.proxy_pressure_visits),
        exact_finalist_rows: after
            .exact_finalist_rows
            .saturating_sub(before.exact_finalist_rows),
        experimental_collision_builds: after
            .experimental_collision_builds
            .saturating_sub(before.experimental_collision_builds),
        validator_collision_builds: after
            .validator_collision_builds
            .saturating_sub(before.validator_collision_builds),
        experimental_pair_visits: after
            .experimental_pair_visits
            .saturating_sub(before.experimental_pair_visits),
        validator_pair_visits: after
            .validator_pair_visits
            .saturating_sub(before.validator_pair_visits),
        transformed_collision_vertices: after
            .transformed_collision_vertices
            .saturating_sub(before.transformed_collision_vertices),
        clipper_input_vertices: after
            .clipper_input_vertices
            .saturating_sub(before.clipper_input_vertices),
        clipper_output_vertices: after
            .clipper_output_vertices
            .saturating_sub(before.clipper_output_vertices),
        partial_audits: after.partial_audits.saturating_sub(before.partial_audits),
        complete_audits: after.complete_audits.saturating_sub(before.complete_audits),
        retained_peak_bytes: 0,
        selector_diagnostic_peak_bytes: 0,
        total_retained_peak_bytes: 0,
    }
}

fn repair_work_delta(
    after: GeneralPersistentVacancyWorkDiagnostics,
    before: GeneralPersistentVacancyWorkDiagnostics,
    peaks: GeneralPersistentVacancyWorkDiagnostics,
) -> GeneralPersistentVacancyWorkDiagnostics {
    let mut delta = work_delta(after, before);
    delta.retained_peak_bytes = peaks.retained_peak_bytes;
    delta.selector_diagnostic_peak_bytes = peaks.selector_diagnostic_peak_bytes;
    delta.total_retained_peak_bytes = peaks.total_retained_peak_bytes;
    delta
}

fn persistent_diagnostic_bytes(diagnostics: &GeneralPersistentVacancyDiagnostics) -> usize {
    option_string_bytes(&diagnostics.parent_fingerprint)
        .saturating_add(option_string_bytes(&diagnostics.initial_state_fingerprint))
        .saturating_add(string_vec_bytes(
            &diagnostics.initial_active_piece_ids,
            diagnostics.initial_active_piece_ids.capacity(),
        ))
        .saturating_add(string_vec_bytes(
            &diagnostics.initial_inactive_piece_ids,
            diagnostics.initial_inactive_piece_ids.capacity(),
        ))
        .saturating_add(option_string_bytes(
            &diagnostics.initial_inactive_order_hash,
        ))
        .saturating_add(option_string_bytes(
            &diagnostics.final_placement_fingerprint,
        ))
        .saturating_add(
            diagnostics.final_placements.capacity()
                * size_of::<GeneralCoupledSeparatorPlacementDiagnostics>(),
        )
        .saturating_add(
            diagnostics
                .final_placements
                .iter()
                .map(|placement| placement.piece_id.capacity())
                .sum::<usize>(),
        )
        .saturating_add(
            diagnostics.layers.capacity() * size_of::<GeneralPersistentVacancyLayerDiagnostics>(),
        )
        .saturating_add(
            diagnostics
                .layers
                .iter()
                .map(layer_diagnostic_heap_bytes)
                .sum::<usize>(),
        )
        .saturating_add(option_string_bytes(
            &diagnostics.pre_expedition_behavior_hash,
        ))
        .saturating_add(
            diagnostics
                .repair_expedition
                .as_ref()
                .map_or(0, repair_diagnostic_heap_bytes),
        )
        .saturating_add(option_string_bytes(&diagnostics.cap_exhausted))
        .saturating_add(option_string_bytes(&diagnostics.failure_reason))
}

fn layer_diagnostic_heap_bytes(layer: &GeneralPersistentVacancyLayerDiagnostics) -> usize {
    string_vec_bytes(
        &layer.selected_piece_ids,
        layer.selected_piece_ids.capacity(),
    )
    .saturating_add(
        layer.parent_selections.capacity()
            * size_of::<GeneralPersistentVacancyParentSelectionDiagnostics>(),
    )
    .saturating_add(
        layer
            .parent_selections
            .iter()
            .map(parent_selection_heap_bytes)
            .sum::<usize>(),
    )
    .saturating_add(string_vec_bytes(
        &layer.best_inactive_piece_ids,
        layer.best_inactive_piece_ids.capacity(),
    ))
    .saturating_add(layer.best_inactive_area_grid2.capacity())
    .saturating_add(layer.best_state_fingerprint.capacity())
    .saturating_add(option_string_bytes(&layer.retained_population_hash))
    .saturating_add(
        layer
            .macro_expansion
            .as_ref()
            .map_or(0, macro_expansion_diagnostic_heap_bytes),
    )
    .saturating_add(
        layer
            .elite
            .as_ref()
            .map_or(0, elite_layer_diagnostic_heap_bytes),
    )
}

fn repair_diagnostic_heap_bytes(repair: &GeneralPersistentVacancyRepairDiagnostics) -> usize {
    repair
        .scheduler_family
        .capacity()
        .saturating_add(repair.root_state_fingerprint.capacity())
        .saturating_add(repair.root_inactive_area_grid2.capacity())
        .saturating_add(string_vec_bytes(
            &repair.root_queue_piece_ids,
            repair.root_queue_piece_ids.capacity(),
        ))
        .saturating_add(
            repair.depths.capacity() * size_of::<GeneralPersistentVacancyRepairDepthDiagnostics>(),
        )
        .saturating_add(
            repair
                .depths
                .iter()
                .map(repair_depth_diagnostic_heap_bytes)
                .sum::<usize>(),
        )
        .saturating_add(option_string_bytes(&repair.endpoint_state_fingerprint))
        .saturating_add(option_string_bytes(&repair.endpoint_inactive_area_grid2))
        .saturating_add(option_string_bytes(&repair.final_placement_fingerprint))
        .saturating_add(option_string_bytes(&repair.cap_exhausted))
        .saturating_add(option_string_bytes(&repair.failure_reason))
}

fn repair_depth_diagnostic_heap_bytes(
    depth: &GeneralPersistentVacancyRepairDepthDiagnostics,
) -> usize {
    depth
        .expansions
        .capacity()
        .saturating_mul(size_of::<GeneralPersistentVacancyRepairExpansionDiagnostics>())
        .saturating_add(
            depth
                .expansions
                .iter()
                .map(repair_expansion_diagnostic_heap_bytes)
                .sum::<usize>(),
        )
        .saturating_add(depth.frontier_hash.capacity())
        .saturating_add(
            depth.frontier.capacity() * size_of::<GeneralPersistentVacancyRepairNodeDiagnostics>(),
        )
        .saturating_add(
            depth
                .frontier
                .iter()
                .map(repair_node_diagnostic_heap_bytes)
                .sum::<usize>(),
        )
        .saturating_add(option_string_bytes(&depth.best_inactive_area_grid2))
}

fn repair_expansion_diagnostic_heap_bytes(
    expansion: &GeneralPersistentVacancyRepairExpansionDiagnostics,
) -> usize {
    expansion
        .parent_augmented_identity_hash
        .capacity()
        .saturating_add(expansion.parent_state_fingerprint.capacity())
        .saturating_add(string_vec_bytes(
            &expansion.parent_queue_piece_ids,
            expansion.parent_queue_piece_ids.capacity(),
        ))
        .saturating_add(expansion.selected_piece_id.capacity())
        .saturating_add(expansion.proposal_order_hash.capacity())
        .saturating_add(expansion.exact_row_order_hash.capacity())
        .saturating_add(expansion.generated_child_order_hash.capacity())
}

fn repair_node_diagnostic_heap_bytes(
    node: &GeneralPersistentVacancyRepairNodeDiagnostics,
) -> usize {
    node.augmented_identity_hash
        .capacity()
        .saturating_add(node.state_fingerprint.capacity())
        .saturating_add(string_vec_bytes(
            &node.queue_piece_ids,
            node.queue_piece_ids.capacity(),
        ))
        .saturating_add(node.inactive_area_grid2.capacity())
}

fn macro_expansion_diagnostic_heap_bytes(
    macro_expansion: &GeneralPersistentVacancyMacroExpansionDiagnostics,
) -> usize {
    macro_expansion
        .parent_state_fingerprint
        .capacity()
        .saturating_add(
            macro_expansion
                .parent_origin
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(macro_expansion.child_order_hash.capacity())
        .saturating_add(string_vec_bytes(
            &macro_expansion.novel_child_fingerprints,
            macro_expansion.novel_child_fingerprints.capacity(),
        ))
        .saturating_add(string_vec_bytes(
            &macro_expansion.retained_child_fingerprints,
            macro_expansion.retained_child_fingerprints.capacity(),
        ))
        .saturating_add(string_vec_bytes(
            &macro_expansion.selected_piece_ids,
            macro_expansion.selected_piece_ids.capacity(),
        ))
        .saturating_add(parent_selection_heap_bytes(
            &macro_expansion.parent_selection,
        ))
}

fn elite_layer_diagnostic_heap_bytes(
    elite: &GeneralPersistentVacancyEliteLayerDiagnostics,
) -> usize {
    elite
        .entering_population_hash
        .capacity()
        .saturating_add(elite.ordinary_child_order_hash.capacity())
        .saturating_add(elite.complete_candidate_order_hash.capacity())
        .saturating_add(elite.area_elite_fingerprint.capacity())
        .saturating_add(elite.area_elite_inactive_area_grid2.capacity())
        .saturating_add(elite.count_elite_fingerprint.capacity())
        .saturating_add(elite.count_elite_inactive_area_grid2.capacity())
        .saturating_add(elite.best_ever_area_elite_fingerprint.capacity())
        .saturating_add(elite.best_ever_area_elite_inactive_area_grid2.capacity())
        .saturating_add(elite.best_ever_count_elite_fingerprint.capacity())
        .saturating_add(elite.best_ever_count_elite_inactive_area_grid2.capacity())
        .saturating_add(string_vec_bytes(
            &elite.offered_carryover_fingerprints,
            elite.offered_carryover_fingerprints.capacity(),
        ))
        .saturating_add(string_vec_bytes(
            &elite.retained_carryover_fingerprints,
            elite.retained_carryover_fingerprints.capacity(),
        ))
        .saturating_add(string_vec_bytes(
            &elite.expanded_carryover_fingerprints,
            elite.expanded_carryover_fingerprints.capacity(),
        ))
}

fn parent_selection_heap_bytes(
    selection: &GeneralPersistentVacancyParentSelectionDiagnostics,
) -> usize {
    selection
        .parent_state_fingerprint
        .capacity()
        .saturating_add(selection.inactive_order_hash.capacity())
        .saturating_add(selection.scheduler_family.capacity())
        .saturating_add(selection.hardest_piece_id.capacity())
        .saturating_add(
            selection
                .coverage_piece_id
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(
            selection.slots.capacity()
                * size_of::<GeneralPersistentVacancySelectionSlotDiagnostics>(),
        )
        .saturating_add(
            selection
                .slots
                .iter()
                .map(|slot| slot.piece_id.capacity())
                .sum::<usize>(),
        )
}

fn string_vec_bytes(strings: &[String], capacity: usize) -> usize {
    capacity
        .saturating_mul(size_of::<String>())
        .saturating_add(strings.iter().map(String::capacity).sum::<usize>())
}

fn option_string_bytes(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::IrregularPoint;

    fn square(size: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            IrregularPoint::new(0.0, 0.0),
            IrregularPoint::new(size, 0.0),
            IrregularPoint::new(size, size),
            IrregularPoint::new(0.0, size),
        ])
        .unwrap()
    }

    fn state_with_two_squares(second_x: f64, second_y: f64) -> (Vec<PolygonSet>, VacancyState) {
        let polygons = vec![square(10.0), square(10.0)];
        let placements = vec![
            RelaxedPlacement {
                input_index: 0,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            },
            RelaxedPlacement {
                input_index: 1,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: second_x,
                translate_y: second_y,
            },
        ];
        let collisions = vec![
            Some(Arc::new(polygons[0].clone())),
            Some(Arc::new(
                polygons[1]
                    .transformed(0.0, false, second_x, second_y)
                    .unwrap(),
            )),
        ];
        (
            polygons,
            VacancyState {
                placements,
                active: vec![true, true],
                collisions,
                last_transition: None,
            },
        )
    }

    fn selector_ids(ids: &[&str], layer: usize, mode: usize) -> Vec<String> {
        let polygons = ids.iter().map(|_| square(10.0)).collect::<Vec<_>>();
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let placements = ids
            .iter()
            .enumerate()
            .map(|(index, _)| RelaxedPlacement {
                input_index: index,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            })
            .collect::<Vec<_>>();
        let state = VacancyState {
            placements,
            active: vec![false; ids.len()],
            collisions: vec![None; ids.len()],
            last_transition: None,
        };
        let difficulty = ids
            .iter()
            .map(|id| {
                let rank = match *id {
                    "b" => 100,
                    "d" => 80,
                    "c" => 60,
                    _ => 40,
                };
                PieceDifficulty {
                    expanded_area_grid2: rank,
                    hull_deficit_grid2: rank,
                    minimum_side_grid: rank as i64,
                    material_area_grid2: rank,
                }
            })
            .collect::<Vec<_>>();
        selected_inactive_pieces(&state, &pieces, &difficulty, layer, mode)
            .indices
            .into_iter()
            .map(|index| pieces[index].id.to_owned())
            .collect()
    }

    fn state_with_active_mask(active: Vec<bool>) -> VacancyState {
        let placements = active
            .iter()
            .enumerate()
            .map(|(index, _)| RelaxedPlacement {
                input_index: index,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: index as f64 * 20.0,
                translate_y: 0.0,
            })
            .collect::<Vec<_>>();
        VacancyState {
            collisions: vec![None; active.len()],
            placements,
            active,
            last_transition: None,
        }
    }

    fn test_difficulties(areas: &[i128]) -> Vec<PieceDifficulty> {
        areas
            .iter()
            .map(|area| PieceDifficulty {
                expanded_area_grid2: *area,
                hull_deficit_grid2: *area,
                minimum_side_grid: *area as i64,
                material_area_grid2: *area,
            })
            .collect()
    }

    #[test]
    fn semantic_identity_ignores_diagnostic_history_and_inactive_pose() {
        let (_, mut first) = state_with_two_squares(10.0, 0.0);
        first.active[1] = false;
        first.collisions[1] = None;
        let mut second = first.clone();
        second.placements[1].translate_x = 999.0;
        assert_eq!(state_identity(&first), state_identity(&second));
    }

    #[test]
    fn last_transition_remains_part_of_semantic_identity() {
        let (polygons, mut first) = state_with_two_squares(10.0, 0.0);
        let mut second = first.clone();
        first.last_transition = Some(VacancyTransition {
            inserted: 0,
            ejected: vec![1],
        });
        second.last_transition = Some(VacancyTransition {
            inserted: 1,
            ejected: vec![0],
        });
        assert_ne!(state_identity(&first), state_identity(&second));
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        assert_ne!(
            state_fingerprint(&first, &pieces),
            state_fingerprint(&second, &pieces)
        );
        assert_eq!(
            coupled_fast_placement_fingerprint(&fast_placements(&first, &pieces, false)),
            coupled_fast_placement_fingerprint(&fast_placements(&second, &pieces, false))
        );
    }

    #[test]
    fn contact_signature_distinguishes_axis_and_ignores_corner() {
        let (polygons, x_state) = state_with_two_squares(10.0, 0.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let (_, y_state) = state_with_two_squares(0.0, 10.0);
        let (_, corner_state) = state_with_two_squares(10.0, 10.0);
        assert_eq!(contact_signature(&x_state, &pieces).edges[0].axis, 0);
        assert_eq!(contact_signature(&y_state, &pieces).edges[0].axis, 1);
        assert!(contact_signature(&corner_state, &pieces).edges.is_empty());
        assert_ne!(
            contact_signature_hash(&contact_signature(&x_state, &pieces)),
            contact_signature_hash(&contact_signature(&y_state, &pieces))
        );
    }

    #[test]
    fn shared_state_seed_does_not_depend_on_population_ordinal() {
        let (polygons, state) = state_with_two_squares(10.0, 0.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let seed = parent_seed_key(&state, &pieces);
        assert_eq!(
            derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ seed, 4, 0),
            derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ seed, 4, 0)
        );
    }

    #[test]
    fn selector_families_share_streams_across_retention_modes() {
        let ids = ["d", "a", "c", "b"];
        assert_eq!(selector_ids(&ids, 2, 1), selector_ids(&ids, 2, 2));
        assert_eq!(selector_ids(&ids, 2, 3), selector_ids(&ids, 2, 4));
        assert_eq!(selector_ids(&ids, 2, 3), selector_ids(&ids, 2, 5));
        assert_eq!(selector_ids(&ids, 2, 3), selector_ids(&ids, 2, 6));
        assert_eq!(
            selector_ids(&ids, 2, 3),
            selector_ids(&ids, 2, MACRO_CONTROL_MODE)
        );
        assert_eq!(
            selector_ids(&ids, 2, 3),
            selector_ids(&ids, 2, MACRO_TREATMENT_MODE)
        );
        assert_eq!(
            selector_ids(&ids, 2, 3),
            selector_ids(&ids, 2, PRESERVED_BEST_MACRO_MODE)
        );
    }

    #[test]
    fn stateless_rotation_is_stable_under_input_storage_permutation() {
        let first = selector_ids(&["d", "a", "c", "b"], 2, 3);
        let second = selector_ids(&["b", "c", "a", "d"], 2, 3);
        assert_eq!(first, second);
    }

    #[test]
    fn stateless_rotation_singleton_has_only_the_hard_slot() {
        assert_eq!(selector_ids(&["b"], 7, 3), vec!["b"]);
    }

    #[test]
    fn stateless_rotation_skips_hard_slot_and_covers_a_fixed_set() {
        let ids = ["d", "a", "c", "b"];
        let mut coverage = BTreeSet::new();
        for layer in 0..ids.len() {
            let selected = selector_ids(&ids, layer, 3);
            assert_eq!(selected[0], "b");
            assert_ne!(selected[1], "b");
            coverage.insert(selected[1].clone());
        }
        assert_eq!(
            coverage,
            BTreeSet::from(["a".to_owned(), "c".to_owned(), "d".to_owned()])
        );
    }

    #[test]
    fn selector_diagnostics_are_accounted_separately_from_state_memory() {
        let mut diagnostics = GeneralPersistentVacancyDiagnostics {
            initial_active_piece_ids: vec!["a".to_owned()],
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        let pending = GeneralPersistentVacancyLayerDiagnostics {
            selected_piece_ids: vec!["a".to_owned()],
            parent_selections: vec![GeneralPersistentVacancyParentSelectionDiagnostics {
                parent_state_fingerprint: "state".to_owned(),
                inactive_order_hash: "inactive".to_owned(),
                scheduler_family: "twoHardest".to_owned(),
                hardest_piece_id: "a".to_owned(),
                slots: vec![GeneralPersistentVacancySelectionSlotDiagnostics {
                    piece_id: "a".to_owned(),
                    ..GeneralPersistentVacancySelectionSlotDiagnostics::default()
                }],
                ..GeneralPersistentVacancyParentSelectionDiagnostics::default()
            }],
            ..GeneralPersistentVacancyLayerDiagnostics::default()
        };
        let mut work = RunWork::default();
        charge_retained_memory(&[], None, 0, &mut diagnostics, &pending, &mut work).unwrap();
        assert_eq!(work.diagnostics.retained_peak_bytes, 0);
        assert!(work.diagnostics.selector_diagnostic_peak_bytes > 0);
        assert_eq!(
            work.diagnostics.total_retained_peak_bytes,
            work.diagnostics.selector_diagnostic_peak_bytes
        );
    }

    #[test]
    fn repair_and_population_hash_diagnostics_are_charged() {
        let mut diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let baseline = persistent_diagnostic_bytes(&diagnostics);
        diagnostics.pre_expedition_behavior_hash = Some("p".repeat(64));
        diagnostics
            .layers
            .push(GeneralPersistentVacancyLayerDiagnostics {
                retained_population_hash: Some("r".repeat(64)),
                ..GeneralPersistentVacancyLayerDiagnostics::default()
            });
        diagnostics.repair_expedition = Some(GeneralPersistentVacancyRepairDiagnostics {
            scheduler_family: "oneSlotDisplacedFirst".to_owned(),
            root_state_fingerprint: "s".repeat(64),
            root_inactive_area_grid2: "123".to_owned(),
            root_queue_piece_ids: vec!["piece".to_owned()],
            depths: vec![GeneralPersistentVacancyRepairDepthDiagnostics {
                expansions: vec![GeneralPersistentVacancyRepairExpansionDiagnostics {
                    parent_augmented_identity_hash: "a".repeat(64),
                    parent_state_fingerprint: "b".repeat(64),
                    parent_queue_piece_ids: vec!["piece".to_owned()],
                    selected_piece_id: "piece".to_owned(),
                    proposal_order_hash: "c".repeat(64),
                    exact_row_order_hash: "d".repeat(64),
                    generated_child_order_hash: "e".repeat(64),
                    ..GeneralPersistentVacancyRepairExpansionDiagnostics::default()
                }],
                frontier_hash: "f".repeat(64),
                frontier: vec![GeneralPersistentVacancyRepairNodeDiagnostics {
                    augmented_identity_hash: "g".repeat(64),
                    state_fingerprint: "h".repeat(64),
                    queue_piece_ids: vec!["piece".to_owned()],
                    inactive_area_grid2: "456".to_owned(),
                    ..GeneralPersistentVacancyRepairNodeDiagnostics::default()
                }],
                ..GeneralPersistentVacancyRepairDepthDiagnostics::default()
            }],
            ..GeneralPersistentVacancyRepairDiagnostics::default()
        });
        assert!(persistent_diagnostic_bytes(&diagnostics) > baseline + 9 * 64);
    }

    #[test]
    fn repair_memory_preflight_accounts_full_piece_id_capacity() {
        let (polygons, root) = state_with_two_squares(10.0, 0.0);
        let short_ids = ["a", "b"];
        let long_ids = [
            "piece-with-a-frozen-identifier-that-is-longer-than-thirty-two-a",
            "piece-with-a-frozen-identifier-that-is-longer-than-thirty-two-b",
        ];
        let short_pieces = short_ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let long_pieces = long_ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut short_work = RunWork::for_mode(REPAIR_CONTROL_MODE);
        preflight_repair_memory(&root, &short_pieces, &diagnostics, "test", &mut short_work)
            .unwrap();
        let mut long_work = RunWork::for_mode(REPAIR_CONTROL_MODE);
        preflight_repair_memory(&root, &long_pieces, &diagnostics, "test", &mut long_work).unwrap();
        assert!(
            long_work.diagnostics.total_retained_peak_bytes
                > short_work.diagnostics.total_retained_peak_bytes
        );
        assert!(long_work.diagnostics.selector_diagnostic_peak_bytes > 0);
    }

    #[test]
    fn repair_expedition_events_publish_only_at_commit() {
        let mut diagnostics = GeneralPersistentVacancyDiagnostics {
            direct_insertions: 7,
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        let mut work = RunWork::for_mode(REPAIR_CONTROL_MODE);
        work.diagnostics.selected_piece_slots = 11;
        let events = GeneralPersistentVacancyDiagnostics {
            direct_insertions: 3,
            ejection_insertions: 2,
            complete_states: 1,
            publication_rejections: 1,
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        let mut staged_work = RunWork::for_mode(REPAIR_CONTROL_MODE);
        staged_work.diagnostics.selected_piece_slots = 19;
        assert_eq!(diagnostics.direct_insertions, 7);
        assert_eq!(work.diagnostics.selected_piece_slots, 11);
        commit_repair_expedition(
            &mut diagnostics,
            &mut work,
            events,
            staged_work,
            GeneralPersistentVacancyRepairDiagnostics::default(),
        );
        assert_eq!(diagnostics.direct_insertions, 10);
        assert_eq!(diagnostics.ejection_insertions, 2);
        assert_eq!(diagnostics.complete_states, 1);
        assert_eq!(diagnostics.publication_rejections, 1);
        assert_eq!(work.diagnostics.selected_piece_slots, 19);
        assert!(diagnostics.repair_expedition.is_some());
    }

    #[test]
    fn failed_repair_record_retains_work_without_semantic_events() {
        let (polygons, root) = state_with_two_squares(10.0, 0.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let difficulty = test_difficulties(&[1, 2]);
        let consumed = GeneralPersistentVacancyWorkDiagnostics {
            selected_piece_slots: 1,
            total_retained_peak_bytes: 1234,
            ..GeneralPersistentVacancyWorkDiagnostics::default()
        };
        let failed = failed_repair_diagnostics(
            &root,
            &pieces,
            &difficulty,
            REPAIR_TREATMENT_MODE,
            false,
            "cap: test failure",
            consumed,
        );
        assert_eq!(failed.cap_exhausted.as_deref(), Some("test failure"));
        assert_eq!(failed.failure_reason.as_deref(), Some("cap: test failure"));
        assert_eq!(failed.work, consumed);
        assert!(failed.depths.is_empty());
        assert!(!failed.root_dual_valid);
        let failed_after_root_audit = failed_repair_diagnostics(
            &root,
            &pieces,
            &difficulty,
            REPAIR_TREATMENT_MODE,
            true,
            "cap: later failure",
            consumed,
        );
        assert!(failed_after_root_audit.root_dual_valid);
    }

    #[test]
    fn count_comparator_prefers_fewer_inactive_pieces_over_lower_area() {
        let polygons = (0..3).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = ["a", "b", "c"];
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = test_difficulties(&[1, 1, 100]);
        let low_area = state_with_active_mask(vec![false, false, true]);
        let low_count = state_with_active_mask(vec![true, true, false]);
        assert!(compare_states(&low_area, &low_count, &pieces, &difficulty).is_lt());
        assert!(compare_count_states(&low_count, &low_area, &pieces, &difficulty).is_lt());
    }

    #[test]
    fn dual_objective_retention_reserves_both_elites_and_keeps_width() {
        let polygons = (0..10).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = (0..10)
            .map(|index| format!("p{index:02}"))
            .collect::<Vec<_>>();
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = test_difficulties(&[1, 2, 4, 8, 16, 32, 64, 128, 256, 1_000]);
        let area_elite = state_with_active_mask(vec![
            false, false, true, true, true, true, true, true, true, true,
        ]);
        let count_elite = state_with_active_mask(vec![
            true, true, true, true, true, true, true, true, true, false,
        ]);
        let mut states = vec![area_elite.clone(), count_elite.clone()];
        for first in 1..9 {
            let mut active = vec![true; 10];
            active[first] = false;
            active[(first + 1) % 9] = false;
            states.push(state_with_active_mask(active));
        }
        states.sort_by(|first, second| compare_states(first, second, &pieces, &difficulty));
        let (retained, _) = retain_population(states, &pieces, &difficulty, 6);
        assert_eq!(retained.len(), BEAM_WIDTH);
        let identities = retained.iter().map(state_identity).collect::<BTreeSet<_>>();
        assert!(identities.contains(&state_identity(&area_elite)));
        assert!(identities.contains(&state_identity(&count_elite)));
        assert_eq!(identities.len(), retained.len());
    }

    #[test]
    fn carryover_pool_changes_mode_five_but_not_mode_six() {
        let polygons = (0..3).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = ["a", "b", "c"];
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = test_difficulties(&[1, 10, 100]);
        let ordinary = state_with_active_mask(vec![true, false, false]);
        let carryover = state_with_active_mask(vec![false, true, true]);
        let (mode_six, _) = retention_pool(
            vec![ordinary.clone()],
            vec![carryover.clone()],
            &pieces,
            &difficulty,
            6,
        );
        let (mode_five, _) = retention_pool(
            vec![ordinary],
            vec![carryover.clone()],
            &pieces,
            &difficulty,
            5,
        );
        assert_eq!(mode_six.len(), 1);
        assert_eq!(mode_five.len(), 2);
        assert!(mode_five
            .iter()
            .any(|state| state_identity(state) == state_identity(&carryover)));
    }

    #[test]
    fn population_and_child_order_hashes_are_deterministic_and_domain_separated() {
        let polygons = vec![square(10.0), square(10.0)];
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let first = state_with_active_mask(vec![true, false]);
        let second = state_with_active_mask(vec![true, true]);
        let population = vec![first.clone(), second.clone()];
        assert_eq!(
            population_hash(&population, &pieces),
            population_hash(&population, &pieces)
        );
        assert_eq!(
            child_order_hash(&population, &pieces),
            child_order_hash(&population, &pieces)
        );
        assert_ne!(
            population_hash(&population, &pieces),
            child_order_hash(&population, &pieces)
        );
        assert_ne!(
            child_order_hash(&population, &pieces),
            child_order_hash(&[second, first], &pieces)
        );
    }

    #[test]
    fn macro_novelty_and_treatment_admission_use_semantic_identity() {
        let polygons = vec![square(10.0), square(10.0)];
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let ordinary = state_with_active_mask(vec![true, false]);
        let novel = state_with_active_mask(vec![false, true]);
        let macro_children = vec![ordinary.clone(), novel.clone()];
        assert_eq!(
            novel_macro_child_fingerprints(&[ordinary.clone()], &macro_children, &pieces),
            vec![state_fingerprint(&novel, &pieces)]
        );

        let combined = vec![ordinary.clone(), novel.clone()];
        let control = select_macro_retention_children(
            vec![ordinary.clone()],
            Some(combined.clone()),
            MACRO_CONTROL_MODE,
        );
        let treatment = select_macro_retention_children(
            vec![ordinary.clone()],
            Some(combined),
            MACRO_TREATMENT_MODE,
        );
        assert_eq!(control.len(), 1);
        assert!(same_state_identity(&control[0], &ordinary));
        assert_eq!(treatment.len(), 2);
        assert!(treatment
            .iter()
            .any(|state| same_state_identity(state, &novel)));
    }

    #[test]
    fn preserved_best_macro_parent_is_used_only_when_absent_from_ordinary_children() {
        let ordinary = state_with_active_mask(vec![true, false]);
        let preserved = state_with_active_mask(vec![false, true]);
        let ordinary_children = vec![ordinary.clone()];
        let choice = select_macro_parent(
            &ordinary_children,
            Some(&preserved),
            PRESERVED_BEST_MACRO_MODE,
        )
        .unwrap();
        assert!(same_state_identity(choice.state, &preserved));
        assert_eq!(choice.origin, Some("bestEverArea"));
        assert_eq!(choice.preserved_parent_absent_from_ordinary, Some(true));

        let present_children = vec![preserved.clone(), ordinary];
        let choice = select_macro_parent(
            &present_children,
            Some(&preserved),
            PRESERVED_BEST_MACRO_MODE,
        )
        .unwrap();
        assert!(same_state_identity(choice.state, &present_children[0]));
        assert_eq!(choice.origin, Some("ordinaryBest"));
        assert_eq!(choice.preserved_parent_absent_from_ordinary, Some(false));
    }

    #[test]
    fn legacy_macro_diagnostics_omit_preserved_parent_fields() {
        let value = serde_json::to_value(GeneralPersistentVacancyMacroExpansionDiagnostics {
            parent_state_fingerprint: "parent".to_owned(),
            ..GeneralPersistentVacancyMacroExpansionDiagnostics::default()
        })
        .unwrap();
        assert!(value.get("parentOrigin").is_none());
        assert!(value.get("preservedParentAbsentFromOrdinary").is_none());
    }

    #[test]
    fn macro_complete_candidate_is_audited_but_not_accepted_by_control() {
        let (polygons, mut complete) = state_with_two_squares(10.0, 0.0);
        complete.placements[0].translate_x = 1.0;
        complete.placements[0].translate_y = 1.0;
        complete.placements[1].translate_x = 12.0;
        complete.placements[1].translate_y = 1.0;
        complete.collisions = vec![
            Some(Arc::new(
                polygons[0].transformed(0.0, false, 1.0, 1.0).unwrap(),
            )),
            Some(Arc::new(
                polygons[1].transformed(0.0, false, 12.0, 1.0).unwrap(),
            )),
        ];
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut ordinary_partial = complete.clone();
        ordinary_partial.active[1] = false;
        ordinary_partial.collisions[1] = None;
        let settings = GeneralFastSettings::deterministic_test(100.0, TARGET_DEPTH_MM);
        let mut diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut work = RunWork::default();
        let (combined_accepted, ordinary_accepted) = audit_macro_complete_candidates(
            &[complete],
            &[ordinary_partial],
            &pieces,
            settings,
            &mut diagnostics,
            &mut work,
        )
        .unwrap();
        assert!(combined_accepted.is_some());
        assert!(ordinary_accepted.is_none());
        assert_eq!(work.diagnostics.complete_audits, 1);
        assert_eq!(diagnostics.publication_rejections, 0);
    }

    #[test]
    fn live_pool_memory_counts_a_duplicate_carryover_before_deduplication() {
        let (_, state) = state_with_two_squares(10.0, 0.0);
        let entering = vec![state.clone()];
        let pending = GeneralPersistentVacancyLayerDiagnostics::default();
        let mut without_diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut without_work = RunWork::default();
        preflight_live_memory(
            &entering,
            0,
            0,
            0,
            0,
            0,
            &mut without_diagnostics,
            &pending,
            &mut without_work,
        )
        .unwrap();
        let mut with_diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut with_work = RunWork::default();
        preflight_live_memory(
            &entering,
            0,
            0,
            state_vec_bytes(&vec![state]),
            0,
            0,
            &mut with_diagnostics,
            &pending,
            &mut with_work,
        )
        .unwrap();
        assert!(
            with_work.diagnostics.total_retained_peak_bytes
                > without_work.diagnostics.total_retained_peak_bytes
        );
    }

    #[test]
    fn preserved_state_receives_a_distinct_later_layer_stream() {
        let polygons = (0..4).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = ["a", "b", "c", "d"];
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let state = state_with_active_mask(vec![false; 4]);
        let difficulty = test_difficulties(&[1, 100, 10, 20]);
        let first = selected_inactive_pieces(&state, &pieces, &difficulty, 0, 5);
        let second = selected_inactive_pieces(&state, &pieces, &difficulty, 1, 5);
        assert_ne!(first.indices, second.indices);
        let seed = parent_seed_key(&state, &pieces);
        assert_ne!(
            derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ seed, 0, 0),
            derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ seed, 1, 0)
        );
    }

    #[test]
    fn shared_population_has_identical_layer_local_work_evidence() {
        let polygons = vec![square(10.0), square(10.0)];
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let population = vec![state_with_active_mask(vec![true, false])];
        let before = GeneralPersistentVacancyWorkDiagnostics {
            selected_piece_slots: 10,
            hazard_queries: 20,
            ..GeneralPersistentVacancyWorkDiagnostics::default()
        };
        let after = GeneralPersistentVacancyWorkDiagnostics {
            selected_piece_slots: 12,
            hazard_queries: 27,
            retained_peak_bytes: 999,
            ..GeneralPersistentVacancyWorkDiagnostics::default()
        };
        let first_hash = population_hash(&population, &pieces);
        let second_hash = population_hash(&population.clone(), &pieces);
        let first_delta = work_delta(
            generation_work_snapshot(after),
            generation_work_snapshot(before),
        );
        let second_delta = work_delta(
            generation_work_snapshot(after),
            generation_work_snapshot(before),
        );
        assert_eq!(first_hash, second_hash);
        assert_eq!(first_delta, second_delta);
        assert_eq!(first_delta.selected_piece_slots, 2);
        assert_eq!(first_delta.hazard_queries, 7);
        assert_eq!(first_delta.retained_peak_bytes, 0);
    }

    #[test]
    fn best_ever_elites_are_monotonic_under_their_own_comparators() {
        let polygons = (0..3).map(|_| square(10.0)).collect::<Vec<_>>();
        let ids = ["a", "b", "c"];
        let pieces = ids
            .iter()
            .enumerate()
            .map(|(index, id)| GeneralFastPiece {
                id,
                polygon: &polygons[index],
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = test_difficulties(&[1, 1, 100]);
        let lower_area = elite_snapshot(
            &state_with_active_mask(vec![false, false, true]),
            &pieces,
            &difficulty,
        );
        let lower_count = elite_snapshot(
            &state_with_active_mask(vec![true, true, false]),
            &pieces,
            &difficulty,
        );
        let mut best_area = None;
        update_best_area(&mut best_area, &lower_count);
        update_best_area(&mut best_area, &lower_area);
        update_best_area(&mut best_area, &lower_count);
        assert_eq!(best_area.unwrap().fingerprint, lower_area.fingerprint);
        let mut best_count = None;
        update_best_count(&mut best_count, &lower_area);
        update_best_count(&mut best_count, &lower_count);
        update_best_count(&mut best_count, &lower_area);
        assert_eq!(best_count.unwrap().fingerprint, lower_count.fingerprint);
    }

    #[test]
    fn raw_live_pool_cap_failure_is_atomic() {
        let mut diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut work = RunWork::default();
        let result = preflight_raw_live_memory(
            &Vec::new(),
            0,
            MAX_RETAINED_BYTES,
            0,
            0,
            0,
            &[],
            &[],
            0,
            &mut diagnostics,
            &mut work,
        );
        assert_eq!(
            result.unwrap_err(),
            "cap: pre-deduplication live-pool memory budget exhausted"
        );
        assert!(diagnostics.layers.is_empty());
    }

    #[test]
    fn preserved_best_state_is_charged_before_raw_pool_allocation() {
        let mut diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut work = RunWork::default();
        let result = preflight_raw_live_memory(
            &Vec::new(),
            MAX_RETAINED_BYTES,
            0,
            0,
            0,
            0,
            &[],
            &[],
            0,
            &mut diagnostics,
            &mut work,
        );
        assert_eq!(
            result.unwrap_err(),
            "cap: pre-deduplication live-pool memory budget exhausted"
        );
        assert!(diagnostics.layers.is_empty());
    }

    #[test]
    fn raw_macro_pool_cap_failure_accounts_pending_diagnostics_atomically() {
        let macro_diagnostics = GeneralPersistentVacancyMacroExpansionDiagnostics {
            parent_state_fingerprint: "parent".to_owned(),
            child_order_hash: "children".to_owned(),
            novel_child_fingerprints: vec!["novel".to_owned()],
            retained_child_fingerprints: vec!["retained".to_owned()],
            selected_piece_ids: vec!["piece".to_owned()],
            ..GeneralPersistentVacancyMacroExpansionDiagnostics::default()
        };
        let mut diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut work = RunWork::default();
        let result = preflight_raw_live_memory(
            &Vec::new(),
            0,
            MAX_RETAINED_BYTES,
            0,
            0,
            0,
            &[],
            &[],
            macro_expansion_diagnostic_heap_bytes(&macro_diagnostics),
            &mut diagnostics,
            &mut work,
        );
        assert_eq!(
            result.unwrap_err(),
            "cap: pre-deduplication live-pool memory budget exhausted"
        );
        assert!(diagnostics.layers.is_empty());
        assert!(work.diagnostics.selector_diagnostic_peak_bytes > 0);
    }

    #[test]
    fn dual_objective_modes_reject_nonterminal_width_changes() {
        assert!(enforce_population_width(6, false, BEAM_WIDTH - 1, 4).is_err());
        assert!(enforce_population_width(5, false, BEAM_WIDTH, 4).is_ok());
        assert!(enforce_population_width(6, true, 1, 4).is_ok());
        assert!(enforce_population_width(MACRO_CONTROL_MODE, false, BEAM_WIDTH - 1, 4).is_err());
        assert!(enforce_population_width(MACRO_TREATMENT_MODE, false, BEAM_WIDTH - 1, 4).is_err());
        assert!(enforce_population_width(PRESERVED_BEST_MACRO_MODE, false, BEAM_WIDTH, 4).is_ok());
        assert!(enforce_population_width(3, false, BEAM_WIDTH - 1, 4).is_ok());
    }

    #[test]
    fn mirrored_source_budget_funds_both_traversals() {
        assert_eq!(
            MAX_SOURCE_FEATURE_VISITS,
            MAX_SELECTED_PIECE_SLOTS * 2 * MAX_SOURCE_FEATURES
        );
    }

    #[test]
    fn aggregate_quota_formulas_match_the_reviewed_contract() {
        assert_eq!(MAX_PARENT_EXPANSIONS, 360);
        assert_eq!(MAX_SELECTED_PIECE_SLOTS, 720);
        assert_eq!(MAX_ORIENTATION_STREAMS, 8_640);
        assert_eq!(MAX_EXACT_FINALIST_ROWS, 5_760);
        assert_eq!(MAX_EXPERIMENTAL_COLLISION_BUILDS, 61 + 8_640 + 5_760);
        assert_eq!(MAX_EXPERIMENTAL_PAIR_VISITS, 1_830 + 345_600);
        assert_eq!(MAX_VALIDATOR_COLLISION_BUILDS, 105 * 122);
        assert_eq!(MAX_VALIDATOR_PAIR_VISITS, 105 * 3_660);
        assert_eq!(
            MAX_TRANSFORMED_COLLISION_VERTICES,
            (MAX_EXPERIMENTAL_COLLISION_BUILDS + MAX_VALIDATOR_COLLISION_BUILDS)
                * MAX_COLLISION_VERTICES
        );
        assert_eq!(
            MAX_CLIPPER_INPUT_VERTICES,
            (MAX_EXPERIMENTAL_PAIR_VISITS + MAX_VALIDATOR_PAIR_VISITS) * 2 * MAX_COLLISION_VERTICES
        );
    }

    #[test]
    fn repair_quota_formulas_match_the_reviewed_contract() {
        assert_eq!(REPAIR_PARENT_EXPANSIONS, 61);
        assert_eq!(REPAIR_MAX_SELECTED_PIECE_SLOTS, 781);
        assert_eq!(REPAIR_MAX_ORIENTATION_STREAMS, 9_372);
        assert_eq!(REPAIR_MAX_SOURCE_FEATURE_VISITS, 799_744);
        assert_eq!(REPAIR_MAX_POSITION_SOURCE_ATTEMPTS, 4_957_788);
        assert_eq!(REPAIR_MAX_RETURNED_POSITIONS, 299_904);
        assert_eq!(REPAIR_MAX_HAZARD_QUERIES, 299_904);
        assert_eq!(REPAIR_MAX_PROXY_PRESSURE_VISITS, 18_294_144);
        assert_eq!(REPAIR_MAX_EXACT_FINALIST_ROWS, 6_248);
        assert_eq!(REPAIR_MAX_EXPERIMENTAL_COLLISION_BUILDS, 15_681);
        assert_eq!(REPAIR_MAX_EXPERIMENTAL_PAIR_VISITS, 376_710);
        assert_eq!(REPAIR_MAX_PARTIAL_AUDITS, 58);
        assert_eq!(REPAIR_MAX_COMPLETE_AUDITS, 552);
        assert_eq!(REPAIR_MAX_VALIDATOR_AUDITS, 610);
        assert_eq!(REPAIR_MAX_VALIDATOR_COLLISION_BUILDS, 74_420);
        assert_eq!(REPAIR_MAX_VALIDATOR_PAIR_VISITS, 2_232_600);
        assert_eq!(REPAIR_MAX_TRANSFORMED_COLLISION_VERTICES, 46_131_712);
        assert_eq!(REPAIR_MAX_CLIPPER_INPUT_VERTICES, 2_671_933_440);
        let protected_phase = RunWork::for_mode(REPAIR_TREATMENT_MODE);
        assert_eq!(protected_phase.limits.selected_piece_slots, 720);
        let repair = WorkLimits::repair();
        assert_eq!(repair.selected_piece_slots, 781);
        let legacy = RunWork::for_mode(PRESERVED_BEST_MACRO_MODE);
        assert_eq!(legacy.limits.selected_piece_slots, 720);
    }

    #[test]
    fn displaced_queue_prepends_blockers_while_control_rebuilds_global_order() {
        let polygons = vec![square(10.0), square(10.0), square(10.0)];
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "c",
                polygon: &polygons[2],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let difficulty = test_difficulties(&[2, 3, 4]);
        let parent = RepairNode {
            state: state_with_active_mask(vec![true, false, false]),
            queue: vec![2, 1],
        };
        let mut child = state_with_active_mask(vec![false, false, true]);
        child.active[1] = false;
        child.last_transition = Some(VacancyTransition {
            inserted: 2,
            ejected: vec![0],
        });
        assert_eq!(
            repair_child_queue(
                &parent,
                &child,
                2,
                &pieces,
                &difficulty,
                REPAIR_CONTROL_MODE,
            )
            .unwrap(),
            vec![1, 0]
        );
        assert_eq!(
            repair_child_queue(
                &parent,
                &child,
                2,
                &pieces,
                &difficulty,
                REPAIR_TREATMENT_MODE,
            )
            .unwrap(),
            vec![0, 1]
        );
    }

    #[test]
    fn repair_seed_ignores_queue_tail_but_augmented_identity_does_not() {
        let polygons = vec![square(10.0), square(10.0), square(10.0), square(10.0)];
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &polygons[0],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &polygons[1],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "c",
                polygon: &polygons[2],
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "d",
                polygon: &polygons[3],
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let state = state_with_active_mask(vec![true, false, false, false]);
        let first = RepairNode {
            state: state.clone(),
            queue: vec![1, 2, 3],
        };
        let second = RepairNode {
            state,
            queue: vec![1, 3, 2],
        };
        assert_eq!(
            repair_transition_seed(&first.state, 1, &pieces),
            repair_transition_seed(&second.state, 1, &pieces)
        );
        assert_ne!(repair_node_identity(&first), repair_node_identity(&second));
    }
}
