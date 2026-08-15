use super::*;

use std::mem::size_of;

const PERSISTENT_VACANCY_SEED_DOMAIN: u64 = 0x5650_4f50_3030_3031;
const TARGET_DEPTH_MM: f64 = 165.0;
const EXPECTED_PARENT_FINGERPRINT: &str =
    "b9335a72cdcdd8df29be21450818f4ab1766ea1ea0b16765ad3998942a2ea6c5";
const EXPECTED_PARENT_DEPTH_MM: f64 = 168.361;
const MAX_LAYERS: usize = 40;
const BEAM_WIDTH: usize = 8;
const SELECTED_PIECES_PER_PARENT: usize = 2;
const ORIENTATIONS_PER_PIECE: usize = 12;
const POSITIONS_PER_ORIENTATION: usize = 32;
const FINALISTS_PER_PIECE: usize = 8;
const MAX_INACTIVE_PIECES: usize = 32;
const MAX_SOURCE_FEATURES: usize = 512;
const MAX_COLLISION_VERTICES: usize = 512;
// Modes 7 and 8 revive one archived elite topology on deterministically
// detected stagnation. A revival may fire no earlier than layer
// ARCHIVE_STAGNATION_LAYERS and at least ARCHIVE_REVIVAL_COOLDOWN layers after
// the previous expanded revival, so at most
// 1 + (MAX_LAYERS - 1 - ARCHIVE_STAGNATION_LAYERS) / ARCHIVE_REVIVAL_COOLDOWN
// revival expansions exist. Mode 7 expands the revived state as an extra
// parent; the quota formulas below fund that lane explicitly on top of the
// ordinary 8-parent schedule. Mode 8 swaps the revived state into the
// comparator-worst entering slot and adds no work.
const ARCHIVE_STAGNATION_LAYERS: usize = 3;
const ARCHIVE_REVIVAL_COOLDOWN: usize = 3;
const MAX_ARCHIVE_REVIVALS: usize =
    1 + (MAX_LAYERS - 1 - ARCHIVE_STAGNATION_LAYERS) / ARCHIVE_REVIVAL_COOLDOWN;
const ORDINARY_SELECTED_PIECE_SLOTS: usize = MAX_LAYERS * BEAM_WIDTH * SELECTED_PIECES_PER_PARENT;
const ARCHIVE_SELECTED_PIECE_SLOTS: usize = MAX_ARCHIVE_REVIVALS * SELECTED_PIECES_PER_PARENT;
const MAX_SELECTED_PIECE_SLOTS: usize =
    ORDINARY_SELECTED_PIECE_SLOTS + ARCHIVE_SELECTED_PIECE_SLOTS;
const MAX_ORIENTATION_STREAMS: usize = MAX_SELECTED_PIECE_SLOTS * ORIENTATIONS_PER_PIECE;
const MAX_SOURCE_FEATURE_VISITS: usize = MAX_SELECTED_PIECE_SLOTS * 2 * MAX_SOURCE_FEATURES;
const POSITION_SOURCE_ATTEMPTS_PER_ORIENTATION: usize = 529;
const MAX_POSITION_SOURCE_ATTEMPTS: usize =
    MAX_ORIENTATION_STREAMS * POSITION_SOURCE_ATTEMPTS_PER_ORIENTATION;
const MAX_RETURNED_POSITIONS: usize = MAX_ORIENTATION_STREAMS * POSITIONS_PER_ORIENTATION;
const MAX_HAZARD_QUERIES: usize = MAX_RETURNED_POSITIONS;
const MAX_PROXY_PRESSURE_VISITS: usize = MAX_RETURNED_POSITIONS * 61;
const MAX_EXACT_FINALIST_ROWS: usize = MAX_SELECTED_PIECE_SLOTS * FINALISTS_PER_PIECE;
const MAX_EXPERIMENTAL_COLLISION_BUILDS: usize =
    61 + MAX_ORIENTATION_STREAMS + MAX_EXACT_FINALIST_ROWS;
const MAX_VALIDATOR_COLLISION_BUILDS: usize = 12_810;
const MAX_EXPERIMENTAL_PAIR_VISITS: usize = 1_830 + MAX_EXACT_FINALIST_ROWS * 60;
const MAX_VALIDATOR_PAIR_VISITS: usize = 384_300;
const MAX_TRANSFORMED_COLLISION_VERTICES: usize =
    (MAX_EXPERIMENTAL_COLLISION_BUILDS + MAX_VALIDATOR_COLLISION_BUILDS) * MAX_COLLISION_VERTICES;
const MAX_CLIPPER_INPUT_VERTICES: usize =
    2 * MAX_COLLISION_VERTICES * (MAX_EXPERIMENTAL_PAIR_VISITS + MAX_VALIDATOR_PAIR_VISITS);
const MAX_CLIPPER_OUTPUT_VERTICES: usize = 4_000_000;
const MAX_PARTIAL_AUDITS: usize = 41;
const MAX_COMPLETE_AUDITS: usize = 64;
const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;

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

/// Bounded out-of-beam topology archive for modes 7 and 8.
///
/// The archive stores full clones of the best-ever area-first and count-first
/// elite states. It never occupies an ordinary beam slot; a revived state is
/// either expanded as one extra parent (mode 7) or swapped into the
/// comparator-worst entering slot (mode 8) on deterministically detected
/// stagnation layers only. Every decision derives from the run's own layer
/// history and semantic state identities; no wall clock, platform, or
/// population-ordinal information enters the schedule.
struct TopologyArchive {
    area: Option<(EliteSnapshot, VacancyState)>,
    count: Option<(EliteSnapshot, VacancyState)>,
    last_improvement_layer: usize,
    last_revival_layer: Option<usize>,
    revivals_expanded: usize,
    revivals_skipped: usize,
    revival_ordinal: usize,
    peak_bytes: usize,
    revival_children_generated: usize,
    revival_children_retained: usize,
}

enum RevivalDecision {
    NotStagnant,
    Skipped(&'static str),
    Revive {
        kind: &'static str,
        state: VacancyState,
        fingerprint: String,
    },
}

impl TopologyArchive {
    fn new() -> Self {
        Self {
            area: None,
            count: None,
            last_improvement_layer: 0,
            last_revival_layer: None,
            revivals_expanded: 0,
            revivals_skipped: 0,
            revival_ordinal: 0,
            peak_bytes: 0,
            revival_children_generated: 0,
            revival_children_retained: 0,
        }
    }

    fn bytes(&self) -> usize {
        [self.area.as_ref(), self.count.as_ref()]
            .into_iter()
            .flatten()
            .map(|(snapshot, state)| {
                size_of::<EliteSnapshot>()
                    .saturating_add(elite_snapshot_heap_bytes(snapshot))
                    .saturating_add(size_of::<VacancyState>())
                    .saturating_add(state_heap_bytes(state))
            })
            .sum()
    }

    fn charge_peak(&mut self) {
        self.peak_bytes = self.peak_bytes.max(self.bytes());
    }

    fn plan_revival(
        &self,
        layer: usize,
        population: &[VacancyState],
        pieces: &[GeneralFastPiece<'_>],
        difficulty: &[PieceDifficulty],
        mode: usize,
    ) -> RevivalDecision {
        if self.area.is_none() && self.count.is_none() {
            return RevivalDecision::NotStagnant;
        }
        if layer.saturating_sub(self.last_improvement_layer) < ARCHIVE_STAGNATION_LAYERS {
            return RevivalDecision::NotStagnant;
        }
        if let Some(last) = self.last_revival_layer {
            if layer.saturating_sub(last) < ARCHIVE_REVIVAL_COOLDOWN {
                return RevivalDecision::NotStagnant;
            }
        }
        if self.revivals_expanded >= MAX_ARCHIVE_REVIVALS {
            return RevivalDecision::Skipped("revivalBudgetExhausted");
        }
        if mode == 8 && population.len() < 2 {
            return RevivalDecision::Skipped("populationTooSmall");
        }
        let candidates: [(&'static str, Option<&(EliteSnapshot, VacancyState)>); 2] =
            if self.revival_ordinal.is_multiple_of(2) {
                [("area", self.area.as_ref()), ("count", self.count.as_ref())]
            } else {
                [("count", self.count.as_ref()), ("area", self.area.as_ref())]
            };
        let mut last_reason = "archiveEmpty";
        for (kind, entry) in candidates {
            let Some((snapshot, state)) = entry else {
                continue;
            };
            if population
                .iter()
                .any(|member| same_state_identity(member, state))
            {
                last_reason = "inPopulation";
                continue;
            }
            if mode == 8 {
                let worst = population
                    .last()
                    .expect("a mode-8 revival population has at least two states");
                let better = if kind == "area" {
                    compare_states(state, worst, pieces, difficulty).is_lt()
                } else {
                    compare_count_states(state, worst, pieces, difficulty).is_lt()
                };
                if !better {
                    last_reason = "notBetterThanWorst";
                    continue;
                }
            }
            return RevivalDecision::Revive {
                kind,
                state: state.clone(),
                fingerprint: snapshot.fingerprint.clone(),
            };
        }
        RevivalDecision::Skipped(last_reason)
    }
}

fn elite_snapshot_heap_bytes(snapshot: &EliteSnapshot) -> usize {
    snapshot
        .fingerprint
        .capacity()
        .saturating_add(
            snapshot
                .inactive_difficulty_sequence
                .capacity()
                .saturating_mul(size_of::<(i128, i128, i64, String)>()),
        )
        .saturating_add(
            snapshot
                .inactive_difficulty_sequence
                .iter()
                .map(|(_, _, _, id)| id.capacity())
                .sum::<usize>(),
        )
        .saturating_add(
            snapshot
                .identity
                .active_placements
                .capacity()
                .saturating_mul(size_of::<(usize, i64, bool, i64, i64)>()),
        )
        .saturating_add(
            snapshot
                .identity
                .inactive
                .capacity()
                .saturating_mul(size_of::<usize>()),
        )
}

#[derive(Default)]
struct RunWork {
    diagnostics: GeneralPersistentVacancyWorkDiagnostics,
}

impl RunWork {
    fn cap(&self, reason: &str) -> String {
        format!("cap: {reason}")
    }

    fn charge_source_features(&mut self, amount: usize) -> Result<(), String> {
        self.diagnostics.source_feature_visits = self
            .diagnostics
            .source_feature_visits
            .saturating_add(amount);
        if self.diagnostics.source_feature_visits > MAX_SOURCE_FEATURE_VISITS {
            return Err(self.cap("source-feature visit budget exhausted"));
        }
        Ok(())
    }

    fn charge_position_sources(&mut self, amount: usize) -> Result<(), String> {
        self.diagnostics.position_source_attempts = self
            .diagnostics
            .position_source_attempts
            .saturating_add(amount);
        if self.diagnostics.position_source_attempts > MAX_POSITION_SOURCE_ATTEMPTS {
            return Err(self.cap("position-source attempt budget exhausted"));
        }
        Ok(())
    }

    fn charge_experimental_pair(&mut self) -> Result<(), String> {
        self.diagnostics.experimental_pair_visits =
            self.diagnostics.experimental_pair_visits.saturating_add(1);
        if self.diagnostics.experimental_pair_visits > MAX_EXPERIMENTAL_PAIR_VISITS {
            return Err(self.cap("experimental pair-visit budget exhausted"));
        }
        Ok(())
    }

    fn charge_validator_audit(&mut self, complete: bool) -> Result<(), String> {
        if complete {
            if self.diagnostics.complete_audits >= MAX_COMPLETE_AUDITS {
                return Err(self.cap("complete-audit budget exhausted"));
            }
            self.diagnostics.complete_audits += 1;
        } else {
            if self.diagnostics.partial_audits >= MAX_PARTIAL_AUDITS {
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
            > MAX_VALIDATOR_COLLISION_BUILDS
        {
            return Err(self.cap("validator collision-build budget exhausted"));
        }
        if self
            .diagnostics
            .validator_pair_visits
            .saturating_add(pair_visits)
            > MAX_VALIDATOR_PAIR_VISITS
        {
            return Err(self.cap("validator pair-visit budget exhausted"));
        }
        if self
            .diagnostics
            .transformed_collision_vertices
            .saturating_add(collision_vertices)
            > MAX_TRANSFORMED_COLLISION_VERTICES
        {
            return Err(self.cap("transformed collision-vertex budget exhausted"));
        }
        if self
            .diagnostics
            .clipper_input_vertices
            .saturating_add(input_vertices)
            > MAX_CLIPPER_INPUT_VERTICES
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

pub(super) fn run_persistent_vacancy_population(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    _relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
    mode: usize,
) -> GeneralPersistentVacancyDiagnostics {
    let mut diagnostics = GeneralPersistentVacancyDiagnostics {
        mode,
        seed_domain: PERSISTENT_VACANCY_SEED_DOMAIN,
        target_depth_mm: TARGET_DEPTH_MM,
        parent_source,
        ..GeneralPersistentVacancyDiagnostics::default()
    };
    let mut work = RunWork::default();
    match run_population(
        pieces,
        fast_settings,
        parent,
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
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    mode: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<Option<(VacancyState, f64)>, String> {
    if !matches!(mode, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8) {
        return Err("persistent vacancy mode must be 1, 2, 3, 4, 5, 6, 7, or 8".to_owned());
    }
    if pieces.len() != 61 {
        return Err("persistent vacancy experiment is pinned to Mixed-61".to_owned());
    }
    if parent.final_placements.len() != pieces.len() {
        return Err("persistent vacancy parent is not a complete exact-valid layout".to_owned());
    }
    let parent_fast = diagnostic_fast_placements(&parent.final_placements);
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
    let baseline = relaxed_state_from_diagnostics(pieces, &parent.final_placements)?;
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
    let mut retained_carryovers = BTreeSet::new();
    let mut archive = matches!(mode, 7 | 8).then(TopologyArchive::new);
    for layer in 0..MAX_LAYERS {
        // Modes 7/8 plan a revival before the entering-population hash so the
        // hash always reflects the population that is actually expanded
        // (mode 8 swaps the comparator-worst entering slot in place).
        let mut layer_archive = None;
        let mut revival_parent: Option<VacancyState> = None;
        if let Some(archive_state) = archive.as_mut() {
            let layers_since_improvement =
                layer.saturating_sub(archive_state.last_improvement_layer);
            let mut row = GeneralPersistentVacancyArchiveLayerDiagnostics {
                layers_since_improvement,
                ..GeneralPersistentVacancyArchiveLayerDiagnostics::default()
            };
            match archive_state.plan_revival(layer, &population, pieces, &difficulty, mode) {
                RevivalDecision::NotStagnant => {}
                RevivalDecision::Skipped(reason) => {
                    archive_state.revivals_skipped =
                        archive_state.revivals_skipped.saturating_add(1);
                    row.revival_attempted = true;
                    row.skipped_reason = Some(reason.to_owned());
                }
                RevivalDecision::Revive {
                    kind,
                    state,
                    fingerprint,
                } => {
                    archive_state.revivals_expanded =
                        archive_state.revivals_expanded.saturating_add(1);
                    archive_state.last_revival_layer = Some(layer);
                    archive_state.revival_ordinal = archive_state.revival_ordinal.saturating_add(1);
                    row.revival_attempted = true;
                    row.revival_expanded = true;
                    row.revival_kind = Some(kind.to_owned());
                    row.revived_state_fingerprint = Some(fingerprint);
                    if mode == 8 {
                        let replaced_index = population.len() - 1;
                        row.replaced_state_fingerprint =
                            Some(state_fingerprint(&population[replaced_index], pieces));
                        population[replaced_index] = state;
                    } else {
                        revival_parent = Some(state);
                    }
                }
            }
            layer_archive = Some(row);
        }
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
        let ordinary_children_count = children.len();
        if let Some(revived_state) = &revival_parent {
            let revival_row_index = parent_selections.len();
            expand_parent(
                revived_state,
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
            if let Some(row) = parent_selections.get_mut(revival_row_index) {
                row.revived = Some(true);
            }
        }
        let revival_child_fingerprints = children[ordinary_children_count..]
            .iter()
            .map(|state| state_fingerprint(state, pieces))
            .collect::<BTreeSet<_>>();
        if let (Some(archive_state), Some(row)) = (archive.as_mut(), layer_archive.as_mut()) {
            let generated = children.len().saturating_sub(ordinary_children_count);
            row.revival_children_generated = generated;
            archive_state.revival_children_generated = archive_state
                .revival_children_generated
                .saturating_add(generated);
        }
        if children.is_empty() {
            return Err(format!(
                "persistent vacancy layer {layer} produced no exact-valid child"
            ));
        }
        let selected_piece_ids = selected_piece_ids.into_iter().collect::<Vec<_>>();
        let ordinary_live_state_bytes = state_vec_bytes(&children);
        let carryover_live_state_bytes = state_vec_bytes(&carryover_states);
        let combined_pool_backing_bytes = children
            .len()
            .saturating_add(carryover_states.len())
            .saturating_mul(size_of::<VacancyState>());
        let mut largest_clone_bytes = 0usize;
        for state in children.iter().chain(&carryover_states) {
            let bytes = size_of::<VacancyState>().saturating_add(state_heap_bytes(state));
            largest_clone_bytes = largest_clone_bytes.max(bytes);
        }
        let retained_clone_bytes = largest_clone_bytes.saturating_mul(2);
        preflight_raw_live_memory(
            &population,
            ordinary_live_state_bytes,
            carryover_live_state_bytes,
            retained_clone_bytes,
            combined_pool_backing_bytes,
            archive.as_ref().map_or(0, TopologyArchive::bytes),
            &selected_piece_ids,
            &parent_selections,
            diagnostics,
            work,
        )?;
        // The ordinary child-order hash keeps its cross-mode meaning: it
        // covers exactly the ordinary parents' children. Mode-7 revival
        // children are merged only after that hash is taken.
        let mut revival_children = children.split_off(ordinary_children_count);
        children.sort_by(|first, second| compare_states(first, second, pieces, &difficulty));
        let before_dedup = children.len();
        children.dedup_by(|first, second| same_state_identity(first, second));
        diagnostics.deduplicated_states = diagnostics
            .deduplicated_states
            .saturating_add(before_dedup.saturating_sub(children.len()));
        let ordinary_child_order_hash = child_order_hash(&children, pieces);
        if !revival_children.is_empty() {
            children.append(&mut revival_children);
            children.sort_by(|first, second| compare_states(first, second, pieces, &difficulty));
            let before_merge_dedup = children.len();
            children.dedup_by(|first, second| same_state_identity(first, second));
            diagnostics.deduplicated_states = diagnostics
                .deduplicated_states
                .saturating_add(before_merge_dedup.saturating_sub(children.len()));
        }

        let complete_count = children
            .iter()
            .take_while(|state| state.active.iter().all(|active| *active))
            .count();
        let complete_candidate_order_hash = child_order_hash(&children[..complete_count], pieces);
        diagnostics.complete_states = diagnostics.complete_states.saturating_add(complete_count);
        let mut accepted_complete = None;
        for candidate in children.iter().take(complete_count) {
            match audit_state(candidate, pieces, target_settings, true, work) {
                Ok(_) => {
                    let placements = fast_placements(candidate, pieces, false);
                    let independent =
                        coupled_independent_source_depth(pieces, &placements, target_settings)
                            .map_err(|error| format!("persistent vacancy final depth: {error}"))?;
                    accepted_complete = Some((candidate.clone(), independent));
                    break;
                }
                Err(reason) if !reason.starts_with("cap: ") => {
                    diagnostics.publication_rejections =
                        diagnostics.publication_rejections.saturating_add(1);
                }
                Err(reason) => return Err(reason),
            }
        }
        children.retain(|state| state.active.iter().any(|active| !*active));
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
        if let (Some(archive_state), Some(row)) = (archive.as_mut(), layer_archive.as_mut()) {
            if !revival_child_fingerprints.is_empty() {
                let retained = next
                    .iter()
                    .filter(|state| {
                        revival_child_fingerprints.contains(&state_fingerprint(state, pieces))
                    })
                    .count();
                row.revival_children_retained = retained;
                archive_state.revival_children_retained = archive_state
                    .revival_children_retained
                    .saturating_add(retained);
            }
        }
        let (area_elite, count_elite) = population_elites(&next, pieces, &difficulty);
        let area_snapshot = elite_snapshot(area_elite, pieces, &difficulty);
        let count_snapshot = elite_snapshot(count_elite, pieces, &difficulty);
        let area_improved = update_best_area(&mut best_ever_area, &area_snapshot);
        let count_improved = update_best_count(&mut best_ever_count, &count_snapshot);
        if let Some(archive_state) = archive.as_mut() {
            if area_improved {
                archive_state.area = Some((area_snapshot.clone(), area_elite.clone()));
            }
            if count_improved {
                archive_state.count = Some((count_snapshot.clone(), count_elite.clone()));
            }
            if area_improved || count_improved {
                archive_state.last_improvement_layer = layer;
            }
            archive_state.charge_peak();
            if let Some(row) = layer_archive.as_mut() {
                row.archived_area_updated = area_improved;
                row.archived_count_updated = count_improved;
            }
        }
        let best_ever_area_snapshot = best_ever_area
            .as_ref()
            .expect("the current area elite initializes best-ever history");
        let best_ever_count_snapshot = best_ever_count
            .as_ref()
            .expect("the current count elite initializes best-ever history");
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
            archive: layer_archive,
        };
        preflight_live_memory(
            &population,
            ordinary_live_state_bytes,
            carryover_live_state_bytes,
            retained_clone_bytes,
            combined_pool_backing_bytes,
            archive.as_ref().map_or(0, TopologyArchive::bytes),
            diagnostics,
            &layer_diagnostics,
            work,
        )?;
        charge_retained_memory(
            &next,
            archive.as_ref().map_or(0, TopologyArchive::bytes),
            diagnostics,
            &layer_diagnostics,
            work,
        )?;
        diagnostics.layers.push(layer_diagnostics);
        diagnostics.layers_completed = layer + 1;
        if let Some(archive_state) = archive.as_ref() {
            diagnostics.archive = Some(GeneralPersistentVacancyArchiveDiagnostics {
                stagnation_threshold_layers: ARCHIVE_STAGNATION_LAYERS,
                revival_cooldown_layers: ARCHIVE_REVIVAL_COOLDOWN,
                max_revival_expansions: MAX_ARCHIVE_REVIVALS,
                revival_policy: if mode == 7 {
                    "extraParent".to_owned()
                } else {
                    "swapWorstEntering".to_owned()
                },
                revivals_expanded: archive_state.revivals_expanded,
                revivals_skipped: archive_state.revivals_skipped,
                revival_children_generated: archive_state.revival_children_generated,
                revival_children_retained: archive_state.revival_children_retained,
                archive_peak_bytes: archive_state.peak_bytes,
                final_archived_area_fingerprint: archive_state
                    .area
                    .as_ref()
                    .map(|(snapshot, _)| snapshot.fingerprint.clone()),
                final_archived_count_fingerprint: archive_state
                    .count
                    .as_ref()
                    .map(|(snapshot, _)| snapshot.fingerprint.clone()),
            });
        }
        if let Some(complete) = accepted_complete {
            return Ok(Some(complete));
        }
        retained_carryovers = retained_carryover_fingerprints.into_iter().collect();
        population = next;
    }
    Ok(None)
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
        revived: None,
        slots: Vec::with_capacity(selection.indices.len()),
    };
    for (selected_ordinal, piece_index) in selection.indices.into_iter().enumerate() {
        selected_piece_ids.insert(pieces[piece_index].id.to_owned());
        work.diagnostics.selected_piece_slots =
            work.diagnostics.selected_piece_slots.saturating_add(1);
        if work.diagnostics.selected_piece_slots > MAX_SELECTED_PIECE_SLOTS {
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
        selection_diagnostics
            .slots
            .push(GeneralPersistentVacancySelectionSlotDiagnostics {
                selected_ordinal,
                piece_id: pieces[piece_index].id.to_owned(),
                angle_seed,
                diversity_seed,
            });
        let mut merged = Vec::new();
        for (orientation_ordinal, (rotation_deg, mirrored)) in orientations.into_iter().enumerate()
        {
            work.diagnostics.orientation_streams =
                work.diagnostics.orientation_streams.saturating_add(1);
            if work.diagnostics.orientation_streams > MAX_ORIENTATION_STREAMS {
                return Err(work.cap("orientation-stream budget exhausted"));
            }
            let orientation = RelaxedPlacement {
                input_index: piece_index,
                rotation_deg,
                mirrored,
                translate_x: 0.0,
                translate_y: 0.0,
            };
            let local_collision =
                build_collision(pieces[piece_index], &orientation, settings, work)?;
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
                if work.diagnostics.hazard_queries > MAX_HAZARD_QUERIES {
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
                    if work.diagnostics.proxy_pressure_visits > MAX_PROXY_PRESSURE_VISITS {
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
        let mut placement_keys = BTreeSet::new();
        merged.retain(|proposal| placement_keys.insert(placement_key(&proposal.placement)));
        merged.truncate(FINALISTS_PER_PIECE);
        for finalist in merged {
            work.diagnostics.exact_finalist_rows =
                work.diagnostics.exact_finalist_rows.saturating_add(1);
            if work.diagnostics.exact_finalist_rows > MAX_EXACT_FINALIST_ROWS {
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
    }
    parent_selections.push(selection_diagnostics);
    Ok(())
}

fn compare_proposals(first: &RankedProposal, second: &RankedProposal) -> Ordering {
    first
        .proxy_loss
        .total_cmp(&second.proxy_loss)
        .then_with(|| first.orientation_ordinal.cmp(&second.orientation_ordinal))
        .then_with(|| first.diversity_key.cmp(&second.diversity_key))
        .then_with(|| placement_key(&first.placement).cmp(&placement_key(&second.placement)))
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
    if work.diagnostics.returned_positions > MAX_RETURNED_POSITIONS {
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
    if !terminal_complete && matches!(mode, 5 | 6) && retained != BEAM_WIDTH {
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
    if matches!(mode, 1 | 3 | 7 | 8) {
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
    if work.diagnostics.experimental_collision_builds > MAX_EXPERIMENTAL_COLLISION_BUILDS {
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
    if work.diagnostics.transformed_collision_vertices > MAX_TRANSFORMED_COLLISION_VERTICES {
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
        > MAX_CLIPPER_INPUT_VERTICES
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
    if !matches!(mode, 3 | 4 | 5 | 6 | 7 | 8) || inactive.len() <= 1 {
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
    if matches!(mode, 3 | 4 | 5 | 6 | 7 | 8) {
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

fn update_best_area(best: &mut Option<EliteSnapshot>, candidate: &EliteSnapshot) -> bool {
    if best.as_ref().map_or(true, |current| {
        compare_area_snapshots(candidate, current).is_lt()
    }) {
        *best = Some(candidate.clone());
        return true;
    }
    false
}

fn update_best_count(best: &mut Option<EliteSnapshot>, candidate: &EliteSnapshot) -> bool {
    if best.as_ref().map_or(true, |current| {
        compare_count_snapshots(candidate, current).is_lt()
    }) {
        *best = Some(candidate.clone());
        return true;
    }
    false
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
    archive_bytes: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    pending_layer: &GeneralPersistentVacancyLayerDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    diagnostics.layers.reserve(1);
    let legacy_state_bytes = legacy_state_slice_bytes(population);
    let state_bytes = state_slice_bytes(population)
        .saturating_add(population.len().saturating_mul(size_of::<VacancyState>()));
    let diagnostic_bytes = persistent_diagnostic_bytes(diagnostics)
        .saturating_add(layer_diagnostic_heap_bytes(pending_layer));
    let total_bytes = state_bytes
        .saturating_add(diagnostic_bytes)
        .saturating_add(archive_bytes);
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
    ordinary_live_state_bytes: usize,
    carryover_live_state_bytes: usize,
    retained_clone_bytes: usize,
    combined_pool_backing_bytes: usize,
    archive_bytes: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    pending_layer: &GeneralPersistentVacancyLayerDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    diagnostics.layers.reserve(1);
    let diagnostic_bytes = persistent_diagnostic_bytes(diagnostics)
        .saturating_add(layer_diagnostic_heap_bytes(pending_layer));
    let total_bytes = state_vec_bytes(entering_population)
        .saturating_add(ordinary_live_state_bytes)
        .saturating_add(carryover_live_state_bytes)
        .saturating_add(retained_clone_bytes)
        .saturating_add(combined_pool_backing_bytes)
        .saturating_add(archive_bytes)
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
    ordinary_live_state_bytes: usize,
    carryover_live_state_bytes: usize,
    retained_clone_bytes: usize,
    combined_pool_backing_bytes: usize,
    archive_bytes: usize,
    selected_piece_ids: &[String],
    parent_selections: &[GeneralPersistentVacancyParentSelectionDiagnostics],
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
        .saturating_add(ELITE_DIAGNOSTIC_HEAP_UPPER_BOUND);
    let diagnostic_bytes =
        persistent_diagnostic_bytes(diagnostics).saturating_add(pending_selector_bytes);
    let total_bytes = state_vec_bytes(entering_population)
        .saturating_add(ordinary_live_state_bytes)
        .saturating_add(carryover_live_state_bytes)
        .saturating_add(retained_clone_bytes)
        .saturating_add(combined_pool_backing_bytes)
        .saturating_add(archive_bytes)
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

fn state_vec_bytes(states: &Vec<VacancyState>) -> usize {
    states
        .capacity()
        .saturating_mul(size_of::<VacancyState>())
        .saturating_add(state_slice_bytes(states))
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
    .saturating_add(
        layer
            .elite
            .as_ref()
            .map_or(0, elite_layer_diagnostic_heap_bytes),
    )
    .saturating_add(
        layer
            .archive
            .as_ref()
            .map_or(0, archive_layer_diagnostic_heap_bytes),
    )
}

fn archive_layer_diagnostic_heap_bytes(
    archive: &GeneralPersistentVacancyArchiveLayerDiagnostics,
) -> usize {
    size_of::<GeneralPersistentVacancyArchiveLayerDiagnostics>()
        .saturating_add(archive.revival_kind.as_ref().map_or(0, String::capacity))
        .saturating_add(
            archive
                .revived_state_fingerprint
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(
            archive
                .replaced_state_fingerprint
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(archive.skipped_reason.as_ref().map_or(0, String::capacity))
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
        let (_, mut first) = state_with_two_squares(10.0, 0.0);
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
        charge_retained_memory(&[], 0, &mut diagnostics, &pending, &mut work).unwrap();
        assert_eq!(work.diagnostics.retained_peak_bytes, 0);
        assert!(work.diagnostics.selector_diagnostic_peak_bytes > 0);
        assert_eq!(
            work.diagnostics.total_retained_peak_bytes,
            work.diagnostics.selector_diagnostic_peak_bytes
        );
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
            state_vec_bytes(&vec![state]),
            0,
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
            MAX_RETAINED_BYTES,
            0,
            0,
            0,
            0,
            &[],
            &[],
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
    fn dual_objective_modes_reject_nonterminal_width_changes() {
        assert!(enforce_population_width(6, false, BEAM_WIDTH - 1, 4).is_err());
        assert!(enforce_population_width(5, false, BEAM_WIDTH, 4).is_ok());
        assert!(enforce_population_width(6, true, 1, 4).is_ok());
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
        // The ordinary 8-parent, 40-layer schedule funds 640 selected-piece
        // slots; the archive revival lane of modes 7/8 adds at most 13
        // expansions of 2 slots each, so every downstream ceiling carries the
        // ordinary term plus the revival-lane term.
        assert_eq!(MAX_ARCHIVE_REVIVALS, 13);
        assert_eq!(ORDINARY_SELECTED_PIECE_SLOTS, 640);
        assert_eq!(ARCHIVE_SELECTED_PIECE_SLOTS, 26);
        assert_eq!(MAX_SELECTED_PIECE_SLOTS, 640 + 26);
        assert_eq!(MAX_ORIENTATION_STREAMS, 7_680 + 312);
        assert_eq!(MAX_POSITION_SOURCE_ATTEMPTS, (7_680 + 312) * 529);
        assert_eq!(MAX_RETURNED_POSITIONS, 245_760 + 9_984);
        assert_eq!(MAX_HAZARD_QUERIES, 245_760 + 9_984);
        assert_eq!(MAX_PROXY_PRESSURE_VISITS, (245_760 + 9_984) * 61);
        assert_eq!(MAX_EXACT_FINALIST_ROWS, 5_120 + 208);
        assert_eq!(
            MAX_EXPERIMENTAL_COLLISION_BUILDS,
            61 + (7_680 + 312) + (5_120 + 208)
        );
        assert_eq!(MAX_EXPERIMENTAL_PAIR_VISITS, 1_830 + (5_120 + 208) * 60);
        assert_eq!(MAX_VALIDATOR_COLLISION_BUILDS, 105 * 122);
        assert_eq!(MAX_VALIDATOR_PAIR_VISITS, 105 * 3_660);
        assert_eq!(
            MAX_TRANSFORMED_COLLISION_VERTICES,
            (MAX_EXPERIMENTAL_COLLISION_BUILDS + MAX_VALIDATOR_COLLISION_BUILDS) * 512
        );
        assert_eq!(
            MAX_CLIPPER_INPUT_VERTICES,
            2 * 512 * (MAX_EXPERIMENTAL_PAIR_VISITS + MAX_VALIDATOR_PAIR_VISITS)
        );
    }

    fn archived_entry(state: &VacancyState, fingerprint: &str) -> (EliteSnapshot, VacancyState) {
        (
            EliteSnapshot {
                fingerprint: fingerprint.to_owned(),
                inactive_piece_count: state.active.iter().filter(|active| !**active).count(),
                inactive_area_grid2: 0,
                inactive_difficulty_sequence: Vec::new(),
                ejected_material_area_grid2: 0,
                ejected_piece_count: 0,
                active_frontier_grid: 0,
                identity: state_identity(state),
            },
            state.clone(),
        )
    }

    fn archive_test_pieces_and_difficulty(
        polygons: &[PolygonSet],
    ) -> (Vec<GeneralFastPiece<'_>>, Vec<PieceDifficulty>) {
        let pieces = polygons
            .iter()
            .enumerate()
            .map(|(index, polygon)| GeneralFastPiece {
                id: ["a", "b", "c", "d"][index],
                polygon,
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let difficulty = polygons
            .iter()
            .map(|_| PieceDifficulty {
                expanded_area_grid2: 100,
                hull_deficit_grid2: 100,
                minimum_side_grid: 100,
                material_area_grid2: 100,
            })
            .collect::<Vec<_>>();
        (pieces, difficulty)
    }

    #[test]
    fn archive_revival_schedule_is_deterministic_and_bounded() {
        let polygons = vec![square(10.0), square(10.0), square(10.0)];
        let (pieces, difficulty) = archive_test_pieces_and_difficulty(&polygons);
        let population = vec![state_with_active_mask(vec![true, true, false])];
        let archived = state_with_active_mask(vec![true, false, true]);
        let mut archive = TopologyArchive::new();

        // An empty archive never plans a revival.
        assert!(matches!(
            archive.plan_revival(10, &population, &pieces, &difficulty, 7),
            RevivalDecision::NotStagnant
        ));

        archive.area = Some(archived_entry(&archived, "area-fp"));
        // Below the stagnation threshold nothing fires.
        assert!(matches!(
            archive.plan_revival(
                ARCHIVE_STAGNATION_LAYERS - 1,
                &population,
                &pieces,
                &difficulty,
                7
            ),
            RevivalDecision::NotStagnant
        ));
        // At the threshold the area elite is revived.
        match archive.plan_revival(
            ARCHIVE_STAGNATION_LAYERS,
            &population,
            &pieces,
            &difficulty,
            7,
        ) {
            RevivalDecision::Revive {
                kind, fingerprint, ..
            } => {
                assert_eq!(kind, "area");
                assert_eq!(fingerprint, "area-fp");
            }
            _ => panic!("expected a revival at the stagnation threshold"),
        }
        // Cooldown suppresses the next firing until it elapses.
        archive.revivals_expanded = 1;
        archive.revival_ordinal = 1;
        archive.last_revival_layer = Some(ARCHIVE_STAGNATION_LAYERS);
        assert!(matches!(
            archive.plan_revival(
                ARCHIVE_STAGNATION_LAYERS + ARCHIVE_REVIVAL_COOLDOWN - 1,
                &population,
                &pieces,
                &difficulty,
                7
            ),
            RevivalDecision::NotStagnant
        ));
        assert!(matches!(
            archive.plan_revival(
                ARCHIVE_STAGNATION_LAYERS + ARCHIVE_REVIVAL_COOLDOWN,
                &population,
                &pieces,
                &difficulty,
                7
            ),
            RevivalDecision::Revive { .. }
        ));
        // The expansion budget rejects further revivals explicitly.
        archive.revivals_expanded = MAX_ARCHIVE_REVIVALS;
        assert!(matches!(
            archive.plan_revival(30, &population, &pieces, &difficulty, 7),
            RevivalDecision::Skipped("revivalBudgetExhausted")
        ));
    }

    #[test]
    fn archive_revival_alternates_between_area_and_count() {
        let polygons = vec![square(10.0), square(10.0), square(10.0)];
        let (pieces, difficulty) = archive_test_pieces_and_difficulty(&polygons);
        let population = vec![state_with_active_mask(vec![true, true, false])];
        let area_state = state_with_active_mask(vec![true, false, true]);
        let count_state = state_with_active_mask(vec![false, true, true]);
        let mut archive = TopologyArchive::new();
        archive.area = Some(archived_entry(&area_state, "area-fp"));
        archive.count = Some(archived_entry(&count_state, "count-fp"));

        match archive.plan_revival(10, &population, &pieces, &difficulty, 7) {
            RevivalDecision::Revive { kind, .. } => assert_eq!(kind, "area"),
            _ => panic!("expected an even-ordinal area revival"),
        }
        archive.revival_ordinal = 1;
        match archive.plan_revival(10, &population, &pieces, &difficulty, 7) {
            RevivalDecision::Revive { kind, .. } => assert_eq!(kind, "count"),
            _ => panic!("expected an odd-ordinal count revival"),
        }
        // A candidate whose identity is already in the population falls
        // through to the other elite.
        archive.revival_ordinal = 0;
        let population = vec![area_state.clone()];
        match archive.plan_revival(10, &population, &pieces, &difficulty, 7) {
            RevivalDecision::Revive { kind, .. } => assert_eq!(kind, "count"),
            _ => panic!("expected fallthrough to the count elite"),
        }
        // Both candidates in the population produce an explicit skip.
        let population = vec![area_state, count_state];
        assert!(matches!(
            archive.plan_revival(10, &population, &pieces, &difficulty, 7),
            RevivalDecision::Skipped("inPopulation")
        ));
    }

    #[test]
    fn mode_eight_revival_requires_strict_improvement_over_the_worst_slot() {
        let polygons = vec![square(10.0), square(10.0), square(10.0)];
        let (pieces, difficulty) = archive_test_pieces_and_difficulty(&polygons);
        // Two inactive pieces make the archived state worse under the
        // area-first comparator than both population states (one inactive).
        let worse_archived = state_with_active_mask(vec![false, false, true]);
        let mut archive = TopologyArchive::new();
        archive.area = Some(archived_entry(&worse_archived, "area-fp"));
        let population = vec![
            state_with_active_mask(vec![true, true, false]),
            state_with_active_mask(vec![true, false, true]),
        ];
        assert!(matches!(
            archive.plan_revival(10, &population, &pieces, &difficulty, 8),
            RevivalDecision::Skipped("notBetterThanWorst")
        ));
        // A single-state population cannot be swapped.
        let single = vec![state_with_active_mask(vec![true, true, false])];
        assert!(matches!(
            archive.plan_revival(10, &single, &pieces, &difficulty, 8),
            RevivalDecision::Skipped("populationTooSmall")
        ));
        // A strictly better archived state is swapped in under mode 8.
        let better_archived = state_with_active_mask(vec![true, true, true]);
        archive.area = Some(archived_entry(&better_archived, "better-fp"));
        assert!(matches!(
            archive.plan_revival(10, &population, &pieces, &difficulty, 8),
            RevivalDecision::Revive { kind: "area", .. }
        ));
    }

    #[test]
    fn modes_seven_and_eight_reuse_the_rotating_scheduler_and_area_retention() {
        assert_eq!(scheduler_family(7), "hardPlusStatelessRotation");
        assert_eq!(scheduler_family(8), "hardPlusStatelessRotation");
        assert_eq!(selector_ids(&["a", "b", "c", "d"], 0, 7), vec!["b", "a"]);
        assert_eq!(selector_ids(&["a", "b", "c", "d"], 1, 7), vec!["b", "c"]);
        assert_eq!(selector_ids(&["a", "b", "c", "d"], 1, 8), vec!["b", "c"]);

        let (_, first) = state_with_two_squares(20.0, 0.0);
        let (_, second) = state_with_two_squares(25.0, 0.0);
        let (_, third) = state_with_two_squares(30.0, 0.0);
        let polygons = vec![square(10.0), square(10.0)];
        let polygons = polygons[..2].to_vec();
        let (pieces, difficulty) = archive_test_pieces_and_difficulty(&polygons);
        let sorted = vec![first, second, third];
        let (mode3, signatures3) = retain_population(sorted.clone(), &pieces, &difficulty, 3);
        let (mode7, signatures7) = retain_population(sorted.clone(), &pieces, &difficulty, 7);
        let (mode8, signatures8) = retain_population(sorted, &pieces, &difficulty, 8);
        assert_eq!(mode3.len(), mode7.len());
        assert_eq!(signatures3, signatures7);
        assert_eq!(signatures3, signatures8);
        for (left, right) in mode3.iter().zip(mode7.iter()) {
            assert!(same_state_identity(left, right));
        }
        for (left, right) in mode3.iter().zip(mode8.iter()) {
            assert!(same_state_identity(left, right));
        }
    }

    #[test]
    fn archive_bytes_charge_grows_with_archived_states() {
        let mut archive = TopologyArchive::new();
        assert_eq!(archive.bytes(), 0);
        let (_, state) = state_with_two_squares(20.0, 0.0);
        archive.area = Some(archived_entry(&state, "area-fp"));
        let with_area = archive.bytes();
        assert!(with_area > 0);
        archive.count = Some(archived_entry(&state, "count-fp"));
        assert!(archive.bytes() > with_area);
        archive.charge_peak();
        assert_eq!(archive.peak_bytes, archive.bytes());
    }

    #[test]
    fn pinned_parent_fixture_reproduces_the_frozen_fingerprint() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/mixed-61/persistent-vacancy-parent-b9335a72.json"
        );
        let bytes = std::fs::read(path).expect("the pinned parent fixture is committed");
        let fixture: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            fixture["requestSha256"],
            "dfd2ceecf02efe3475e3344dfefbfb2a2a5bd8a673008b449f5689507c933ba1"
        );
        assert_eq!(fixture["reportedDepthMm"], 168.625);
        assert_eq!(fixture["independentDepthMm"], 168.361);
        let placements = fixture["placements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|placement| GeneralFastPlacement {
                piece_id: placement["pieceId"].as_str().unwrap().to_owned(),
                rotation_deg: placement["rotationDeg"].as_f64().unwrap(),
                mirrored: placement["mirrored"].as_bool().unwrap(),
                translate_short_axis: placement["translateShortAxis"].as_f64().unwrap(),
                translate_long_axis: placement["translateLongAxis"].as_f64().unwrap(),
            })
            .collect::<Vec<_>>();
        assert_eq!(placements.len(), 61);
        assert_eq!(
            coupled_fast_placement_fingerprint(&placements),
            EXPECTED_PARENT_FINGERPRINT
        );
        assert_eq!(
            fixture["expectedPlacementFingerprint"],
            EXPECTED_PARENT_FINGERPRINT
        );
    }
}
