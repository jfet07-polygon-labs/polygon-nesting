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
// Mode 11 runs a translation-only exact settling prelude before the target
// initializer: SETTLE_SWEEPS bottom-up passes over all 61 pieces, each
// attempt exploring one orientation stream and exact-confirming candidate
// positions in ascending settle-key order until the first strictly lower
// valid pose. Each settle attempt may exact-confirm up to
// POSITIONS_PER_ORIENTATION candidate rows, so the finalist-row and pair
// ceilings carry an explicit settle term instead of the 8-per-slot
// population term.
const SETTLE_SWEEPS: usize = 3;
const SETTLE_PROBES_PER_ATTEMPT: usize = 64;
const SETTLE_SELECTED_PIECE_SLOTS: usize = SETTLE_SWEEPS * 61;
// Mode 13 rebuilds the layout from an external hint fixture: one guided
// insertion per piece, ranked by grid distance to the hint pose, with at most
// RECONSTRUCTION_ROWS_PER_PIECE exact confirmations per piece.
const RECONSTRUCTION_SELECTED_PIECE_SLOTS: usize = 2 * 61;
const RECONSTRUCTION_ROWS_PER_PIECE: usize = 192;
// Mode 14 alternates one settle sweep with one guillotine group-drop pass per
// compaction round: for a descending ladder of horizontal cuts, every active
// piece above the cut translates downward as one rigid group, so pairs inside
// the group need no re-checking and mutually wedged clusters can harvest
// slack no single-piece move reaches.
const COMPACTION_ROUNDS: usize = 3;
const GROUP_DROP_CUTS: usize = 61;
const GROUP_DROP_PROBES_PER_CUT: usize = 64;
const GROUP_DROP_PAIR_VISITS: usize =
    COMPACTION_ROUNDS * GROUP_DROP_CUTS * GROUP_DROP_PROBES_PER_CUT * 61;
// Mode 15 runs a non-monotone lift/resettle/reinsert lifecycle: each round
// removes the frontier piece plus its nearest neighbors (an adaptive
// neighborhood schedule), lets the survivors resettle into the vacated
// space, reinserts the removed pieces with full orientation freedom, and
// accepts only rounds whose complete result strictly lowers the frontier,
// reverting to the snapshot otherwise. Two settle sweeps run per round (one
// before removal, one on the survivors) plus one initial sweep.
const LNS_ROUNDS: usize = 24;
const LNS_NEIGHBORHOOD_SCHEDULE: [usize; LNS_ROUNDS] = [
    4, 6, 8, 10, 12, 16, 20, 24, 4, 6, 8, 10, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56,
];
const LNS_SETTLE_SWEEPS: usize = 3 * LNS_ROUNDS + 1;
const LNS_SCHEDULE_TOTAL: usize =
    2 * (4 + 6 + 8 + 10 + 12 + 16 + 20 + 24) + (28 + 32 + 36 + 40 + 44 + 48 + 52 + 56);
const LNS_REINSERT_SLOTS: usize = LNS_SCHEDULE_TOTAL
    + LNS_ROUNDS * SEPARATION_RELOCATIONS_PER_ROUND
    + OPTIMIZER_CYCLES * OPTIMIZER_CANDIDATES_PER_PIECE * LNS_SCHEDULE_TOTAL;
// Mode 16 replaces greedy reinsertion with overlap-mediated separation:
// removed pieces return at their old poses (overlaps permitted), then a
// bounded deterministic descent moves one overlapping soft piece at a time
// along the compass ladder, accepting only strict decreases of the
// grid-quantized total exact overlap area, until overlap reaches zero or the
// move budget is exhausted. Only a zero-overlap endpoint may compete for
// acceptance.
const SEPARATION_MOVES_PER_ROUND: usize = 200;
const SEPARATION_RELOCATIONS_PER_ROUND: usize = 12;
// Mode-17 endpoint optimizer: after a round's endpoint is feasible, up to
// OPTIMIZER_CYCLES steepest-descent passes re-place each lifted piece at the
// best of its top OPTIMIZER_CANDIDATES_PER_PIECE candidate poses under the
// full acceptance key, so the endpoint generator optimizes rather than
// merely places.
const OPTIMIZER_CYCLES: usize = 2;
const OPTIMIZER_CANDIDATES_PER_PIECE: usize = 3;
const SEPARATION_PROBES_PER_MOVE: usize = 96;
const SEPARATION_PAIR_VISITS: usize =
    LNS_ROUNDS * SEPARATION_MOVES_PER_ROUND * SEPARATION_PROBES_PER_MOVE * 61;
const ORDINARY_SELECTED_PIECE_SLOTS: usize = MAX_LAYERS * BEAM_WIDTH * SELECTED_PIECES_PER_PARENT;
const ARCHIVE_SELECTED_PIECE_SLOTS: usize = MAX_ARCHIVE_REVIVALS * SELECTED_PIECES_PER_PARENT;
const POPULATION_SELECTED_PIECE_SLOTS: usize =
    ORDINARY_SELECTED_PIECE_SLOTS + ARCHIVE_SELECTED_PIECE_SLOTS;
const LNS_SETTLE_SELECTED_PIECE_SLOTS: usize = LNS_SETTLE_SWEEPS * 61;
const MAX_SELECTED_PIECE_SLOTS: usize = POPULATION_SELECTED_PIECE_SLOTS
    + SETTLE_SELECTED_PIECE_SLOTS
    + RECONSTRUCTION_SELECTED_PIECE_SLOTS
    + LNS_SETTLE_SELECTED_PIECE_SLOTS
    + LNS_REINSERT_SLOTS;
const MAX_ORIENTATION_STREAMS: usize = MAX_SELECTED_PIECE_SLOTS * ORIENTATIONS_PER_PIECE;
const MAX_SOURCE_FEATURE_VISITS: usize = MAX_SELECTED_PIECE_SLOTS * 2 * MAX_SOURCE_FEATURES;
const POSITION_SOURCE_ATTEMPTS_PER_ORIENTATION: usize = 529;
const MAX_POSITION_SOURCE_ATTEMPTS: usize =
    MAX_ORIENTATION_STREAMS * POSITION_SOURCE_ATTEMPTS_PER_ORIENTATION;
const MAX_RETURNED_POSITIONS: usize = MAX_ORIENTATION_STREAMS * POSITIONS_PER_ORIENTATION;
const MAX_HAZARD_QUERIES: usize = MAX_RETURNED_POSITIONS;
const MAX_PROXY_PRESSURE_VISITS: usize = MAX_RETURNED_POSITIONS * 61;
const MAX_EXACT_FINALIST_ROWS: usize = POPULATION_SELECTED_PIECE_SLOTS * FINALISTS_PER_PIECE
    + SETTLE_SELECTED_PIECE_SLOTS * SETTLE_PROBES_PER_ATTEMPT
    + RECONSTRUCTION_SELECTED_PIECE_SLOTS * RECONSTRUCTION_ROWS_PER_PIECE
    + LNS_SETTLE_SELECTED_PIECE_SLOTS * SETTLE_PROBES_PER_ATTEMPT
    + LNS_REINSERT_SLOTS * RECONSTRUCTION_ROWS_PER_PIECE;
// Two initial 61-piece collision builds are funded: the mode-11 settle
// prelude builds the full active state once, and the target initializer
// rebuilds it once.
// The reconstruction lane also builds one hint-orientation collision per
// selected slot for shelf anchoring.
// Three full-state 61-piece collision builds are funded: the settle or
// compaction prelude, the target initializer, and the mode-14 exact
// re-anchor after group drops.
const SEPARATION_COLLISION_BUILDS: usize =
    LNS_ROUNDS * (LNS_REINSERT_SLOTS / 2 + SEPARATION_MOVES_PER_ROUND * SEPARATION_PROBES_PER_MOVE);
const MAX_EXPERIMENTAL_COLLISION_BUILDS: usize = 3 * 61
    + MAX_ORIENTATION_STREAMS
    + MAX_EXACT_FINALIST_ROWS
    + RECONSTRUCTION_SELECTED_PIECE_SLOTS
    + LNS_REINSERT_SLOTS
    + SEPARATION_COLLISION_BUILDS;
const MAX_VALIDATOR_COLLISION_BUILDS: usize = 12_810;
const MAX_EXPERIMENTAL_PAIR_VISITS: usize =
    1_830 + MAX_EXACT_FINALIST_ROWS * 60 + GROUP_DROP_PAIR_VISITS + SEPARATION_PAIR_VISITS;
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
        if matches!(mode, 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18) && population.len() < 2 {
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
            if matches!(mode, 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18) {
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
        .saturating_add(
            snapshot
                .identity
                .last_transition
                .as_ref()
                .map_or(0, |transition| {
                    transition
                        .ejected
                        .capacity()
                        .saturating_mul(size_of::<usize>())
                }),
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
    relaxed_settings: GeneralRelaxedSettings,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_source: Option<String>,
    mode: usize,
) -> GeneralPersistentVacancyDiagnostics {
    let parent_is_pinned = parent_source.is_some();
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
        relaxed_settings.persistent_vacancy_target_depth_mm,
        parent,
        parent_is_pinned,
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
    target_override_mm: Option<f64>,
    parent: &GeneralCoupledSeparatorArmDiagnostics,
    parent_is_pinned: bool,
    mode: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<Option<(VacancyState, f64)>, String> {
    if !matches!(
        mode,
        1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18
    ) {
        return Err("persistent vacancy mode must be between 1 and 18".to_owned());
    }
    // Modes 1-8 are the frozen diagnostic screens: their 165.0 mm target and
    // b9335a72 parent identity are part of the pinned experiment contract.
    // Mode 9 is the descending-target contraction lane: it requires an
    // explicitly pinned exact-valid parent fixture plus an explicit target,
    // and skips only the frozen fingerprint/depth equality pins while keeping
    // full parent validation.
    let target_depth_mm = match (mode, target_override_mm) {
        (9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18, Some(target)) => {
            if !target.is_finite() || target <= 0.0 {
                return Err(
                    "persistent vacancy target depth must be a positive finite value".to_owned(),
                );
            }
            target
        }
        (9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18, None) => {
            return Err(
                "persistent vacancy modes 9-18 require an explicit target depth".to_owned(),
            );
        }
        (_, Some(_)) => {
            return Err("persistent vacancy target depth overrides require modes 9-18".to_owned());
        }
        (_, None) => TARGET_DEPTH_MM,
    };
    if matches!(mode, 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18) && !parent_is_pinned {
        return Err("persistent vacancy modes 9-18 require a pinned parent fixture".to_owned());
    }
    diagnostics.target_depth_mm = target_depth_mm;
    if pieces.len() != 61 {
        return Err("persistent vacancy experiment is pinned to Mixed-61".to_owned());
    }
    if parent.final_placements.len() != pieces.len() {
        return Err("persistent vacancy parent is not a complete exact-valid layout".to_owned());
    }
    let parent_fast = diagnostic_fast_placements(&parent.final_placements);
    if mode != 13 {
        validate_and_measure_placements(pieces, &parent_fast, fast_settings)
            .map_err(|error| format!("persistent vacancy parent validation: {error}"))?;
    }
    let parent_fingerprint = coupled_fast_placement_fingerprint(&parent_fast);
    diagnostics.parent_fingerprint = Some(parent_fingerprint.clone());
    if !matches!(mode, 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18)
        && parent_fingerprint != EXPECTED_PARENT_FINGERPRINT
    {
        return Err(format!(
            "persistent vacancy parent fingerprint mismatch: expected {EXPECTED_PARENT_FINGERPRINT}, got {parent_fingerprint}"
        ));
    }
    if mode != 13 {
        let parent_depth = coupled_independent_source_depth(pieces, &parent_fast, fast_settings)
            .map_err(|error| format!("persistent vacancy parent depth: {error}"))?;
        if matches!(mode, 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18) {
            diagnostics.parent_independent_depth_mm = Some(parent_depth);
        }
        if !matches!(mode, 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18)
            && grid_key(parent_depth) != grid_key(EXPECTED_PARENT_DEPTH_MM)
        {
            return Err(format!(
                "persistent vacancy parent depth mismatch: expected {EXPECTED_PARENT_DEPTH_MM}, got {parent_depth}"
            ));
        }
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
        sheet_long_axis_mm: target_depth_mm,
        ..fast_settings
    };
    let mut baseline = relaxed_state_from_diagnostics_with_target(
        pieces,
        &parent.final_placements,
        target_depth_mm,
    )?;
    if mode == 13 {
        let (state, independent) = reconstruct_from_hints(
            pieces,
            fast_settings,
            target_depth_mm,
            &baseline,
            diagnostics,
            work,
        )?;
        return Ok(Some((state, independent)));
    }
    if matches!(mode, 11 | 12) {
        baseline = settle_baseline(pieces, fast_settings, baseline, diagnostics, work)?;
    }
    if mode == 14 {
        baseline = compact_baseline(pieces, fast_settings, baseline, diagnostics, work)?;
    }
    if mode == 18 {
        baseline = frontier_band_feasibility(pieces, fast_settings, baseline, diagnostics, work)?;
    }
    if matches!(mode, 15 | 16 | 17) {
        baseline = lift_resettle_reinsert(
            pieces,
            fast_settings,
            target_depth_mm,
            baseline,
            matches!(mode, 16 | 17),
            mode == 17,
            diagnostics,
            work,
        )?;
    }
    let (initial, difficulty, inactive_order) = initial_vacancy_state(
        pieces,
        target_settings,
        baseline,
        diagnostics,
        work,
        matches!(mode, 11 | 12 | 14 | 15 | 16 | 17 | 18),
    )?;
    diagnostics.initial_state_fingerprint = Some(state_fingerprint(&initial, pieces));
    diagnostics.initial_active_piece_ids = active_ids(&initial, pieces);
    diagnostics.initial_inactive_piece_ids = inactive_order
        .iter()
        .map(|index| pieces[*index].id.to_owned())
        .collect();
    diagnostics.initial_inactive_order_hash = Some(id_order_hash(&inactive_order, pieces));
    if inactive_order.is_empty() {
        // Modes 11/12 only: the settling prelude already pulled every piece
        // inside the target strip, so the settled state is a complete
        // candidate. It is counted before the audit, must still pass the
        // unchanged dual publication audit, and a non-cap audit failure is
        // recorded as a publication rejection before the arm fails.
        diagnostics.complete_states = diagnostics.complete_states.saturating_add(1);
        if let Err(reason) = audit_state(&initial, pieces, target_settings, true, work) {
            if !reason.starts_with("cap: ") {
                diagnostics.publication_rejections =
                    diagnostics.publication_rejections.saturating_add(1);
            }
            return Err(reason);
        }
        let placements = fast_placements(&initial, pieces, false);
        let independent = coupled_independent_source_depth(pieces, &placements, target_settings)
            .map_err(|error| format!("persistent vacancy settled depth: {error}"))?;
        return Ok(Some((initial, independent)));
    }
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
    let mut archive = matches!(mode, 7 | 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18)
        .then(TopologyArchive::new);
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
                    if matches!(mode, 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18) {
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

#[derive(Clone, Copy)]
struct SettleKey {
    max_y: i64,
    translate_y: i64,
    translate_x: i64,
}

fn settle_key_for(
    collision: &PolygonSet,
    placement: &RelaxedPlacement,
) -> Result<SettleKey, String> {
    let bounds = collision
        .bounds()
        .ok_or_else(|| "settle candidate has empty collision geometry".to_owned())?;
    Ok(SettleKey {
        max_y: grid_key(bounds.max_y),
        translate_y: grid_key(placement.translate_y),
        translate_x: grid_key(placement.translate_x),
    })
}

fn settle_key_less(first: SettleKey, second: SettleKey) -> bool {
    (first.max_y, first.translate_y, first.translate_x)
        < (second.max_y, second.translate_y, second.translate_x)
}

/// Mode-11 exact settling prelude: translation-only, bottom-up drop
/// compaction over every piece of the full exact-valid parent layout, before
/// any target deactivation. Each attempt keeps the piece's current
/// orientation and horizontal position and lowers the piece with a
/// decreasing step ladder (0.512 mm down to 0.001 mm), exact-confirming every
/// probe with full-sheet containment plus zero exact pair intersection
/// against every other piece. This is an endpoint-exact re-placement move,
/// not a swept-motion contract: near-tangent neighbors can form forbidden
/// bands thinner than one step, so a probe may land beyond a band no
/// continuous slide could cross. That matches every other placement operator
/// in this experiment, all of which relocate pieces discontinuously; validity
/// rests entirely on the per-probe exact gates and the final dual
/// publication audit, never on motion continuity.
const SETTLE_STEP_LADDER_MM: [f64; 10] = [
    0.512, 0.256, 0.128, 0.064, 0.032, 0.016, 0.008, 0.004, 0.002, 0.001,
];

fn settle_baseline(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<RelaxedState, String> {
    let mut state = VacancyState {
        collisions: baseline
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                build_collision(pieces[index], placement, fast_settings, work)
                    .map(|collision| Some(Arc::new(collision)))
            })
            .collect::<Result<Vec<_>, _>>()?,
        placements: baseline.placements.clone(),
        active: vec![true; pieces.len()],
        last_transition: None,
    };
    let frontier = |state: &VacancyState| -> i64 {
        state
            .collisions
            .iter()
            .flatten()
            .filter_map(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.max_y))
            .max()
            .unwrap_or(i64::MIN)
    };
    let mut settle = GeneralPersistentVacancySettleDiagnostics {
        sweeps: SETTLE_SWEEPS,
        attempts: 0,
        accepted_moves: 0,
        exact_rows: 0,
        frontier_before_grid: frontier(&state),
        frontier_after_grid: 0,
    };
    let inset = collision_sheet_inset_mm(fast_settings);
    for _sweep in 0..SETTLE_SWEEPS {
        settle_sweep(
            &mut state,
            pieces,
            fast_settings,
            inset,
            false,
            &mut settle,
            work,
        )?;
    }
    settle.frontier_after_grid = frontier(&state);
    diagnostics.settle = Some(settle);
    Ok(RelaxedState {
        placements: state.placements,
        strip_depth_mm: baseline.strip_depth_mm,
    })
}

/// Mode-14 compaction prelude: alternates one per-piece settle sweep with
/// one guillotine group-drop pass per round, then exactly re-anchors the
/// state by rebuilding every collision from its placement and re-verifying
/// all pairs, failing closed on any disagreement. Group drops translate every
/// active piece above a horizontal cut downward as one rigid body, so pairs
/// inside the group are preserved by construction and only group-versus-rest
/// pairs plus containment need exact confirmation.
fn compact_baseline(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<RelaxedState, String> {
    let mut state = VacancyState {
        collisions: baseline
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                build_collision(pieces[index], placement, fast_settings, work)
                    .map(|collision| Some(Arc::new(collision)))
            })
            .collect::<Result<Vec<_>, _>>()?,
        placements: baseline.placements.clone(),
        active: vec![true; pieces.len()],
        last_transition: None,
    };
    let frontier = |state: &VacancyState| -> i64 {
        state
            .collisions
            .iter()
            .flatten()
            .filter_map(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.max_y))
            .max()
            .unwrap_or(i64::MIN)
    };
    let mut settle = GeneralPersistentVacancySettleDiagnostics {
        sweeps: COMPACTION_ROUNDS,
        attempts: 0,
        accepted_moves: 0,
        exact_rows: 0,
        frontier_before_grid: frontier(&state),
        frontier_after_grid: 0,
    };
    let mut group_drop = GeneralPersistentVacancyGroupDropDiagnostics {
        rounds: COMPACTION_ROUNDS,
        cuts_evaluated: 0,
        probes: 0,
        accepted_drops: 0,
        frontier_before_grid: settle.frontier_before_grid,
        frontier_after_grid: 0,
    };
    let inset = collision_sheet_inset_mm(fast_settings);
    for _round in 0..COMPACTION_ROUNDS {
        settle_sweep(
            &mut state,
            pieces,
            fast_settings,
            inset,
            true,
            &mut settle,
            work,
        )?;
        group_drop_pass(&mut state, fast_settings, inset, &mut group_drop, work)?;
    }
    settle.frontier_after_grid = frontier(&state);
    group_drop.frontier_after_grid = settle.frontier_after_grid;
    diagnostics.settle = Some(settle);
    diagnostics.group_drop = Some(group_drop);
    // Exact re-anchor: incremental group translations accumulate f64 sums, so
    // every collision is rebuilt from its placement and every pair re-proved
    // before the compacted state is trusted.
    for index in 0..pieces.len() {
        let rebuilt =
            build_collision(pieces[index], &state.placements[index], fast_settings, work)?;
        if !rebuilt.fits_rect(
            inset,
            inset,
            fast_settings.sheet_short_axis_mm - inset,
            fast_settings.sheet_long_axis_mm - inset,
        ) {
            return Err(format!(
                "compaction re-anchor: piece {} escaped containment",
                pieces[index].id
            ));
        }
        state.collisions[index] = Some(Arc::new(rebuilt));
    }
    for first in 0..pieces.len() {
        for second in (first + 1)..pieces.len() {
            work.charge_experimental_pair()?;
            let a = state.collisions[first]
                .as_ref()
                .ok_or_else(|| "re-anchor missing collision".to_owned())?;
            let b = state.collisions[second]
                .as_ref()
                .ok_or_else(|| "re-anchor missing collision".to_owned())?;
            if exact_intersection_area(a, b, work)? > 0.0 {
                return Err(format!(
                    "compaction re-anchor: pieces {} and {} overlap after group drops",
                    pieces[first].id, pieces[second].id
                ));
            }
        }
    }
    Ok(RelaxedState {
        placements: state.placements,
        strip_depth_mm: baseline.strip_depth_mm,
    })
}

/// One guillotine pass: for each distinct active min-y cut in descending
/// order, the rigid group of all pieces at or above the cut slides downward
/// with the settle step ladder. A probe is legal when every group piece stays
/// inside the full-sheet inset rectangle and no translated group piece
/// exactly intersects any piece outside the group.
fn group_drop_pass(
    state: &mut VacancyState,
    settings: GeneralFastSettings,
    inset: f64,
    diagnostics: &mut GeneralPersistentVacancyGroupDropDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    let min_y_of = |state: &VacancyState, index: usize| -> Option<i64> {
        state.collisions[index]
            .as_ref()
            .and_then(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.min_y))
    };
    let mut cuts = (0..state.active.len())
        .filter(|index| state.active[*index])
        .filter_map(|index| min_y_of(state, index))
        .collect::<Vec<_>>();
    cuts.sort_unstable();
    cuts.dedup();
    cuts.reverse();
    cuts.truncate(GROUP_DROP_CUTS);
    for cut in cuts {
        diagnostics.cuts_evaluated += 1;
        let group = (0..state.active.len())
            .filter(|index| state.active[*index])
            .filter(|index| min_y_of(state, *index).is_some_and(|min_y| min_y >= cut))
            .collect::<Vec<_>>();
        if group.is_empty() {
            continue;
        }
        let in_group = {
            let mut mask = vec![false; state.active.len()];
            for index in &group {
                mask[*index] = true;
            }
            mask
        };
        let mut probes = 0usize;
        'ladder: for step in SETTLE_STEP_LADDER_MM {
            loop {
                if probes >= GROUP_DROP_PROBES_PER_CUT {
                    break 'ladder;
                }
                probes += 1;
                diagnostics.probes += 1;
                let mut legal = true;
                let mut translated = Vec::with_capacity(group.len());
                for index in &group {
                    let collision = state.collisions[*index]
                        .as_ref()
                        .ok_or_else(|| "group drop missing collision".to_owned())?;
                    let bounds = collision
                        .bounds()
                        .ok_or_else(|| "group drop empty collision".to_owned())?;
                    if bounds.min_y - step < inset {
                        legal = false;
                        break;
                    }
                    let moved = collision
                        .translated(0.0, -step)
                        .map_err(|error| format!("group drop translation: {error}"))?;
                    translated.push((*index, moved));
                }
                if legal {
                    'pairs: for (_, moved) in &translated {
                        for fixed_index in 0..state.active.len() {
                            if !state.active[fixed_index] || in_group[fixed_index] {
                                continue;
                            }
                            work.charge_experimental_pair()?;
                            let fixed = state.collisions[fixed_index]
                                .as_ref()
                                .ok_or_else(|| "group drop missing fixed collision".to_owned())?;
                            if exact_intersection_area(moved, fixed, work)? > 0.0 {
                                legal = false;
                                break 'pairs;
                            }
                        }
                    }
                }
                if !legal {
                    break;
                }
                for (index, moved) in translated {
                    state.placements[index].translate_y -= step;
                    state.collisions[index] = Some(Arc::new(moved));
                }
                diagnostics.accepted_drops += 1;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// Mode-15 non-monotone lifecycle: lift the frontier piece with its nearest
/// neighborhood, resettle the survivors into the vacated space, reinsert the
/// removed pieces with full orientation freedom, and accept only rounds
/// whose complete result strictly lowers the frontier, reverting to the
/// round snapshot otherwise. Every intermediate state is exact-valid
/// (removal cannot invalidate, every reinsertion passes the exact gates),
/// motion is deliberately non-monotone for the lifted pieces, and every
/// selection derives from geometry and stable identifiers only.
fn lift_resettle_reinsert(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    target_depth_mm: f64,
    baseline: RelaxedState,
    separation: bool,
    vacancy_transport: bool,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<RelaxedState, String> {
    // The lifecycle works at full-sheet settings so lifted pieces can park
    // high transiently inside a round (the essential non-monotone freedom);
    // the requested target only gates the downstream deactivation and the
    // dual publication audit. The target's grid value additionally salts the
    // walk seed, so a caller can restart a stalled parent onto a distinct
    // deterministic walk by micro-varying the target - a replayable
    // multi-start without any hidden state.
    let target_salt = grid_key(target_depth_mm) as u64;
    let work_settings = fast_settings;
    let mut state = VacancyState {
        collisions: baseline
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                build_collision(pieces[index], placement, work_settings, work)
                    .map(|collision| Some(Arc::new(collision)))
            })
            .collect::<Result<Vec<_>, _>>()?,
        placements: baseline.placements.clone(),
        active: vec![true; pieces.len()],
        last_transition: None,
    };
    let frontier = |state: &VacancyState| -> i64 {
        state
            .collisions
            .iter()
            .flatten()
            .filter_map(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.max_y))
            .max()
            .unwrap_or(i64::MIN)
    };
    // Lexicographic acceptance key: the frontier first, then the sum of all
    // piece frontiers. Plateau rounds that keep the frontier but thin the
    // top band are accepted, progressively draining the band until the
    // frontier itself can drop; the pair strictly decreases on every
    // accepted round, so the walk terminates.
    let depth_key = |state: &VacancyState| -> (i64, i128, i128) {
        let mut sum: i128 = 0;
        let mut max = i64::MIN;
        for collision in state.collisions.iter().flatten() {
            if let Some(bounds) = collision.bounds() {
                let key = grid_key(bounds.max_y);
                sum += i128::from(key);
                max = max.max(key);
            }
        }
        // Vacancy-transport signal: trapped-void cells become the middle key
        // so an endpoint that lifts a piece but drains a trapped void toward
        // the top-connected region reads as progress; the piece-centric keys
        // cannot see that trade.
        let voids = if vacancy_transport {
            i128::try_from(trapped_void_cells(state, fast_settings, max)).unwrap_or(i128::MAX)
        } else {
            0
        };
        (max, voids, sum)
    };
    let mut settle = GeneralPersistentVacancySettleDiagnostics {
        sweeps: LNS_SETTLE_SWEEPS,
        attempts: 0,
        accepted_moves: 0,
        exact_rows: 0,
        frontier_before_grid: frontier(&state),
        frontier_after_grid: 0,
    };
    let mut lns = GeneralPersistentVacancyLnsDiagnostics {
        rounds: LNS_ROUNDS,
        rounds_accepted: 0,
        rounds_reverted: 0,
        reinsertions: 0,
        reinsert_failures: 0,
        separation_moves: 0,
        separation_probes: 0,
        separation_zero_overlap: 0,
        separation_recruits: 0,
        separation_pair_moves: 0,
        separation_weight_bumps: 0,
        separation_relocations: 0,
        rounds_wandered: 0,
        optimizer_improvements: 0,
        frontier_before_grid: settle.frontier_before_grid,
        frontier_after_grid: 0,
    };
    let inset = collision_sheet_inset_mm(work_settings);
    let hazard_catalog = Arc::new(
        JaguaHazardCatalog::new(pieces, work_settings)
            .map_err(|error| format!("lns hazard catalog: {error}"))?,
    );
    let lns_seed = parent_seed_key(&state, pieces) ^ target_salt;
    settle_sweep(
        &mut state,
        pieces,
        work_settings,
        inset,
        true,
        &mut settle,
        work,
    )?;
    let mut recon = GeneralPersistentVacancyReconstructionDiagnostics::default();
    recon.rows_per_piece_cap = RECONSTRUCTION_ROWS_PER_PIECE;
    // Record-to-record travel: a round endpoint within the deterministic
    // tolerance of the entry key is kept as the wander state so later rounds
    // explore from it, while the best state ever seen is tracked separately
    // and returned, so the published result can never regress.
    const LNS_TOLERANCE_GRID: [i128; LNS_ROUNDS] = [
        0, 2_000, 4_000, 8_000, 0, 2_000, 4_000, 8_000, 0, 2_000, 4_000, 8_000, 0, 2_000, 4_000,
        8_000, 0, 2_000, 4_000, 8_000, 0, 2_000, 4_000, 8_000,
    ];
    const LNS_FRONTIER_TOLERANCE_GRID: [i64; LNS_ROUNDS] = [
        0, 500, 1_000, 2_000, 0, 500, 1_000, 2_000, 0, 500, 1_000, 2_000, 0, 500, 1_000, 2_000, 0,
        500, 1_000, 2_000, 0, 500, 1_000, 2_000,
    ];
    let mut best_state = state.clone();
    let mut best_key = depth_key(&state);
    // Tabu memory: wander endpoints whose semantic fingerprint was already
    // visited in this walk are reverted, breaking the deterministic limit
    // cycles that otherwise trap the record-to-record traversal.
    let mut visited = BTreeSet::new();
    visited.insert(state_fingerprint(&state, pieces));
    for (round, neighborhood) in LNS_NEIGHBORHOOD_SCHEDULE.into_iter().enumerate() {
        let snapshot = state.clone();
        let entry_key = depth_key(&state);
        // Frontier piece: the round-th deepest active piece (modulo four),
        // ties by stable ID, so consecutive rounds attack different members
        // of the frontier band instead of retrying one piece.
        let mut by_depth = (0..pieces.len())
            .filter(|index| state.active[*index])
            .filter_map(|index| {
                state.collisions[index]
                    .as_ref()
                    .and_then(|collision| collision.bounds())
                    .map(|bounds| (grid_key(bounds.max_y), index))
            })
            .collect::<Vec<_>>();
        by_depth.sort_by(|first, second| {
            second
                .0
                .cmp(&first.0)
                .then_with(|| pieces[first.1].id.cmp(pieces[second.1].id))
        });
        let Some(frontier_piece) = by_depth
            .get(round % 4)
            .or_else(|| by_depth.first())
            .map(|(_, index)| *index)
        else {
            break;
        };
        let frontier_center = state.collisions[frontier_piece]
            .as_ref()
            .and_then(|collision| collision.bounds())
            .map(|bounds| {
                (
                    grid_key((bounds.min_x + bounds.max_x) * 0.5),
                    grid_key((bounds.min_y + bounds.max_y) * 0.5),
                )
            })
            .ok_or_else(|| "frontier piece has no bounds".to_owned())?;
        let mut by_distance = (0..pieces.len())
            .filter(|index| state.active[*index] && *index != frontier_piece)
            .filter_map(|index| {
                state.collisions[index]
                    .as_ref()
                    .and_then(|collision| collision.bounds())
                    .map(|bounds| {
                        let center_x = grid_key((bounds.min_x + bounds.max_x) * 0.5);
                        let center_y = grid_key((bounds.min_y + bounds.max_y) * 0.5);
                        let distance = center_x
                            .abs_diff(frontier_center.0)
                            .saturating_add(center_y.abs_diff(frontier_center.1));
                        (distance, index)
                    })
            })
            .collect::<Vec<_>>();
        by_distance.sort_by(|first, second| {
            first
                .0
                .cmp(&second.0)
                .then_with(|| pieces[first.1].id.cmp(pieces[second.1].id))
        });
        let mut removed = vec![frontier_piece];
        removed.extend(
            by_distance
                .into_iter()
                .take(neighborhood.saturating_sub(1))
                .map(|(_, index)| index),
        );
        // Old poses are the reinsertion hints; removal itself cannot
        // invalidate the remaining exact-valid layout.
        let hints = RelaxedState {
            placements: state.placements.clone(),
            strip_depth_mm: work_settings.sheet_long_axis_mm,
        };
        for index in &removed {
            state.active[*index] = false;
            state.collisions[*index] = None;
        }
        settle_sweep(
            &mut state,
            pieces,
            work_settings,
            inset,
            true,
            &mut settle,
            work,
        )?;
        // Reinsert in descending material area, ties by stable ID, so large
        // pieces claim space first.
        let mut reinsert_order = removed.clone();
        reinsert_order.sort_by(|first, second| {
            let area = |index: usize| grid_key(pieces[index].polygon.area_mm2());
            area(*second)
                .cmp(&area(*first))
                .then_with(|| pieces[*first].id.cmp(pieces[*second].id))
        });
        let mut failed = false;
        if separation {
            failed = !overlap_mediated_reinsert(
                pieces,
                work_settings,
                &hints,
                &mut state,
                &removed,
                &hazard_catalog,
                round,
                lns_seed,
                &mut recon,
                &mut lns,
                work,
            )?;
        } else {
            for (slot, piece_index) in reinsert_order.into_iter().enumerate() {
                let mut screen = JaguaHazardIndex::from_catalog_active(
                    pieces,
                    work_settings,
                    work_settings.sheet_long_axis_mm,
                    &state.placements.iter().map(hazard_pose).collect::<Vec<_>>(),
                    &state.active,
                    &hazard_catalog,
                )
                .map_err(|error| format!("lns hazard screen index: {error}"))?;
                let placed = reconstruct_insert_piece(
                    pieces,
                    work_settings,
                    &hints,
                    &mut state,
                    lns_seed,
                    200 + round * 32 + slot,
                    piece_index,
                    true,
                    Some(&mut screen),
                    &mut recon,
                    work,
                )?;
                if placed {
                    lns.reinsertions += 1;
                } else {
                    failed = true;
                    lns.reinsert_failures += 1;
                    break;
                }
            }
        }
        if failed {
            state = snapshot;
            lns.rounds_reverted += 1;
            continue;
        }
        // Endpoint optimizer: steepest-descent re-placement of the lifted
        // pieces under the full acceptance key. Each pass removes one lifted
        // piece, evaluates its top candidate poses by the complete key, and
        // keeps the best strictly improving pose; passes repeat until no
        // piece improves or the cycle budget is exhausted.
        if vacancy_transport {
            for _cycle in 0..OPTIMIZER_CYCLES {
                let mut any_improved = false;
                for lifted in &removed {
                    let index = *lifted;
                    if !state.active[index] {
                        continue;
                    }
                    let entry = depth_key(&state);
                    let saved_placement = state.placements[index].clone();
                    let saved_collision = state.collisions[index].clone();
                    state.active[index] = false;
                    state.collisions[index] = None;
                    let mut screen = JaguaHazardIndex::from_catalog_active(
                        pieces,
                        work_settings,
                        work_settings.sheet_long_axis_mm,
                        &state.placements.iter().map(hazard_pose).collect::<Vec<_>>(),
                        &state.active,
                        &hazard_catalog,
                    )
                    .map_err(|error| format!("optimizer screen index: {error}"))?;
                    let mut best_pose: Option<((i64, i128, i128), RelaxedPlacement, PolygonSet)> =
                        None;
                    for attempt in 0..OPTIMIZER_CANDIDATES_PER_PIECE {
                        let placed = reconstruct_insert_piece(
                            pieces,
                            work_settings,
                            &hints,
                            &mut state,
                            lns_seed,
                            1_000 + round * 64 + attempt * 8,
                            index,
                            true,
                            Some(&mut screen),
                            &mut recon,
                            work,
                        )?;
                        if !placed {
                            break;
                        }
                        let key = depth_key(&state);
                        let placement = state.placements[index].clone();
                        let collision = state.collisions[index]
                            .clone()
                            .ok_or_else(|| "optimizer missing collision".to_owned())?;
                        if best_pose
                            .as_ref()
                            .is_none_or(|(best_key, _, _)| key < *best_key)
                        {
                            best_pose = Some((key, placement, Arc::unwrap_or_clone(collision)));
                        }
                        state.active[index] = false;
                        state.collisions[index] = None;
                    }
                    match best_pose {
                        Some((key, placement, collision)) if key < entry => {
                            state.placements[index] = placement;
                            state.collisions[index] = Some(Arc::new(collision));
                            state.active[index] = true;
                            any_improved = true;
                            lns.optimizer_improvements =
                                lns.optimizer_improvements.saturating_add(1);
                        }
                        _ => {
                            state.placements[index] = saved_placement;
                            state.collisions[index] = saved_collision;
                            state.active[index] = true;
                        }
                    }
                }
                if !any_improved {
                    break;
                }
            }
        }
        // Post-endpoint settle: shelved and separated pieces drop into the
        // voids the rearrangement drained toward the top-connected region
        // before the acceptance key is measured; without this, every shelf
        // landing reads as a frontier regression and vacancy transport is
        // invisible to the key.
        settle_sweep(
            &mut state,
            pieces,
            work_settings,
            inset,
            true,
            &mut settle,
            work,
        )?;
        let endpoint_key = depth_key(&state);
        if endpoint_key < best_key {
            best_state = state.clone();
            best_key = endpoint_key;
        }
        let tolerance = LNS_TOLERANCE_GRID[round];
        let frontier_tolerance = LNS_FRONTIER_TOLERANCE_GRID[round];
        // Trapped-void wander tolerance: up to 50 cells (about 200 mm2 at
        // the 2 mm raster) of transient void regression per tolerant round.
        let void_tolerance: i128 = if LNS_TOLERANCE_GRID[round] > 0 { 50 } else { 0 };
        let within_tolerance = endpoint_key.0 <= entry_key.0.saturating_add(frontier_tolerance)
            && endpoint_key.1 <= entry_key.1.saturating_add(void_tolerance)
            && endpoint_key.2 <= entry_key.2.saturating_add(tolerance);
        let fresh = visited.insert(state_fingerprint(&state, pieces));
        if endpoint_key < entry_key && fresh {
            lns.rounds_accepted += 1;
        } else if within_tolerance && fresh {
            lns.rounds_wandered = lns.rounds_wandered.saturating_add(1);
        } else {
            state = snapshot;
            lns.rounds_reverted += 1;
        }
    }
    state = best_state;
    settle.frontier_after_grid = frontier(&state);
    lns.frontier_after_grid = settle.frontier_after_grid;
    diagnostics.settle = Some(settle);
    diagnostics.lns = Some(lns);
    if recon.insertions > 0 || recon.exact_rows > 0 {
        diagnostics.reconstruction = Some(recon.clone());
    }
    // Exact re-anchor before the state is trusted, mirroring mode 14.
    for index in 0..pieces.len() {
        let rebuilt =
            build_collision(pieces[index], &state.placements[index], work_settings, work)?;
        if !rebuilt.fits_rect(
            inset,
            inset,
            work_settings.sheet_short_axis_mm - inset,
            work_settings.sheet_long_axis_mm - inset,
        ) {
            return Err(format!(
                "lns re-anchor: piece {} escaped containment",
                pieces[index].id
            ));
        }
        state.collisions[index] = Some(Arc::new(rebuilt));
    }
    for first in 0..pieces.len() {
        for second in (first + 1)..pieces.len() {
            work.charge_experimental_pair()?;
            let a = state.collisions[first]
                .as_ref()
                .ok_or_else(|| "lns re-anchor missing collision".to_owned())?;
            let b = state.collisions[second]
                .as_ref()
                .ok_or_else(|| "lns re-anchor missing collision".to_owned())?;
            if exact_intersection_area(a, b, work)? > 0.0 {
                return Err(format!(
                    "lns re-anchor: pieces {} and {} overlap",
                    pieces[first].id, pieces[second].id
                ));
            }
        }
    }
    Ok(RelaxedState {
        placements: state.placements,
        strip_depth_mm: baseline.strip_depth_mm,
    })
}

/// Mode-16 overlap-mediated reinsertion: removed pieces return at their old
/// poses with overlaps permitted, then a bounded deterministic descent moves
/// one overlapping soft piece at a time along the compass ladder, accepting
/// only strict decreases of the grid-quantized total exact overlap area.
/// Returns true only when total overlap reaches exactly zero, so every
/// competing endpoint is exact-valid; a nonzero residual reports failure and
/// the caller reverts the round snapshot.
#[allow(clippy::too_many_arguments)]
fn overlap_mediated_reinsert(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    hints: &RelaxedState,
    state: &mut VacancyState,
    removed: &[usize],
    hazard_catalog: &Arc<JaguaHazardCatalog>,
    round: usize,
    lns_seed: u64,
    recon: &mut GeneralPersistentVacancyReconstructionDiagnostics,
    lns: &mut GeneralPersistentVacancyLnsDiagnostics,
    work: &mut RunWork,
) -> Result<bool, String> {
    const SEPARATION_DIRECTIONS: [(f64, f64); 8] = [
        (0.0, -1.0),
        (0.0, 1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (0.7071067811865476, 0.7071067811865476),
        (-0.7071067811865476, 0.7071067811865476),
        (0.7071067811865476, -0.7071067811865476),
        (-0.7071067811865476, -0.7071067811865476),
    ];
    const SEPARATION_RADII_MM: [f64; 12] = [
        0.256, 0.512, 1.024, 2.048, 3.072, 4.096, 6.144, 8.192, 12.288, 16.384, 24.576, 32.768,
    ];
    let inset = collision_sheet_inset_mm(settings);
    // Soft pieces return at their hint poses, overlaps permitted.
    for index in removed {
        let placement = hints.placements[*index].clone();
        let collision = build_collision(pieces[*index], &placement, settings, work)?;
        state.placements[*index] = placement;
        state.active[*index] = true;
        state.collisions[*index] = Some(Arc::new(collision));
        lns.reinsertions += 1;
    }
    let quantized = |area: f64| -> i128 { (area * 1_000_000.0).round() as i128 };
    // Guided pair weights: pairs that stay overlapping when the descent has
    // no strictly improving move get their weight incremented, so later
    // moves may trade a low-weight overlap increase for a high-weight
    // decrease and cross ridges the unweighted objective cannot. Overlap
    // zero remains the only publication condition and is weight-independent.
    let mut pair_weights: BTreeMap<(usize, usize), i128> = BTreeMap::new();
    let weight_of = |weights: &BTreeMap<(usize, usize), i128>, a: usize, b: usize| -> i128 {
        let key = if a < b { (a, b) } else { (b, a) };
        *weights.get(&key).unwrap_or(&1)
    };
    let piece_overlap = |state: &VacancyState,
                         index: usize,
                         collision: &PolygonSet,
                         weights: &BTreeMap<(usize, usize), i128>,
                         work: &mut RunWork|
     -> Result<(i128, i128), String> {
        let mut weighted = 0i128;
        let mut raw = 0i128;
        for other in 0..pieces.len() {
            if other == index || !state.active[other] {
                continue;
            }
            work.charge_experimental_pair()?;
            let fixed = state.collisions[other]
                .as_ref()
                .ok_or_else(|| "separation missing collision".to_owned())?;
            let overlap = quantized(exact_intersection_area(collision, fixed, work)?);
            raw += overlap;
            weighted += overlap.saturating_mul(weight_of(weights, index, other));
        }
        Ok((weighted, raw))
    };
    let mut soft = removed.to_vec();
    soft.sort_by(|first, second| pieces[*first].id.cmp(pieces[*second].id));
    let mut relocations = 0usize;
    for _move in 0..SEPARATION_MOVES_PER_ROUND {
        let pair_moves_before = lns.separation_pair_moves;
        // Pick the soft piece with the largest current overlap, ties by ID.
        let mut worst: Option<(i128, usize)> = None;
        for index in &soft {
            let collision = state.collisions[*index]
                .as_ref()
                .ok_or_else(|| "separation missing soft collision".to_owned())?
                .clone();
            let (overlap, _raw) = piece_overlap(state, *index, &collision, &pair_weights, work)?;
            if overlap > 0 {
                let candidate = (overlap, *index);
                worst = Some(match worst {
                    None => candidate,
                    Some(current) => {
                        if candidate.0 > current.0
                            || (candidate.0 == current.0
                                && pieces[candidate.1].id < pieces[current.1].id)
                        {
                            candidate
                        } else {
                            current
                        }
                    }
                });
            }
        }
        let Some((current_overlap, index)) = worst else {
            lns.separation_zero_overlap = lns.separation_zero_overlap.saturating_add(1);
            return Ok(true);
        };
        // Best strict-improvement probe: rotational deltas first (they
        // resolve squeezed configurations translations cannot), then the
        // compass translation ladder, all under one probe budget.
        const SEPARATION_ROTATIONS_DEG: [f64; 8] = [-0.5, 0.5, -1.0, 1.0, -2.5, 2.5, -5.0, 5.0];
        let mut best: Option<(i128, RelaxedPlacement, PolygonSet)> = None;
        let mut probes = 0usize;
        let mut candidates_iter: Vec<(f64, f64, f64)> = SEPARATION_ROTATIONS_DEG
            .iter()
            .map(|delta| (0.0, 0.0, *delta))
            .collect();
        for radius in SEPARATION_RADII_MM {
            for (direction_x, direction_y) in SEPARATION_DIRECTIONS {
                candidates_iter.push((radius * direction_x, radius * direction_y, 0.0));
            }
        }
        'probe: for (offset_x, offset_y, rotation_delta) in candidates_iter {
            {
                if probes >= SEPARATION_PROBES_PER_MOVE {
                    break 'probe;
                }
                probes += 1;
                lns.separation_probes = lns.separation_probes.saturating_add(1);
                let mut candidate = state.placements[index].clone();
                candidate.translate_x += offset_x;
                candidate.translate_y += offset_y;
                candidate.rotation_deg += rotation_delta;
                let collision = build_collision(pieces[index], &candidate, settings, work)?;
                if !collision.fits_rect(
                    inset,
                    inset,
                    settings.sheet_short_axis_mm - inset,
                    settings.sheet_long_axis_mm - inset,
                ) {
                    continue;
                }
                let (overlap, _raw) = piece_overlap(state, index, &collision, &pair_weights, work)?;
                if overlap < current_overlap
                    && best
                        .as_ref()
                        .is_none_or(|(best_overlap, _, _)| overlap < *best_overlap)
                {
                    best = Some((overlap, candidate, collision));
                    if overlap == 0 {
                        break 'probe;
                    }
                }
            }
        }
        // Coordinated pair fallback: before recruiting, try moving the stuck
        // piece and its worst-overlap partner simultaneously in opposite
        // directions along their centroid axis - the move that resolves two
        // pieces squeezed between anchors, which no unilateral probe can.
        let mut best = best;
        if best.is_none() {
            let stuck_collision = state.collisions[index]
                .as_ref()
                .ok_or_else(|| "separation missing stuck collision".to_owned())?
                .clone();
            let mut worst_partner: Option<(i128, usize)> = None;
            for other in 0..pieces.len() {
                if other == index || !state.active[other] {
                    continue;
                }
                work.charge_experimental_pair()?;
                let fixed = state.collisions[other]
                    .as_ref()
                    .ok_or_else(|| "separation missing partner collision".to_owned())?;
                let overlap = quantized(exact_intersection_area(&stuck_collision, fixed, work)?);
                if overlap > 0 {
                    let candidate = (overlap, other);
                    worst_partner = Some(match worst_partner {
                        None => candidate,
                        Some(current) if candidate.0 > current.0 => candidate,
                        Some(current) => current,
                    });
                }
            }
            if let Some((_, partner)) = worst_partner {
                let center = |collision: &PolygonSet| {
                    collision.bounds().map(|bounds| {
                        (
                            (bounds.min_x + bounds.max_x) * 0.5,
                            (bounds.min_y + bounds.max_y) * 0.5,
                        )
                    })
                };
                let partner_collision = state.collisions[partner]
                    .as_ref()
                    .ok_or_else(|| "separation missing partner collision".to_owned())?
                    .clone();
                if let (Some(stuck_center), Some(partner_center)) =
                    (center(&stuck_collision), center(&partner_collision))
                {
                    let axis_x = stuck_center.0 - partner_center.0;
                    let axis_y = stuck_center.1 - partner_center.1;
                    let norm = (axis_x * axis_x + axis_y * axis_y).sqrt();
                    if norm > 1e-9 {
                        let unit = (axis_x / norm, axis_y / norm);
                        let pair_total = |a: &PolygonSet,
                                          b: &PolygonSet,
                                          work: &mut RunWork|
                         -> Result<i128, String> {
                            let mut total = 0i128;
                            for other in 0..pieces.len() {
                                if other == index || other == partner || !state.active[other] {
                                    continue;
                                }
                                work.charge_experimental_pair()?;
                                let fixed = state.collisions[other]
                                    .as_ref()
                                    .ok_or_else(|| "separation missing collision".to_owned())?;
                                total += quantized(exact_intersection_area(a, fixed, work)?);
                                work.charge_experimental_pair()?;
                                total += quantized(exact_intersection_area(b, fixed, work)?);
                            }
                            work.charge_experimental_pair()?;
                            total += quantized(exact_intersection_area(a, b, work)?);
                            Ok(total)
                        };
                        let entry_pair_total =
                            pair_total(&stuck_collision, &partner_collision, work)?;
                        'pair: for radius in SEPARATION_RADII_MM {
                            if probes >= SEPARATION_PROBES_PER_MOVE {
                                break;
                            }
                            probes += 1;
                            lns.separation_probes = lns.separation_probes.saturating_add(1);
                            let half = radius * 0.5;
                            let mut moved_a = state.placements[index].clone();
                            moved_a.translate_x += unit.0 * half;
                            moved_a.translate_y += unit.1 * half;
                            let mut moved_b = state.placements[partner].clone();
                            moved_b.translate_x -= unit.0 * half;
                            moved_b.translate_y -= unit.1 * half;
                            let collision_a =
                                build_collision(pieces[index], &moved_a, settings, work)?;
                            let collision_b =
                                build_collision(pieces[partner], &moved_b, settings, work)?;
                            let bounds_ok = |collision: &PolygonSet| {
                                collision.fits_rect(
                                    inset,
                                    inset,
                                    settings.sheet_short_axis_mm - inset,
                                    settings.sheet_long_axis_mm - inset,
                                )
                            };
                            if !bounds_ok(&collision_a) || !bounds_ok(&collision_b) {
                                continue;
                            }
                            let total = pair_total(&collision_a, &collision_b, work)?;
                            if total < entry_pair_total {
                                state.placements[index] = moved_a;
                                state.collisions[index] = Some(Arc::new(collision_a));
                                state.placements[partner] = moved_b;
                                state.collisions[partner] = Some(Arc::new(collision_b));
                                if !soft.contains(&partner) {
                                    soft.push(partner);
                                    soft.sort_by(|first, second| {
                                        pieces[*first].id.cmp(pieces[*second].id)
                                    });
                                }
                                lns.separation_pair_moves =
                                    lns.separation_pair_moves.saturating_add(1);
                                lns.separation_moves = lns.separation_moves.saturating_add(1);
                                best = None;
                                break 'pair;
                            }
                        }
                        if lns.separation_pair_moves > 0 {
                            // A committed pair move restarts the outer loop.
                        }
                    }
                }
            }
        }
        let committed_pair_move_this_iteration = false;
        let _ = committed_pair_move_this_iteration;
        let Some((_, placement, collision)) = best else {
            // A pair move may have just been committed; if so, resume the
            // outer descent from the updated state instead of recruiting.
            if lns.separation_pair_moves > pair_moves_before {
                continue;
            }
            // No strict soft-piece improvement anywhere on the ladder: recruit
            // the anchor contributing the largest exact overlap against the
            // stuck piece into the soft set (bilateral separation). If it is
            // already soft, the configuration is genuinely stuck.
            let stuck_collision = state.collisions[index]
                .as_ref()
                .ok_or_else(|| "separation missing stuck collision".to_owned())?
                .clone();
            let mut worst_anchor: Option<(i128, usize)> = None;
            for other in 0..pieces.len() {
                if other == index || !state.active[other] || soft.contains(&other) {
                    continue;
                }
                work.charge_experimental_pair()?;
                let fixed = state.collisions[other]
                    .as_ref()
                    .ok_or_else(|| "separation missing anchor collision".to_owned())?;
                let overlap = quantized(exact_intersection_area(&stuck_collision, fixed, work)?);
                if overlap > 0 {
                    let candidate = (overlap, other);
                    worst_anchor = Some(match worst_anchor {
                        None => candidate,
                        Some(current) => {
                            if candidate.0 > current.0
                                || (candidate.0 == current.0
                                    && pieces[candidate.1].id < pieces[current.1].id)
                            {
                                candidate
                            } else {
                                current
                            }
                        }
                    });
                }
            }
            // Global relocation escape: deactivate the stuck piece and
            // reinsert it anywhere through the depth-ranked, hazard-screened
            // generator. A successful relocation zeroes that piece's overlap
            // without touching any other pair, so total raw overlap strictly
            // decreases and the descent resumes with global progress.
            if relocations < SEPARATION_RELOCATIONS_PER_ROUND {
                let saved_placement = state.placements[index].clone();
                let saved_collision = state.collisions[index].clone();
                state.active[index] = false;
                state.collisions[index] = None;
                let mut screen = JaguaHazardIndex::from_catalog_active(
                    pieces,
                    settings,
                    settings.sheet_long_axis_mm,
                    &state.placements.iter().map(hazard_pose).collect::<Vec<_>>(),
                    &state.active,
                    hazard_catalog,
                )
                .map_err(|error| format!("separation relocation index: {error}"))?;
                let placed = reconstruct_insert_piece(
                    pieces,
                    settings,
                    hints,
                    state,
                    lns_seed,
                    400 + round * SEPARATION_RELOCATIONS_PER_ROUND + relocations,
                    index,
                    true,
                    Some(&mut screen),
                    recon,
                    work,
                )?;
                relocations += 1;
                if placed {
                    lns.separation_relocations = lns.separation_relocations.saturating_add(1);
                    continue;
                }
                state.placements[index] = saved_placement;
                state.collisions[index] = saved_collision;
                state.active[index] = true;
            }
            let Some((_, recruit)) = worst_anchor else {
                // Weight escalation: no anchor to recruit; increment the
                // weights of every currently overlapping pair touching the
                // stuck piece and retry, allowing ridge-crossing trades. Cap
                // escalations through the shared move budget.
                let stuck = state.collisions[index]
                    .as_ref()
                    .ok_or_else(|| "separation missing stuck collision".to_owned())?
                    .clone();
                let mut bumped = false;
                for other in 0..pieces.len() {
                    if other == index || !state.active[other] {
                        continue;
                    }
                    work.charge_experimental_pair()?;
                    let fixed = state.collisions[other]
                        .as_ref()
                        .ok_or_else(|| "separation missing collision".to_owned())?;
                    if exact_intersection_area(&stuck, fixed, work)? > 0.0 {
                        let key = if index < other {
                            (index, other)
                        } else {
                            (other, index)
                        };
                        *pair_weights.entry(key).or_insert(1) += 1;
                        bumped = true;
                    }
                }
                if bumped {
                    lns.separation_weight_bumps = lns.separation_weight_bumps.saturating_add(1);
                    if lns.separation_weight_bumps <= 40 {
                        continue;
                    }
                }
                return Ok(false);
            };
            soft.push(recruit);
            soft.sort_by(|first, second| pieces[*first].id.cmp(pieces[*second].id));
            lns.separation_recruits = lns.separation_recruits.saturating_add(1);
            continue;
        };
        state.placements[index] = placement;
        state.collisions[index] = Some(Arc::new(collision));
        lns.separation_moves = lns.separation_moves.saturating_add(1);
    }
    // Move budget exhausted; check the residual.
    for index in &soft {
        let collision = state.collisions[*index]
            .as_ref()
            .ok_or_else(|| "separation missing soft collision".to_owned())?
            .clone();
        let (_weighted, raw) = piece_overlap(state, *index, &collision, &pair_weights, work)?;
        if raw > 0 {
            return Ok(false);
        }
    }
    lns.separation_zero_overlap = lns.separation_zero_overlap.saturating_add(1);
    Ok(true)
}

/// Deterministic trapped-void raster for the mode-17 vacancy-transport
/// acceptance signal. The strip up to the current frontier is rasterized at
/// a fixed cell size; a cell is free when its center lies inside no active
/// expanded collision, and free cells flood-fill four-connected from the
/// above-frontier band. The returned value counts free cells NOT connected
/// to that band - the trapped voids whose drainage upward is exactly the
/// slack routing the piece-centric keys cannot see. Guidance only: validity
/// still rests entirely on the exact gates.
fn trapped_void_cells(
    state: &VacancyState,
    settings: GeneralFastSettings,
    frontier_grid: i64,
) -> usize {
    const CELL_MM: f64 = 2.0;
    let width = settings.sheet_short_axis_mm;
    let depth = (frontier_grid as f64) / 1000.0 + 2.0 * CELL_MM;
    let columns = (width / CELL_MM).ceil() as usize;
    let rows = (depth / CELL_MM).ceil() as usize;
    if columns == 0 || rows == 0 {
        return 0;
    }
    let actives = state
        .collisions
        .iter()
        .enumerate()
        .filter(|(index, _)| state.active[*index])
        .filter_map(|(_, collision)| collision.as_ref())
        .map(|collision| (collision.bounds(), collision))
        .collect::<Vec<_>>();
    let mut free = vec![true; columns * rows];
    for row in 0..rows {
        let y = (row as f64 + 0.5) * CELL_MM;
        for column in 0..columns {
            let x = (column as f64 + 0.5) * CELL_MM;
            for (bounds, collision) in &actives {
                if let Some(bounds) = bounds {
                    if x < bounds.min_x || x > bounds.max_x || y < bounds.min_y || y > bounds.max_y
                    {
                        continue;
                    }
                }
                if !matches!(
                    collision.contains_point(IrregularPoint::new(x, y)),
                    PointInPolygonResult::IsOutside
                ) {
                    free[row * columns + column] = false;
                    break;
                }
            }
        }
    }
    // Flood-fill four-connected from the top row (the above-frontier band).
    let mut reachable = vec![false; columns * rows];
    let mut stack = Vec::new();
    let top = rows - 1;
    for column in 0..columns {
        let cell = top * columns + column;
        if free[cell] {
            reachable[cell] = true;
            stack.push(cell);
        }
    }
    while let Some(cell) = stack.pop() {
        let row = cell / columns;
        let column = cell % columns;
        let mut push = |candidate: usize| {
            if free[candidate] && !reachable[candidate] {
                reachable[candidate] = true;
                stack.push(candidate);
            }
        };
        if column > 0 {
            push(cell - 1);
        }
        if column + 1 < columns {
            push(cell + 1);
        }
        if row > 0 {
            push(cell - columns);
        }
        if row + 1 < rows {
            push(cell + columns);
        }
    }
    free.iter()
        .zip(reachable.iter())
        .filter(|(is_free, is_reachable)| **is_free && !**is_reachable)
        .count()
}

/// Mode-18 frontier-band feasibility diagnostic: for each of the deepest
/// FRONTIER_BAND_PIECES pieces, remove the piece and sweep a deterministic
/// lattice of candidate poses (all conflict-ruin orientations crossed with an
/// 8 mm translation lattice over the sub-frontier strip), hazard-screening
/// each pose and exactly confirming survivors, searching for ANY exact-valid
/// pose whose collision frontier lies strictly below the current global
/// frontier. The result converts the open search question into a measured
/// fact: either a sub-frontier pose exists that the search misses, or the
/// incumbent is certified one-piece locally optimal at this resolution.
fn frontier_band_feasibility(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<RelaxedState, String> {
    const FRONTIER_BAND_PIECES: usize = 5;
    const LATTICE_MM: f64 = 8.0;
    let settings = fast_settings;
    let mut state = VacancyState {
        collisions: baseline
            .placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                build_collision(pieces[index], placement, settings, work)
                    .map(|collision| Some(Arc::new(collision)))
            })
            .collect::<Result<Vec<_>, _>>()?,
        placements: baseline.placements.clone(),
        active: vec![true; pieces.len()],
        last_transition: None,
    };
    let hazard_catalog = Arc::new(
        JaguaHazardCatalog::new(pieces, settings)
            .map_err(|error| format!("feasibility hazard catalog: {error}"))?,
    );
    let frontier_grid = state
        .collisions
        .iter()
        .flatten()
        .filter_map(|collision| collision.bounds())
        .map(|bounds| grid_key(bounds.max_y))
        .max()
        .unwrap_or(0);
    let mut by_depth = (0..pieces.len())
        .filter_map(|index| {
            state.collisions[index]
                .as_ref()
                .and_then(|collision| collision.bounds())
                .map(|bounds| (grid_key(bounds.max_y), index))
        })
        .collect::<Vec<_>>();
    by_depth.sort_by(|first, second| {
        second
            .0
            .cmp(&first.0)
            .then_with(|| pieces[first.1].id.cmp(pieces[second.1].id))
    });
    let inset = collision_sheet_inset_mm(settings);
    let seed = parent_seed_key(&state, pieces);
    let mut rows = Vec::new();
    for (piece_depth, index) in by_depth.into_iter().take(FRONTIER_BAND_PIECES) {
        let saved_placement = state.placements[index].clone();
        let saved_collision = state.collisions[index].clone();
        state.active[index] = false;
        state.collisions[index] = None;
        let mut screen = JaguaHazardIndex::from_catalog_active(
            pieces,
            settings,
            settings.sheet_long_axis_mm,
            &state.placements.iter().map(hazard_pose).collect::<Vec<_>>(),
            &state.active,
            &hazard_catalog,
        )
        .map_err(|error| format!("feasibility screen index: {error}"))?;
        let orientations = conflict_ruin_orientations(
            pieces[index],
            &saved_placement,
            derive_seed(seed, 0, index),
        );
        let mut screened = 0usize;
        let mut confirmed = 0usize;
        let mut best_sub_frontier: Option<(i64, RelaxedPlacement)> = None;
        for (rotation_deg, mirrored) in orientations {
            let orientation = RelaxedPlacement {
                input_index: index,
                rotation_deg,
                mirrored,
                translate_x: 0.0,
                translate_y: 0.0,
            };
            let local = build_collision(pieces[index], &orientation, settings, work)?;
            let Some(local_bounds) = local.bounds() else {
                continue;
            };
            let min_x = inset - local_bounds.min_x;
            let max_x = settings.sheet_short_axis_mm - inset - local_bounds.max_x;
            let min_y = inset - local_bounds.min_y;
            // The pose frontier must land strictly below the global frontier.
            let max_y = (frontier_grid as f64) / 1000.0 - 0.001 - local_bounds.max_y;
            if min_x > max_x || min_y > max_y {
                continue;
            }
            let steps_x = ((max_x - min_x) / LATTICE_MM).floor() as usize + 1;
            let steps_y = ((max_y - min_y) / LATTICE_MM).floor() as usize + 1;
            for step_y in 0..steps_y {
                for step_x in 0..steps_x {
                    let candidate = RelaxedPlacement {
                        input_index: index,
                        rotation_deg,
                        mirrored,
                        translate_x: min_x + step_x as f64 * LATTICE_MM,
                        translate_y: min_y + step_y as f64 * LATTICE_MM,
                    };
                    screened += 1;
                    work.diagnostics.hazard_queries =
                        work.diagnostics.hazard_queries.saturating_add(1);
                    if work.diagnostics.hazard_queries > MAX_HAZARD_QUERIES {
                        return Err(work.cap("hazard-query budget exhausted"));
                    }
                    match screen.query_unplaced(index, hazard_pose(&candidate)) {
                        Ok(GeneralHazardQuery::Complete {
                            boundary,
                            colliding_piece_ids,
                        }) => {
                            if boundary || !colliding_piece_ids.is_empty() {
                                continue;
                            }
                        }
                        Ok(_) => {}
                        Err(error) if error.to_string().contains("query envelope") => continue,
                        Err(error) => {
                            return Err(format!("feasibility screen: {error}"));
                        }
                    }
                    work.diagnostics.exact_finalist_rows =
                        work.diagnostics.exact_finalist_rows.saturating_add(1);
                    if work.diagnostics.exact_finalist_rows > MAX_EXACT_FINALIST_ROWS {
                        return Err(work.cap("exact-finalist row budget exhausted"));
                    }
                    let collision = build_collision(pieces[index], &candidate, settings, work)?;
                    if !collision.fits_rect(
                        inset,
                        inset,
                        settings.sheet_short_axis_mm - inset,
                        settings.sheet_long_axis_mm - inset,
                    ) {
                        continue;
                    }
                    let Some(bounds) = collision.bounds() else {
                        continue;
                    };
                    let pose_frontier = grid_key(bounds.max_y);
                    if pose_frontier >= frontier_grid {
                        continue;
                    }
                    let mut overlapping = false;
                    for other in 0..pieces.len() {
                        if other == index || !state.active[other] {
                            continue;
                        }
                        work.charge_experimental_pair()?;
                        let fixed = state.collisions[other]
                            .as_ref()
                            .ok_or_else(|| "feasibility missing collision".to_owned())?;
                        if exact_intersection_area(&collision, fixed, work)? > 0.0 {
                            overlapping = true;
                            break;
                        }
                    }
                    if overlapping {
                        continue;
                    }
                    confirmed += 1;
                    if best_sub_frontier
                        .as_ref()
                        .is_none_or(|(best, _)| pose_frontier < *best)
                    {
                        best_sub_frontier = Some((pose_frontier, candidate.clone()));
                    }
                }
            }
        }
        rows.push(GeneralPersistentVacancyFeasibilityRow {
            piece_id: pieces[index].id.to_owned(),
            piece_frontier_grid: piece_depth,
            lattice_poses_screened: screened,
            exact_valid_sub_frontier_poses: confirmed,
            best_sub_frontier_grid: best_sub_frontier.as_ref().map(|(depth, _)| *depth),
        });
        state.placements[index] = saved_placement;
        state.collisions[index] = saved_collision;
        state.active[index] = true;
    }
    diagnostics.frontier_feasibility = Some(rows);
    Ok(baseline)
}

fn settle_sweep(
    state: &mut VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    inset: f64,
    diagonal: bool,
    settle: &mut GeneralPersistentVacancySettleDiagnostics,
    work: &mut RunWork,
) -> Result<(), String> {
    let mut order = (0..pieces.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        let min_y = state.collisions[*index]
            .as_ref()
            .and_then(|collision| collision.bounds())
            .map(|bounds| grid_key(bounds.min_y))
            .unwrap_or(i64::MAX);
        (min_y, pieces[*index].id)
    });
    for piece_index in order {
        settle.attempts += 1;
        work.diagnostics.selected_piece_slots =
            work.diagnostics.selected_piece_slots.saturating_add(1);
        if work.diagnostics.selected_piece_slots > MAX_SELECTED_PIECE_SLOTS {
            return Err(work.cap("selected-piece slot budget exhausted"));
        }
        work.charge_source_features(pieces[piece_index].polygon.vertex_count().saturating_mul(2))?;
        work.diagnostics.orientation_streams =
            work.diagnostics.orientation_streams.saturating_add(1);
        if work.diagnostics.orientation_streams > MAX_ORIENTATION_STREAMS {
            return Err(work.cap("orientation-stream budget exhausted"));
        }
        let mut temp = state.clone();
        temp.active[piece_index] = false;
        temp.collisions[piece_index] = None;
        // FIX 3: the settle phase owns a full collision state plus one
        // temporary clone per attempt; charge that live set against the
        // retained-memory gate exactly like the population phases.
        let live_bytes = state_slice_bytes(std::slice::from_ref(state))
            .saturating_add(state_slice_bytes(std::slice::from_ref(&temp)))
            .saturating_add(2usize.saturating_mul(size_of::<VacancyState>()));
        work.diagnostics.total_retained_peak_bytes =
            work.diagnostics.total_retained_peak_bytes.max(live_bytes);
        if live_bytes > MAX_RETAINED_BYTES {
            return Err(work.cap("settle live-state memory budget exhausted"));
        }
        let mut best_placement = state.placements[piece_index].clone();
        let mut best_collision: Option<Arc<PolygonSet>> = None;
        let mut probes = 0usize;
        // Every accepted probe strictly lowers the piece, so the compaction
        // is monotone. The plain settle keeps the single downward ladder; a
        // purely lateral phase was tried and rejected. Diagonal settling
        // (mode 14) additionally probes the two 45-degree descent
        // directions, which can slide a piece along a slope that blocks the
        // straight drop; acceptance still requires strict descent, so no
        // lateral drift without depth progress is possible.
        const DESCENT_DIRECTIONS: [(f64, f64); 3] = [
            (0.0, -1.0),
            (-0.7071067811865476, -0.7071067811865476),
            (0.7071067811865476, -0.7071067811865476),
        ];
        let directions: &[(f64, f64)] = if diagonal {
            &DESCENT_DIRECTIONS
        } else {
            &DESCENT_DIRECTIONS[..1]
        };
        'ladder: for (direction_x, direction_y) in directions.iter().copied() {
            for step in SETTLE_STEP_LADDER_MM {
                loop {
                    if probes >= SETTLE_PROBES_PER_ATTEMPT {
                        break 'ladder;
                    }
                    let mut candidate = best_placement.clone();
                    candidate.translate_x += step * direction_x;
                    candidate.translate_y += step * direction_y;
                    probes += 1;
                    settle.exact_rows += 1;
                    work.diagnostics.exact_finalist_rows =
                        work.diagnostics.exact_finalist_rows.saturating_add(1);
                    if work.diagnostics.exact_finalist_rows > MAX_EXACT_FINALIST_ROWS {
                        return Err(work.cap("exact-finalist row budget exhausted"));
                    }
                    let collision =
                        build_collision(pieces[piece_index], &candidate, settings, work)?;
                    if !collision.fits_rect(
                        inset,
                        inset,
                        settings.sheet_short_axis_mm - inset,
                        settings.sheet_long_axis_mm - inset,
                    ) {
                        break;
                    }
                    let descended =
                        grid_key(candidate.translate_y) < grid_key(best_placement.translate_y);
                    if !descended {
                        break;
                    }
                    let mut overlapping = false;
                    for fixed_index in 0..pieces.len() {
                        if fixed_index == piece_index || !temp.active[fixed_index] {
                            continue;
                        }
                        work.charge_experimental_pair()?;
                        let fixed = temp.collisions[fixed_index].as_ref().ok_or_else(|| {
                            format!("active piece {fixed_index} has no collision")
                        })?;
                        if exact_intersection_area(&collision, fixed, work)? > 0.0 {
                            overlapping = true;
                            break;
                        }
                    }
                    if overlapping {
                        break;
                    }
                    best_placement = candidate;
                    best_collision = Some(Arc::new(collision));
                }
            }
        }
        if let Some(collision) = best_collision {
            state.placements[piece_index] = best_placement;
            state.collisions[piece_index] = Some(collision);
            settle.accepted_moves += 1;
        }
    }
    Ok(())
}

/// Mode-13 guided reconstruction: rebuilds the layout from an external hint
/// fixture under the engine's own exact contract. Pieces are inserted in
/// ascending hint-depth order; each insertion ranks displacement probes,
/// generator candidates, and upward shelf fallbacks by canonical-grid L1
/// distance from the hint pose and exact-confirms them in order until the
/// first pose with full-strip containment and zero exact pair intersection
/// against every already-placed piece. Pieces whose pockets are closed
/// during the first pass are deferred and retried after every other piece
/// has settled; the deferred pass completes every retry before failing so
/// the diagnostics record the true unplaced set. The hints are never
/// trusted: the completed state must pass the unchanged dual publication
/// audit. Like the rest of the engine, candidate generation quantizes
/// platform trigonometry onto the canonical grid, so replay identity is
/// promised only on the recorded machine/toolchain identity.
fn reconstruct_from_hints(
    pieces: &[GeneralFastPiece<'_>],
    fast_settings: GeneralFastSettings,
    target_depth_mm: f64,
    hints: &RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
) -> Result<(VacancyState, f64), String> {
    let target_settings = GeneralFastSettings {
        sheet_long_axis_mm: target_depth_mm,
        ..fast_settings
    };
    let mut state = VacancyState {
        placements: hints.placements.clone(),
        active: vec![false; pieces.len()],
        collisions: vec![None; pieces.len()],
        last_transition: None,
    };
    let hint_state = VacancyState {
        placements: hints.placements.clone(),
        active: vec![true; pieces.len()],
        collisions: vec![None; pieces.len()],
        last_transition: None,
    };
    let reconstruction_seed = parent_seed_key(&hint_state, pieces);
    let mut order = (0..pieces.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        (
            grid_key(hints.placements[*index].translate_y),
            pieces[*index].id,
        )
    });
    let mut recon = GeneralPersistentVacancyReconstructionDiagnostics {
        insertions: 0,
        exact_rows: 0,
        rows_per_piece_cap: RECONSTRUCTION_ROWS_PER_PIECE,
        deferred_first_pass: 0,
        failed_piece_id: None,
        failed_piece_count: 0,
    };
    let mut deferred = Vec::new();
    for (ordinal, piece_index) in order.into_iter().enumerate() {
        let placed = reconstruct_insert_piece(
            pieces,
            target_settings,
            hints,
            &mut state,
            reconstruction_seed,
            ordinal,
            piece_index,
            false,
            None,
            &mut recon,
            work,
        )?;
        if !placed {
            deferred.push(piece_index);
            recon.deferred_first_pass = recon.deferred_first_pass.saturating_add(1);
        }
    }
    // Deferred second pass: pieces whose hint pockets were closed during the
    // first pass retry after every other piece has settled, when the shelf
    // region and any reopened pockets are maximally available.
    let mut still_failed = Vec::new();
    for (retry_ordinal, piece_index) in deferred.into_iter().enumerate() {
        let placed = reconstruct_insert_piece(
            pieces,
            target_settings,
            hints,
            &mut state,
            reconstruction_seed,
            61 + retry_ordinal,
            piece_index,
            false,
            None,
            &mut recon,
            work,
        )?;
        if !placed {
            still_failed.push(pieces[piece_index].id.to_owned());
        }
    }
    if let Some(first_failed) = still_failed.first() {
        recon.failed_piece_id = Some(first_failed.clone());
        recon.failed_piece_count = still_failed.len();
        diagnostics.reconstruction = Some(recon.clone());
        return Err(format!(
            "seeded reconstruction left {} pieces without an exact-valid pose after the deferred pass, first {}",
            still_failed.len(),
            first_failed
        ));
    }
    diagnostics.reconstruction = Some(recon.clone());
    diagnostics.complete_states = diagnostics.complete_states.saturating_add(1);
    if let Err(reason) = audit_state(&state, pieces, target_settings, true, work) {
        if !reason.starts_with("cap: ") {
            diagnostics.publication_rejections =
                diagnostics.publication_rejections.saturating_add(1);
        }
        return Err(reason);
    }
    let placements = fast_placements(&state, pieces, false);
    let independent = coupled_independent_source_depth(pieces, &placements, target_settings)
        .map_err(|error| format!("persistent vacancy reconstructed depth: {error}"))?;
    Ok((state, independent))
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_insert_piece(
    pieces: &[GeneralFastPiece<'_>],
    target_settings: GeneralFastSettings,
    hints: &RelaxedState,
    state: &mut VacancyState,
    reconstruction_seed: u64,
    ordinal: usize,
    piece_index: usize,
    rank_by_depth: bool,
    hazard_screen: Option<&mut JaguaHazardIndex>,
    recon: &mut GeneralPersistentVacancyReconstructionDiagnostics,
    work: &mut RunWork,
) -> Result<bool, String> {
    work.diagnostics.selected_piece_slots = work.diagnostics.selected_piece_slots.saturating_add(1);
    if work.diagnostics.selected_piece_slots > MAX_SELECTED_PIECE_SLOTS {
        return Err(work.cap("selected-piece slot budget exhausted"));
    }
    work.charge_source_features(pieces[piece_index].polygon.vertex_count().saturating_mul(2))?;
    // Conservative fixed bound for the per-attempt transient buffers
    // (candidate rows, shelf poses, ranked vector, bucket set); they are
    // structurally bounded far below this figure.
    const RECONSTRUCTION_TRANSIENT_BYTES: usize = 96 * 1024;
    let live_bytes = state_slice_bytes(std::slice::from_ref(state))
        .saturating_add(2usize.saturating_mul(size_of::<VacancyState>()))
        .saturating_add(RECONSTRUCTION_TRANSIENT_BYTES);
    work.diagnostics.total_retained_peak_bytes =
        work.diagnostics.total_retained_peak_bytes.max(live_bytes);
    if live_bytes > MAX_RETAINED_BYTES {
        return Err(work.cap("reconstruction live-state memory budget exhausted"));
    }
    let inset = collision_sheet_inset_mm(target_settings);
    let hint = &hints.placements[piece_index];
    let hint_x = grid_key(hint.translate_x);
    let hint_y = grid_key(hint.translate_y);
    let angle_seed = derive_seed(
        reconstruction_seed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN,
        ordinal,
        piece_index,
    );
    let orientations = conflict_ruin_orientations(pieces[piece_index], hint, angle_seed);
    let mut candidates = Vec::new();
    // Deterministic displacement probes around the hint at the hint
    // orientation: the reconstruction usually needs a sub-millimetre shift
    // away from neighbors that sit at the hint contract's tighter
    // separation, and the general position generator's position cap crowds
    // those poses out.
    const PROBE_RADII_MM: [f64; 12] = [
        0.128, 0.256, 0.384, 0.512, 0.768, 1.024, 1.536, 2.048, 3.072, 4.096, 6.144, 8.192,
    ];
    const PROBE_DIRECTIONS: [(f64, f64); 8] = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (0.7071067811865476, 0.7071067811865476),
        (-0.7071067811865476, 0.7071067811865476),
        (0.7071067811865476, -0.7071067811865476),
        (-0.7071067811865476, -0.7071067811865476),
    ];
    let hint_orientation = RelaxedPlacement {
        input_index: piece_index,
        rotation_deg: hint.rotation_deg,
        mirrored: hint.mirrored,
        translate_x: 0.0,
        translate_y: 0.0,
    };
    let hint_local = build_collision(
        pieces[piece_index],
        &hint_orientation,
        target_settings,
        work,
    )?;
    let hint_local_bounds = hint_local
        .bounds()
        .ok_or_else(|| "reconstruction hint orientation has empty geometry".to_owned())?;
    let hint_local_min_y = hint_local_bounds.min_y;
    let hint_local_max_y = hint_local_bounds.max_y;
    let probe_key = |probe: &RelaxedPlacement| -> u64 {
        if rank_by_depth {
            grid_key(hint_local_max_y + probe.translate_y)
                .max(0)
                .unsigned_abs()
        } else {
            grid_key(probe.translate_x)
                .abs_diff(hint_x)
                .saturating_add(grid_key(probe.translate_y).abs_diff(hint_y))
        }
    };
    candidates.push((probe_key(hint), 0usize, hint.clone()));
    for radius in PROBE_RADII_MM {
        for (direction_x, direction_y) in PROBE_DIRECTIONS {
            let mut probe = hint.clone();
            probe.translate_x += radius * direction_x;
            probe.translate_y += radius * direction_y;
            candidates.push((probe_key(&probe), 0usize, probe));
        }
    }
    // Upward shelf fallback: the region above the current frontier is empty
    // during bottom-up reconstruction, so a piece whose hint pocket is
    // laterally closed under the tighter engine contract can escape upward;
    // the later settling ladder recompacts the layout. Shelf poses anchor
    // the piece's hint-orientation material bottom just above the frontier.
    let frontier_y = state
        .collisions
        .iter()
        .flatten()
        .filter_map(|collision| collision.bounds())
        .map(|bounds| bounds.max_y)
        .fold(0.0f64, f64::max);
    let mut shelf_candidates = Vec::new();
    for step in 1..=12u32 {
        for lateral in [0.0f64, -4.0, 4.0, -8.0, 8.0] {
            let mut probe = hint.clone();
            probe.translate_x += lateral;
            probe.translate_y = frontier_y - hint_local_min_y + 0.6 * f64::from(step);
            shelf_candidates.push(probe);
        }
    }
    for (orientation_ordinal, (rotation_deg, mirrored)) in orientations.into_iter().enumerate() {
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
            build_collision(pieces[piece_index], &orientation, target_settings, work)?;
        let position_seed = derive_seed(
            reconstruction_seed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN,
            ordinal
                .saturating_mul(ORIENTATIONS_PER_PIECE)
                .saturating_add(orientation_ordinal),
            piece_index,
        );
        let proposals = vacancy_positions(
            hint,
            &orientation,
            &local_collision,
            state,
            target_settings,
            position_seed,
            work,
        )?;
        let local_max_y = local_collision
            .bounds()
            .ok_or_else(|| "reconstruction orientation has empty geometry".to_owned())?
            .max_y;
        for placement in proposals {
            let key = if rank_by_depth {
                // Lowest-fit: rank by the approximate landing frontier so a
                // lifted piece claims the deepest pocket anywhere on the
                // sheet rather than returning near its old pose.
                grid_key(local_max_y + placement.translate_y)
                    .max(0)
                    .unsigned_abs()
            } else {
                grid_key(placement.translate_x)
                    .abs_diff(hint_x)
                    .saturating_add(grid_key(placement.translate_y).abs_diff(hint_y))
            };
            candidates.push((key, orientation_ordinal, placement));
        }
    }
    candidates.sort_by(|first, second| {
        first
            .0
            .cmp(&second.0)
            .then_with(|| first.1.cmp(&second.1))
            .then_with(|| placement_key(&first.2).cmp(&placement_key(&second.2)))
    });
    // The last RECONSTRUCTION_SHELF_ROWS of the per-piece budget are
    // reserved for the shelf fallback so local congestion can never starve
    // it.
    const RECONSTRUCTION_SHELF_ROWS: usize = 60;
    let local_row_cap = RECONSTRUCTION_ROWS_PER_PIECE - RECONSTRUCTION_SHELF_ROWS;
    let mut rows = 0usize;
    let mut tried_buckets = BTreeSet::new();
    let ranked = candidates
        .into_iter()
        .map(|(_, orientation_ordinal, candidate)| (false, orientation_ordinal, candidate))
        .chain(
            shelf_candidates
                .into_iter()
                .map(|candidate| (true, 0usize, candidate)),
        )
        .collect::<Vec<_>>();
    let mut hazard_screen = hazard_screen;
    for (is_shelf, orientation_ordinal, candidate) in ranked {
        if rows >= RECONSTRUCTION_ROWS_PER_PIECE {
            break;
        }
        if !is_shelf && rows >= local_row_cap {
            continue;
        }
        if let Some(index) = hazard_screen.as_deref_mut() {
            work.diagnostics.hazard_queries = work.diagnostics.hazard_queries.saturating_add(1);
            if work.diagnostics.hazard_queries > MAX_HAZARD_QUERIES {
                return Err(work.cap("hazard-query budget exhausted"));
            }
            match index.query_unplaced(piece_index, hazard_pose(&candidate)) {
                Ok(GeneralHazardQuery::Complete {
                    boundary,
                    colliding_piece_ids,
                }) => {
                    if boundary || !colliding_piece_ids.is_empty() {
                        continue;
                    }
                }
                Ok(_) => {}
                Err(error) if error.to_string().contains("query envelope") => continue,
                Err(error) => {
                    return Err(format!("reconstruction hazard screen: {error}"));
                }
            }
        }
        let bucket = (
            orientation_ordinal,
            grid_key(candidate.translate_x).div_euclid(256),
            grid_key(candidate.translate_y).div_euclid(256),
        );
        if !tried_buckets.insert(bucket) {
            continue;
        }
        rows += 1;
        recon.exact_rows += 1;
        work.diagnostics.exact_finalist_rows =
            work.diagnostics.exact_finalist_rows.saturating_add(1);
        if work.diagnostics.exact_finalist_rows > MAX_EXACT_FINALIST_ROWS {
            return Err(work.cap("exact-finalist row budget exhausted"));
        }
        let collision = build_collision(pieces[piece_index], &candidate, target_settings, work)?;
        if !collision.fits_rect(
            inset,
            inset,
            target_settings.sheet_short_axis_mm - inset,
            target_settings.sheet_long_axis_mm - inset,
        ) {
            continue;
        }
        let mut overlapping = false;
        for fixed_index in 0..pieces.len() {
            if !state.active[fixed_index] {
                continue;
            }
            work.charge_experimental_pair()?;
            let fixed = state.collisions[fixed_index]
                .as_ref()
                .ok_or_else(|| format!("active piece {fixed_index} has no collision"))?;
            if exact_intersection_area(&collision, fixed, work)? > 0.0 {
                overlapping = true;
                break;
            }
        }
        if overlapping {
            continue;
        }
        state.placements[piece_index] = candidate;
        state.active[piece_index] = true;
        state.collisions[piece_index] = Some(Arc::new(collision));
        recon.insertions += 1;
        return Ok(true);
    }
    Ok(false)
}

fn initial_vacancy_state(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    baseline: RelaxedState,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
    allow_complete: bool,
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
            settings.sheet_long_axis_mm - inset,
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
    if inactive_order.is_empty() && !allow_complete {
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
    let mut index = build_active_hazard_index(parent, pieces, settings, hazard_catalog)?;
    let parent_seed = parent_seed_key(parent, pieces);
    let transition_seed = derive_seed(PERSISTENT_VACANCY_SEED_DOMAIN ^ parent_seed, layer, 0);
    let mut selection = selected_inactive_pieces(parent, pieces, difficulty, layer, mode);
    // Mode 10 replaces the odd-layer coverage-insertion slot with a blocker
    // relocation slot driven by slot zero's observed ejection sets.
    let relocation_layer = matches!(mode, 10 | 12) && !layer.is_multiple_of(2);
    if relocation_layer {
        selection.indices.truncate(1);
    }
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
        relocated_piece_id: None,
        slots: Vec::with_capacity(selection.indices.len()),
    };
    let children_before_slot_zero = children.len();
    for (selected_ordinal, piece_index) in selection.indices.into_iter().enumerate() {
        expand_selected_piece(
            parent,
            &baseline[piece_index],
            pieces,
            settings,
            &mut index,
            transition_seed,
            selected_ordinal,
            piece_index,
            diagnostics,
            work,
            selected_piece_ids,
            &mut selection_diagnostics,
            children,
        )?;
    }
    if relocation_layer {
        let relocated =
            select_relocation_piece(parent, pieces, &children[children_before_slot_zero..]);
        if let Some(relocated_index) = relocated {
            selection_diagnostics.relocated_piece_id = Some(pieces[relocated_index].id.to_owned());
            let mut temp = parent.clone();
            temp.active[relocated_index] = false;
            temp.collisions[relocated_index] = None;
            let mut temp_index =
                build_active_hazard_index(&temp, pieces, settings, hazard_catalog)?;
            expand_selected_piece(
                &temp,
                &parent.placements[relocated_index],
                pieces,
                settings,
                &mut temp_index,
                transition_seed,
                1,
                relocated_index,
                diagnostics,
                work,
                selected_piece_ids,
                &mut selection_diagnostics,
                children,
            )?;
        }
    }
    parent_selections.push(selection_diagnostics);
    Ok(())
}

fn build_active_hazard_index(
    parent: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    hazard_catalog: &Arc<JaguaHazardCatalog>,
) -> Result<JaguaHazardIndex, String> {
    let poses = parent
        .placements
        .iter()
        .map(hazard_pose)
        .collect::<Vec<_>>();
    JaguaHazardIndex::from_catalog_active(
        pieces,
        settings,
        settings.sheet_long_axis_mm,
        &poses,
        &parent.active,
        hazard_catalog,
    )
    .map_err(|error| format!("persistent vacancy partial hazard index: {error}"))
}

/// Chooses the active piece a mode-10 relocation slot moves: the piece most
/// often named as an ejected blocker by slot zero's children, ties broken by
/// stable ID; when slot zero produced no ejection children, the active piece
/// whose expanded collision reaches deepest into the strip.
fn select_relocation_piece(
    parent: &VacancyState,
    pieces: &[GeneralFastPiece<'_>],
    slot_zero_children: &[VacancyState],
) -> Option<usize> {
    let mut blocker_counts: BTreeMap<usize, usize> = BTreeMap::new();
    for child in slot_zero_children {
        if let Some(transition) = &child.last_transition {
            for blocker in &transition.ejected {
                *blocker_counts.entry(*blocker).or_insert(0) += 1;
            }
        }
    }
    if let Some(best) = blocker_counts
        .iter()
        .max_by(|(first_index, first_count), (second_index, second_count)| {
            first_count
                .cmp(second_count)
                .then_with(|| pieces[**second_index].id.cmp(pieces[**first_index].id))
        })
        .map(|(index, _)| *index)
    {
        return Some(best);
    }
    (0..parent.active.len())
        .filter(|index| parent.active[*index])
        .filter_map(|index| {
            parent.collisions[index]
                .as_ref()
                .and_then(|collision| collision.bounds())
                .map(|bounds| (index, grid_key(bounds.max_y)))
        })
        .max_by(|(first_index, first_max), (second_index, second_max)| {
            first_max
                .cmp(second_max)
                .then_with(|| pieces[*second_index].id.cmp(pieces[*first_index].id))
        })
        .map(|(index, _)| index)
}

#[allow(clippy::too_many_arguments)]
fn expand_selected_piece(
    parent: &VacancyState,
    hint: &RelaxedPlacement,
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    index: &mut JaguaHazardIndex,
    transition_seed: u64,
    selected_ordinal: usize,
    piece_index: usize,
    diagnostics: &mut GeneralPersistentVacancyDiagnostics,
    work: &mut RunWork,
    selected_piece_ids: &mut BTreeSet<String>,
    selection_diagnostics: &mut GeneralPersistentVacancyParentSelectionDiagnostics,
    children: &mut Vec<VacancyState>,
) -> Result<(), String> {
    selected_piece_ids.insert(pieces[piece_index].id.to_owned());
    work.diagnostics.selected_piece_slots = work.diagnostics.selected_piece_slots.saturating_add(1);
    if work.diagnostics.selected_piece_slots > MAX_SELECTED_PIECE_SLOTS {
        return Err(work.cap("selected-piece slot budget exhausted"));
    }
    work.charge_source_features(pieces[piece_index].polygon.vertex_count().saturating_mul(2))?;
    let angle_seed = derive_seed(
        transition_seed ^ CONFLICT_RUIN_ANGLE_SEED_DOMAIN,
        selected_ordinal,
        piece_index,
    );
    let orientations = conflict_ruin_orientations(pieces[piece_index], hint, angle_seed);
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
    for (orientation_ordinal, (rotation_deg, mirrored)) in orientations.into_iter().enumerate() {
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
        let local_collision = build_collision(pieces[piece_index], &orientation, settings, work)?;
        let position_seed = derive_seed(
            transition_seed ^ CONFLICT_RUIN_POSITION_SEED_DOMAIN,
            selected_ordinal
                .saturating_mul(ORIENTATIONS_PER_PIECE)
                .saturating_add(orientation_ordinal),
            piece_index,
        );
        let proposals = vacancy_positions(
            hint,
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
        settings.sheet_long_axis_mm - inset,
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
    let max_y = settings.sheet_long_axis_mm - inset - bounds.max_y;
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
    if matches!(
        mode,
        1 | 3 | 7 | 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18
    ) {
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

fn relaxed_state_from_diagnostics_with_target(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralCoupledSeparatorPlacementDiagnostics],
    target_depth_mm: f64,
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
        strip_depth_mm: target_depth_mm,
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
    let max_y = grid_key(settings.sheet_long_axis_mm - inset);
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
    if !matches!(
        mode,
        3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18
    ) || inactive.len() <= 1
    {
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
    if matches!(
        mode,
        3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 14 | 15 | 16 | 17 | 18
    ) {
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
        .saturating_add(option_string_bytes(&diagnostics.parent_source))
        .saturating_add(diagnostics.archive.as_ref().map_or(0, |archive| {
            archive
                .revival_policy
                .capacity()
                .saturating_add(option_string_bytes(
                    &archive.final_archived_area_fingerprint,
                ))
                .saturating_add(option_string_bytes(
                    &archive.final_archived_count_fingerprint,
                ))
        }))
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
    // Heap buffers only: the inline struct storage is already covered by the
    // containing layer row's capacity term.
    (archive.revival_kind.as_ref().map_or(0, String::capacity))
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
            selection
                .relocated_piece_id
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
        assert_eq!(SETTLE_SELECTED_PIECE_SLOTS, 3 * 61);
        assert_eq!(RECONSTRUCTION_SELECTED_PIECE_SLOTS, 2 * 61);
        assert_eq!(POPULATION_SELECTED_PIECE_SLOTS, 640 + 26);
        assert_eq!(LNS_SETTLE_SWEEPS, 73);
        assert_eq!(LNS_SETTLE_SELECTED_PIECE_SLOTS, 73 * 61);
        assert_eq!(SEPARATION_RELOCATIONS_PER_ROUND, 12);
        assert_eq!(LNS_SCHEDULE_TOTAL, 536);
        assert_eq!(LNS_REINSERT_SLOTS, 536 + 24 * 12 + 2 * 3 * 536);
        assert_eq!(
            MAX_SELECTED_PIECE_SLOTS,
            640 + 26 + 183 + 122 + 73 * 61 + 4_040
        );
        assert_eq!(
            MAX_ORIENTATION_STREAMS,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040) * 12
        );
        assert_eq!(
            MAX_POSITION_SOURCE_ATTEMPTS,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040) * 12 * 529
        );
        assert_eq!(
            MAX_RETURNED_POSITIONS,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040) * 12 * 32
        );
        assert_eq!(
            MAX_HAZARD_QUERIES,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040) * 12 * 32
        );
        assert_eq!(
            MAX_PROXY_PRESSURE_VISITS,
            (640 + 26 + 183 + 122 + 73 * 61 + 4_040) * 12 * 32 * 61
        );
        assert_eq!(
            MAX_EXACT_FINALIST_ROWS,
            (640 + 26) * 8 + 183 * 64 + 122 * 192 + 73 * 61 * 64 + 4_040 * 192
        );
        assert_eq!(COMPACTION_ROUNDS, 3);
        assert_eq!(GROUP_DROP_CUTS, 61);
        assert_eq!(GROUP_DROP_PROBES_PER_CUT, 64);
        assert_eq!(GROUP_DROP_PAIR_VISITS, 3 * 61 * 64 * 61);
        assert_eq!(SEPARATION_MOVES_PER_ROUND, 200);
        assert_eq!(SEPARATION_PROBES_PER_MOVE, 96);
        assert_eq!(SEPARATION_PAIR_VISITS, 24 * 200 * 96 * 61);
        assert_eq!(SEPARATION_COLLISION_BUILDS, 24 * (4_040 / 2 + 200 * 96));
        assert_eq!(
            MAX_EXPERIMENTAL_COLLISION_BUILDS,
            3 * 61
                + (640 + 26 + 183 + 122 + 73 * 61 + 4_040) * 12
                + ((640 + 26) * 8 + 183 * 64 + 122 * 192 + 73 * 61 * 64 + 4_040 * 192)
                + 122
                + 4_040
                + 24 * (4_040 / 2 + 200 * 96)
        );
        assert_eq!(
            MAX_EXPERIMENTAL_PAIR_VISITS,
            1_830
                + ((640 + 26) * 8 + 183 * 64 + 122 * 192 + 73 * 61 * 64 + 4_040 * 192) * 60
                + 3 * 61 * 64 * 61
                + 24 * 200 * 96 * 61
        );
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

    #[test]
    fn descent_modes_enforce_target_and_pinned_parent_requirements() {
        let polygons = vec![square(10.0)];
        let pieces = vec![GeneralFastPiece {
            id: "a",
            polygon: &polygons[0],
            allow_rotation: true,
            allow_mirror: true,
        }];
        let fast = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let mut relaxed = GeneralRelaxedSettings::mixed_61_probe(0, 1);
        let parent = GeneralCoupledSeparatorArmDiagnostics::default();

        // Mode 9 without an explicit target is rejected before any work.
        let result =
            run_persistent_vacancy_population(&pieces, fast, relaxed, &parent, Some("f".into()), 9);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("require an explicit target depth"));

        // Mode 9 with a target but no pinned parent fixture is rejected.
        relaxed.persistent_vacancy_target_depth_mm = Some(90.0);
        let result = run_persistent_vacancy_population(&pieces, fast, relaxed, &parent, None, 9);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("require a pinned parent fixture"));

        // Frozen modes reject target overrides outright.
        let result =
            run_persistent_vacancy_population(&pieces, fast, relaxed, &parent, Some("f".into()), 3);
        assert!(result
            .failure_reason
            .unwrap()
            .contains("target depth overrides require modes 9-18"));

        // Non-finite and non-positive targets fail closed.
        relaxed.persistent_vacancy_target_depth_mm = Some(f64::NAN);
        let result = run_persistent_vacancy_population(
            &pieces,
            fast,
            relaxed,
            &parent,
            Some("f".into()),
            11,
        );
        assert!(result
            .failure_reason
            .unwrap()
            .contains("positive finite value"));
    }

    #[test]
    fn settle_key_orders_by_frontier_then_translation() {
        let low = SettleKey {
            max_y: 10,
            translate_y: 5,
            translate_x: 5,
        };
        let high = SettleKey {
            max_y: 11,
            translate_y: 0,
            translate_x: 0,
        };
        assert!(settle_key_less(low, high));
        assert!(!settle_key_less(high, low));
        let same_frontier_lower_y = SettleKey {
            max_y: 10,
            translate_y: 4,
            translate_x: 9,
        };
        assert!(settle_key_less(same_frontier_lower_y, low));
    }

    #[test]
    fn settle_baseline_drops_a_floating_square_onto_the_floor() {
        let polygons = vec![square(10.0), square(10.0)];
        let pieces = polygons
            .iter()
            .enumerate()
            .map(|(index, polygon)| GeneralFastPiece {
                id: ["a", "b"][index],
                polygon,
                allow_rotation: true,
                allow_mirror: true,
            })
            .collect::<Vec<_>>();
        let fast = GeneralFastSettings::deterministic_test(100.0, 100.0);
        let baseline = RelaxedState {
            placements: vec![
                RelaxedPlacement {
                    input_index: 0,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 20.0,
                    translate_y: 0.1,
                },
                RelaxedPlacement {
                    input_index: 1,
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_x: 20.0,
                    translate_y: 40.0,
                },
            ],
            strip_depth_mm: 100.0,
        };
        let mut diagnostics = GeneralPersistentVacancyDiagnostics::default();
        let mut work = RunWork::default();
        let settled =
            settle_baseline(&pieces, fast, baseline, &mut diagnostics, &mut work).unwrap();
        let settle = diagnostics.settle.expect("settle diagnostics recorded");
        let ys = settled
            .placements
            .iter()
            .map(|placement| placement.translate_y)
            .collect::<Vec<_>>();
        assert!(settle.accepted_moves >= 1, "settle: {settle:?} ys: {ys:?}");
        assert!(settle.frontier_after_grid < settle.frontier_before_grid);
        // Down-only settling drops the floating square toward the first
        // square; the exact pair gate keeps the result overlap-free and the
        // expanded-collision allowance retains a tiny gap above it.
        assert!(settled.placements[1].translate_y < 40.0);
        assert!(
            settled.placements[1].translate_y >= settled.placements[0].translate_y + 10.0 - 1e-9
        );
    }
}
